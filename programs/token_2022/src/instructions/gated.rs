use anchor_lang::prelude::*;

use crate::constants::CHANNEL_STATE_SEED;
use crate::errors::OracleError;
use crate::instructions::claim::{compute_leaf, verify_proof};
use crate::state::ChannelState;
use anchor_lang::accounts::account_loader::AccountLoader;

use sha3::{Digest, Keccak256};

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

// ============================================================================
// REQUIRE ATTENTION (Gated CPI Check)
// ============================================================================

/// Accounts required for attention-gated actions.
/// This is a read-only check that verifies a merkle proof and attention threshold.
/// Used by external programs (e.g., pump.fun SDK) to gate actions based on attention.
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

/// Verify that `owner` has at least `min_attention` in the specified channel/epoch.
///
/// This instruction does NOT claim or transfer tokens - it only verifies eligibility.
/// External programs can CPI into this to gate actions (e.g., pump.fun buys).
///
/// # Arguments
/// * `channel` - Channel name (e.g., "pump.fun")
/// * `epoch` - Epoch number to verify against
/// * `index` - Leaf index in the merkle tree
/// * `amount` - Attention amount in the merkle leaf
/// * `id` - User identifier used in leaf computation
/// * `proof` - Merkle proof nodes
/// * `min_attention` - Minimum attention threshold required (micro-tokens)
///
/// # Errors
/// * `InsufficientAttention` - If amount < min_attention
/// * `InvalidProof` - If merkle proof verification fails
/// * `SlotMismatch` - If epoch is not in the ring buffer
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
    // Derive subject_id from channel name
    let subject_id = derive_subject_id(&channel);

    // Validate channel_state PDA
    let mint_key = ctx.accounts.mint.key();
    let seeds = [CHANNEL_STATE_SEED, mint_key.as_ref(), subject_id.as_ref()];
    let (expected_pda, _) = Pubkey::find_program_address(&seeds, ctx.program_id);
    require_keys_eq!(
        expected_pda,
        ctx.accounts.channel_state.to_account_info().key(),
        OracleError::InvalidChannelState
    );

    // Load channel state (read-only)
    let channel_state = ctx.accounts.channel_state.load()?;

    // Validate mint matches
    require!(channel_state.mint == mint_key, OracleError::InvalidMint);
    require!(
        channel_state.subject == subject_id,
        OracleError::InvalidChannelState
    );

    // Find the slot for this epoch
    let slot_idx = ChannelState::slot_index(epoch);
    require!(
        channel_state.slots[slot_idx].epoch == epoch,
        OracleError::SlotMismatch
    );

    // Verify merkle proof
    let leaf = compute_leaf(&ctx.accounts.owner.key(), index, amount, &id);
    require!(
        verify_proof(&proof, leaf, channel_state.slots[slot_idx].root),
        OracleError::InvalidProof
    );

    // Check attention threshold
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

/// Simple attention balance check without merkle proof.
/// Requires the user to have already claimed and holds CCM tokens.
/// This is a lighter-weight check for post-claim verification.
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

/// Verify that `owner` holds at least `min_balance` CCM tokens.
///
/// This is a simpler check that doesn't require merkle proofs - it just checks
/// the user's token balance. Use this for post-claim gating where users have
/// already claimed their attention tokens.
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
