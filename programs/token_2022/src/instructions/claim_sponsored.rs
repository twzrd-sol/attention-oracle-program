use anchor_lang::accounts::account_loader::AccountLoader;
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::{
    constants::{CHANNEL_BITMAP_BYTES, CHANNEL_STATE_SEED, CLAIM_SKIM_BPS, MAX_ID_BYTES, PROTOCOL_SEED},
    errors::OracleError,
    merkle_proof::{compute_leaf, verify_proof},
    state::{ChannelSlot, ChannelState, ProtocolState},
};

// Re-use derive_subject_id from channel module
use super::channel::derive_subject_id;

const CHANNEL_STATE_VERSION: u8 = 1;
const MAX_PROOF_LEN: usize = 32;

// =============================================================================
// SPONSORED CLAIM (for auto-claim / relay)
// =============================================================================

/// Claim channel rewards with a separate payer (for gasless/auto-claim flows).
///
/// Security model: The `claimer` does NOT sign. Instead, authorization is
/// provided by the merkle proof - the claimer's pubkey is encoded in the leaf,
/// so tokens can only be sent to the wallet specified in the published tree.
///
/// Use cases:
/// - Auto-claim: Backend claims on behalf of opted-in users
/// - Relay: User builds tx, relay sponsors gas
/// - Batch claims: Keeper processes multiple claims efficiently
#[derive(Accounts)]
pub struct ClaimChannelSponsored<'info> {
    /// Transaction fee payer (relay bot, keeper, or protocol)
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Reward recipient - NOT a signer.
    /// CHECK: Authorization via merkle proof. The claimer's pubkey is hashed
    /// into the merkle leaf; proof verification fails if wrong recipient.
    pub claimer: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ OracleError::ProtocolPaused,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// CHECK: PDA is validated via derivation in handler
    #[account(mut)]
    pub channel_state: AccountLoader<'info, ChannelState>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = protocol_state,
        associated_token::token_program = token_program
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,

    /// Claimer's ATA - payer covers init costs if needed
    #[account(
        init_if_needed,
        payer = payer,  // Relay/bot pays for ATA creation
        associated_token::mint = mint,
        associated_token::authority = claimer,  // But ATA belongs to claimer
        associated_token::token_program = token_program
    )]
    pub claimer_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_channel_sponsored<'info>(
    ctx: Context<'_, '_, '_, 'info, ClaimChannelSponsored<'info>>,
    channel: String,
    epoch: u64,
    index: u32,
    amount: u64,
    id: String,
    proof: Vec<[u8; 32]>,
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;

    // === Input validation ===
    require_keys_eq!(
        ctx.accounts.mint.key(),
        protocol_state.mint,
        OracleError::InvalidMint
    );
    require!(
        id.len() <= MAX_ID_BYTES,
        OracleError::InvalidInputLength
    );
    require!(
        proof.len() <= MAX_PROOF_LEN,
        OracleError::InvalidProofLength
    );

    // === PDA validation ===
    let subject_id = derive_subject_id(&channel);
    let seeds = [
        CHANNEL_STATE_SEED,
        protocol_state.mint.as_ref(),
        subject_id.as_ref(),
    ];
    let (expected_pda, _) = Pubkey::find_program_address(&seeds, ctx.program_id);
    require_keys_eq!(
        expected_pda,
        ctx.accounts.channel_state.to_account_info().key(),
        OracleError::InvalidChannelState
    );

    // === Load and validate channel state ===
    let mut channel_state = ctx.accounts.channel_state.load_mut()?;

    require!(
        channel_state.version == CHANNEL_STATE_VERSION,
        OracleError::InvalidChannelState
    );
    require!(
        channel_state.mint == protocol_state.mint,
        OracleError::InvalidMint
    );
    require!(
        channel_state.subject == subject_id,
        OracleError::InvalidChannelState
    );

    // === Epoch slot validation ===
    ChannelSlot::validate_index(index as usize)?;
    let slot = channel_state.slot_mut(epoch);
    require!(slot.epoch == epoch, OracleError::SlotMismatch);

    // === Replay protection (bitmap check) ===
    let byte_i = (index / 8) as usize;
    let bit_mask = 1u8 << (index % 8);
    require!(byte_i < CHANNEL_BITMAP_BYTES, OracleError::InvalidIndex);
    require!(
        slot.claimed_bitmap[byte_i] & bit_mask == 0,
        OracleError::AlreadyClaimed
    );

    // === CRITICAL: Merkle proof verification ===
    // This is the authorization - claimer.key() is encoded in the leaf.
    // If someone tries to claim to a different wallet, proof fails.
    let leaf = compute_leaf(&ctx.accounts.claimer.key(), index, amount, &id);
    require!(
        verify_proof(&proof, leaf, slot.root),
        OracleError::InvalidProof
    );

    // === Mark claimed (before transfer for reentrancy safety) ===
    slot.claimed_bitmap[byte_i] |= bit_mask;
    slot.claim_count = slot.claim_count.saturating_add(1);

    // === Calculate fee and net amount ===
    let fee = (amount as u128)
        .saturating_mul(CLAIM_SKIM_BPS as u128)
        .checked_div(10_000)
        .unwrap_or(0) as u64;
    let tokens = amount.saturating_sub(fee);

    // === Transfer tokens from treasury to claimer ===
    let mint_key = protocol_state.mint;
    let seeds: &[&[u8]] = &[PROTOCOL_SEED, mint_key.as_ref(), &[protocol_state.bump]];
    let signer = &[seeds];

    let token_program = ctx.accounts.token_program.to_account_info();
    let from = ctx.accounts.treasury_ata.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let to = ctx.accounts.claimer_ata.to_account_info();
    let authority = ctx.accounts.protocol_state.to_account_info();

    crate::transfer_checked_with_remaining(
        &token_program,
        &from,
        &mint,
        &to,
        &authority,
        tokens,
        ctx.accounts.mint.decimals,
        signer,
        ctx.remaining_accounts,
    )?;

    // === Emit event for indexing ===
    emit!(SponsoredClaimEvent {
        claimer: ctx.accounts.claimer.key(),
        payer: ctx.accounts.payer.key(),
        channel: channel.clone(),
        epoch,
        index,
        amount: tokens,
        fee,
    });

    msg!(
        "Sponsored claim: {} CCM to {}, payer={}, channel={}, epoch={}",
        tokens,
        ctx.accounts.claimer.key(),
        ctx.accounts.payer.key(),
        channel,
        epoch
    );

    Ok(())
}

#[event]
pub struct SponsoredClaimEvent {
    pub claimer: Pubkey,
    pub payer: Pubkey,
    pub channel: String,
    pub epoch: u64,
    pub index: u32,
    pub amount: u64,
    pub fee: u64,
}
