use crate::{
    constants::{ADMIN_AUTHORITY, CHANNEL_STATE_SEED, EPOCH_STATE_SEED, PROTOCOL_SEED},
    errors::OracleError,
    state::{ChannelState, EpochState, ProtocolState},
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(epoch: u64, subject_id: Pubkey)]
pub struct CloseEpochState<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// Global protocol state - verify admin authority
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Epoch state to close
    #[account(
        mut,
        seeds = [EPOCH_STATE_SEED, &epoch.to_le_bytes(), subject_id.as_ref()],
        bump,
        close = admin
    )]
    pub epoch_state: Account<'info, EpochState>,
}

pub fn close_epoch_state(
    ctx: Context<CloseEpochState>,
    _epoch: u64,
    _subject_id: Pubkey,
) -> Result<()> {
    // Safety: only allow closing when all claims are completed
    let es = &ctx.accounts.epoch_state;
    require!(all_claims_completed(es), OracleError::EpochNotFullyClaimed);
    // Account will be closed automatically by Anchor's `close = admin` constraint
    // This recovers the rent to the admin account
    msg!(
        "Closed epoch_state for epoch {} subject {}",
        _epoch,
        _subject_id
    );
    Ok(())
}

/// Open variant for permissionless protocol keyed by mint
#[derive(Accounts)]
#[instruction(epoch: u64, subject_id: Pubkey)]
pub struct CloseEpochStateOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// Global protocol state keyed by mint
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Epoch state to close (includes mint in seeds)
    #[account(
        mut,
        seeds = [EPOCH_STATE_SEED, &epoch.to_le_bytes(), subject_id.as_ref(), protocol_state.mint.as_ref()],
        bump,
        close = admin
    )]
    pub epoch_state: Account<'info, EpochState>,
}

pub fn close_epoch_state_open(
    ctx: Context<CloseEpochStateOpen>,
    _epoch: u64,
    _subject_id: Pubkey,
) -> Result<()> {
    let es = &ctx.accounts.epoch_state;
    require!(all_claims_completed(es), OracleError::EpochNotFullyClaimed);
    msg!(
        "Closed epoch_state_open for epoch {} subject {}",
        _epoch,
        _subject_id
    );
    Ok(())
}

/// Emergency path for legacy PDAs created before ProtocolState existed.
/// Admin is hard-gated to a compile-time constant and epoch_state seeds are checked.
#[derive(Accounts)]
#[instruction(epoch: u64, subject_id: Pubkey)]
pub struct ForceCloseEpochStateLegacy<'info> {
    /// Emergency admin signer
    #[account(mut)]
    pub admin: Signer<'info>,

    /// Target epoch state (legacy: no mint in seeds)
    #[account(
        mut,
        seeds = [EPOCH_STATE_SEED, &epoch.to_le_bytes(), subject_id.as_ref()],
        bump,
        close = admin
    )]
    pub epoch_state: Account<'info, EpochState>,
}

/// Emergency path (open) with mint in seeds but without ProtocolState requirement.
#[derive(Accounts)]
#[instruction(epoch: u64, subject_id: Pubkey, mint: Pubkey)]
pub struct ForceCloseEpochStateOpen<'info> {
    /// Emergency admin signer
    #[account(mut)]
    pub admin: Signer<'info>,

    /// Target epoch state (open: includes mint in seeds)
    #[account(
        mut,
        seeds = [EPOCH_STATE_SEED, &epoch.to_le_bytes(), subject_id.as_ref(), mint.as_ref()],
        bump,
        close = admin
    )]
    pub epoch_state: Account<'info, EpochState>,
}

pub fn force_close_epoch_state_legacy(
    ctx: Context<ForceCloseEpochStateLegacy>,
    _epoch: u64,
    _subject_id: Pubkey,
) -> Result<()> {
    require_keys_eq!(
        ctx.accounts.admin.key(),
        ADMIN_AUTHORITY,
        OracleError::Unauthorized
    );
    // Timelock to reduce risk of premature close
    let now = Clock::get()?.unix_timestamp;
    require!(
        now - ctx.accounts.epoch_state.timestamp >= crate::constants::EPOCH_FORCE_CLOSE_GRACE_SECS,
        OracleError::EpochClosed
    );
    Ok(())
}

pub fn force_close_epoch_state_open(
    ctx: Context<ForceCloseEpochStateOpen>,
    _epoch: u64,
    _subject_id: Pubkey,
    _mint: Pubkey,
) -> Result<()> {
    require_keys_eq!(
        ctx.accounts.admin.key(),
        ADMIN_AUTHORITY,
        OracleError::Unauthorized
    );
    let now = Clock::get()?.unix_timestamp;
    require!(
        now - ctx.accounts.epoch_state.timestamp >= crate::constants::EPOCH_FORCE_CLOSE_GRACE_SECS,
        OracleError::EpochClosed
    );
    Ok(())
}

fn all_claims_completed(es: &EpochState) -> bool {
    // Count set bits up to claim_count
    let mut remaining = es.claim_count as usize;
    let mut idx = 0usize;
    while remaining > 0 {
        let byte_i = idx / 8;
        let bit_in_byte = idx % 8;
        let mask = 1u8 << bit_in_byte;
        let is_set = (es.claimed_bitmap.get(byte_i).copied().unwrap_or(0) & mask) != 0;
        if !is_set {
            return false;
        }
        idx += 1;
        remaining = remaining.saturating_sub(1);
    }
    true
}

// =============================================================================
// Channel State Cleanup
// =============================================================================

/// Close a channel state account (with ProtocolState admin auth)
#[derive(Accounts)]
#[instruction(subject_id: Pubkey)]
pub struct CloseChannelState<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// Protocol state to verify admin authority
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel state to close - using AccountLoader for zero_copy
    /// CHECK: Seeds validated, lamports transferred manually
    #[account(
        mut,
        seeds = [CHANNEL_STATE_SEED, protocol_state.mint.as_ref(), subject_id.as_ref()],
        bump,
    )]
    pub channel_state: AccountLoader<'info, ChannelState>,

    pub system_program: Program<'info, System>,
}

pub fn close_channel_state(ctx: Context<CloseChannelState>, subject_id: Pubkey) -> Result<()> {
    let channel_info = ctx.accounts.channel_state.to_account_info();
    let admin_info = ctx.accounts.admin.to_account_info();

    // Transfer all lamports to admin
    let lamports = channel_info.lamports();
    **channel_info.try_borrow_mut_lamports()? = 0;
    **admin_info.try_borrow_mut_lamports()? = admin_info
        .lamports()
        .checked_add(lamports)
        .ok_or(OracleError::MathOverflow)?;

    // Zero out account data to mark as closed
    channel_info.assign(&System::id());
    channel_info.resize(0, false)?;

    msg!("Closed channel_state for subject {}", subject_id);
    Ok(())
}

/// Force close legacy channel state (pre-ProtocolState, hardcoded admin)
#[derive(Accounts)]
#[instruction(subject_id: Pubkey, mint: Pubkey)]
pub struct ForceCloseChannelStateLegacy<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// Channel state to close (legacy: mint in seeds but no ProtocolState check)
    /// CHECK: Seeds validated, lamports transferred manually
    #[account(
        mut,
        seeds = [CHANNEL_STATE_SEED, mint.as_ref(), subject_id.as_ref()],
        bump,
    )]
    pub channel_state: AccountLoader<'info, ChannelState>,

    pub system_program: Program<'info, System>,
}

pub fn force_close_channel_state_legacy(
    ctx: Context<ForceCloseChannelStateLegacy>,
    subject_id: Pubkey,
    _mint: Pubkey,
) -> Result<()> {
    // Only hardcoded admin can force close
    require_keys_eq!(
        ctx.accounts.admin.key(),
        ADMIN_AUTHORITY,
        OracleError::Unauthorized
    );

    let channel_info = ctx.accounts.channel_state.to_account_info();
    let admin_info = ctx.accounts.admin.to_account_info();

    // Transfer all lamports to admin
    let lamports = channel_info.lamports();
    **channel_info.try_borrow_mut_lamports()? = 0;
    **admin_info.try_borrow_mut_lamports()? = admin_info
        .lamports()
        .checked_add(lamports)
        .ok_or(OracleError::MathOverflow)?;

    // Zero out account data
    channel_info.assign(&System::id());
    channel_info.resize(0, false)?;

    msg!("Force closed legacy channel_state for subject {}", subject_id);
    Ok(())
}
