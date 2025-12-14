//! Claim + Auto-Stake instruction (Channel-based)
//! Extends claim_channel_open with optional CPI to lofi-bank for auto-staking

use anchor_lang::accounts::account_loader::AccountLoader;
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use sha3::{Digest, Keccak256};

use crate::{
    constants::{
        CHANNEL_BITMAP_BYTES, CHANNEL_STATE_SEED, CLAIM_SKIM_BPS, MAX_ID_BYTES, PROTOCOL_SEED,
    },
    errors::OracleError,
    instructions::claim::{compute_leaf, verify_proof},
    state::{ChannelSlot, ChannelState, ProtocolState},
};

use lofi_bank::cpi::accounts::Stake as BankStakeAccounts;
use lofi_bank::cpi::stake as bank_stake_cpi;
use lofi_bank::program::LofiBank;

fn derive_subject_id(channel: &str) -> Pubkey {
    let mut lower = channel.as_bytes().to_vec();
    lower.iter_mut().for_each(|b| *b = b.to_ascii_lowercase());
    let hash = keccak_hashv(&[b"channel:", lower.as_slice()]);
    Pubkey::new_from_array(hash)
}

fn keccak_hashv(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    for p in parts {
        hasher.update(p);
    }
    let out = hasher.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out[..32]);
    arr
}

const CHANNEL_STATE_VERSION: u8 = 1;
const MAX_PROOF_LEN: usize = 32;

/// Claim from channel with optional auto-stake to lofi-bank
#[derive(Accounts)]
pub struct ClaimChannelAndStake<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

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

    /// CCM/TWZRD mint (Token-2022)
    pub mint: InterfaceAccount<'info, Mint>,

    /// Treasury ATA (owned by protocol_state PDA)
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = protocol_state,
        associated_token::token_program = token_program
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,

    /// Claimer ATA (create if needed)
    #[account(
        init_if_needed,
        payer = claimer,
        associated_token::mint = mint,
        associated_token::authority = claimer,
        associated_token::token_program = token_program
    )]
    pub claimer_ata: InterfaceAccount<'info, TokenAccount>,

    // --- LOFI BANK ACCOUNTS (for auto-stake) ---
    /// Lofi Bank program
    pub lofi_bank_program: Program<'info, LofiBank>,

    /// User vault PDA in lofi-bank
    /// CHECK: Validated by lofi-bank CPI (seeds = [b"user_vault", claimer.key()])
    #[account(mut)]
    pub bank_user_vault: UncheckedAccount<'info>,

    /// Treasury state PDA in lofi-bank
    /// CHECK: Validated by lofi-bank CPI (seeds = [b"treasury_state"])
    #[account(mut)]
    pub bank_treasury_state: UncheckedAccount<'info>,

    /// Bank treasury token account (ATA owned by treasury_state PDA)
    #[account(mut)]
    pub bank_treasury_token: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_channel_and_stake<'info>(
    ctx: Context<'_, '_, '_, 'info, ClaimChannelAndStake<'info>>,
    channel: String,
    epoch: u64,
    index: u32,
    amount: u64,
    id: String,
    proof: Vec<[u8; 32]>,
    auto_stake: bool,
    stake_percent: u8, // 0-100, default 50
    lock_epochs: u32,  // lock period in epochs, default 12
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;

    // Ensure provided mint matches the protocol instance
    require_keys_eq!(
        ctx.accounts.mint.key(),
        protocol_state.mint,
        OracleError::InvalidMint
    );
    require!(
        id.as_bytes().len() <= MAX_ID_BYTES,
        OracleError::InvalidInputLength
    );
    require!(
        proof.len() <= MAX_PROOF_LEN,
        OracleError::InvalidProofLength
    );

    // Derive and validate channel_state PDA
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

    // Load via Anchor's zero_copy loader
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

    // Validate index bounds
    ChannelSlot::validate_index(index as usize)?;
    let slot = channel_state.slot_mut(epoch);
    require!(slot.epoch == epoch, OracleError::SlotMismatch);

    // Check bitmap not already claimed
    let byte_i = (index / 8) as usize;
    let bit_mask = 1u8 << (index % 8);
    require!(byte_i < CHANNEL_BITMAP_BYTES, OracleError::InvalidIndex);
    require!(
        slot.claimed_bitmap[byte_i] & bit_mask == 0,
        OracleError::AlreadyClaimed
    );

    // Verify merkle proof
    let leaf = compute_leaf(&ctx.accounts.claimer.key(), index, amount, &id);
    require!(
        verify_proof(&proof, leaf, slot.root),
        OracleError::InvalidProof
    );

    // Mark as claimed
    slot.claimed_bitmap[byte_i] |= bit_mask;
    slot.claim_count = slot.claim_count.saturating_add(1);

    // Claim-time skim: keep a fixed % in the protocol treasury (source ATA) by
    // transferring less to the user.
    let fee = (amount as u128)
        .saturating_mul(CLAIM_SKIM_BPS as u128)
        .checked_div(10_000)
        .unwrap_or(0) as u64;
    let tokens = amount.saturating_sub(fee);
    let protocol_seeds: &[&[u8]] = &[
        PROTOCOL_SEED,
        protocol_state.mint.as_ref(),
        &[protocol_state.bump],
    ];
    let signer = &[protocol_seeds];

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

    // Auto-stake CPI if enabled
    if auto_stake {
        let pct = stake_percent.min(100) as u64;
        let stake_amount = tokens.saturating_mul(pct) / 100;
        let lock_period = lock_epochs.max(1);

        if stake_amount > 0 {
            let cpi_program = ctx.accounts.lofi_bank_program.to_account_info();

            let cpi_accounts = BankStakeAccounts {
                user_vault: ctx.accounts.bank_user_vault.to_account_info(),
                treasury_state: ctx.accounts.bank_treasury_state.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                user_token: ctx.accounts.claimer_ata.to_account_info(),
                treasury_token: ctx.accounts.bank_treasury_token.to_account_info(),
                payer: ctx.accounts.claimer.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
            };

            // Claimer is signing the top-level tx, so they authorize the stake
            let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
            bank_stake_cpi(cpi_ctx, stake_amount, lock_period)?;

            msg!(
                "Auto-staked {} tokens for {} epochs ({}%)",
                stake_amount,
                lock_period,
                pct
            );
        }
    }

    msg!(
        "Claimed {} tokens, channel={}, epoch={}, index={}",
        tokens,
        channel,
        epoch,
        index
    );
    Ok(())
}
