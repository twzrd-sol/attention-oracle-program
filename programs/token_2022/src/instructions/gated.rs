use anchor_lang::{accounts::account_loader::AccountLoader, prelude::*};
use sha3::{Digest, Keccak256};

use crate::{
    constants::CHANNEL_STATE_SEED, errors::OracleError,
    instructions::claim::{compute_leaf, verify_proof}, state::ChannelState,
};

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

/// Accounts required for attention-gated actions.
/// Read-only verification of merkle proof and attention threshold.
#[derive(Accounts)]
pub struct RequireAttention<'info> {
    /// The wallet being checked (does not need to sign for read-only verification)
    /// CHECK: Used only for merkle leaf computation
    pub owner: UncheckedAccount<'info>,

    /// The CCM mint this protocol instance is bound to
    /// CHECK: Used for PDA derivation
    pub mint: UncheckedAccount<'info>,

    /// Channel state holding merkle roots (zero_copy)
    /// CHECK: PDA validated in handler
    pub channel_state: AccountLoader<'info, ChannelState>,
}

/// Verify attention threshold via merkle proof. Does not claim or transfer tokens.
pub fn require_attention_ge(
    ctx: Context<RequireAttention>,
    channel: String,
    epoch: u64,
    index: u32,
    amount: u64,
    id: String,
    proof: Vec<[u8; 32]>,
    min_attention: u64,
) -> Result<()> {
    let subject_id = derive_subject_id(&channel);
    let mint_key = ctx.accounts.mint.key();
    let seeds = [CHANNEL_STATE_SEED, mint_key.as_ref(), subject_id.as_ref()];
    let (expected_pda, _) = Pubkey::find_program_address(&seeds, ctx.program_id);
    require_keys_eq!(
        expected_pda,
        ctx.accounts.channel_state.to_account_info().key(),
        OracleError::InvalidChannelState
    );

    let channel_state = ctx.accounts.channel_state.load()?;
    require!(channel_state.mint == mint_key, OracleError::InvalidMint);
    require!(
        channel_state.subject == subject_id,
        OracleError::InvalidChannelState
    );

    let slot_idx = ChannelState::slot_index(epoch);
    require!(
        channel_state.slots[slot_idx].epoch == epoch,
        OracleError::SlotMismatch
    );

    let leaf = compute_leaf(&ctx.accounts.owner.key(), index, amount, &id);
    require!(
        verify_proof(&proof, leaf, channel_state.slots[slot_idx].root),
        OracleError::InvalidProof
    );

    require!(amount >= min_attention, OracleError::InsufficientAttention);

    msg!(
        "Attention gate passed: owner={}, channel={}, epoch={}, amount={}, min={}",
        ctx.accounts.owner.key(),
        channel,
        epoch,
        amount,
        min_attention
    );

    Ok(())
}

/// Check token balance for post-claim gating.
#[derive(Accounts)]
pub struct RequireAttentionBalance<'info> {
    /// The wallet being checked
    /// CHECK: Used as ATA authority
    pub owner: UncheckedAccount<'info>,

    /// CCM mint
    pub mint: InterfaceAccount<'info, anchor_spl::token_interface::Mint>,

    /// Owner's CCM token account
    #[account(
        associated_token::mint = mint,
        associated_token::authority = owner,
        associated_token::token_program = token_program
    )]
    pub owner_ata: InterfaceAccount<'info, anchor_spl::token_interface::TokenAccount>,

    pub token_program: Interface<'info, anchor_spl::token_interface::TokenInterface>,
}

/// Verify token balance meets minimum threshold.
pub fn require_attention_balance_ge(
    ctx: Context<RequireAttentionBalance>,
    min_balance: u64,
) -> Result<()> {
    let balance = ctx.accounts.owner_ata.amount;
    require!(balance >= min_balance, OracleError::InsufficientAttention);

    msg!(
        "Balance gate passed: owner={}, balance={}, min={}",
        ctx.accounts.owner.key(),
        balance,
        min_balance
    );

    Ok(())
}
