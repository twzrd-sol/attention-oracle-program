use crate::{
    constants::{EPOCH_STATE_SEED, PROTOCOL_SEED},
    errors::OracleError,
    state::{EpochState, ProtocolState},
};
use anchor_lang::prelude::*;
use std::str::FromStr;

#[derive(Accounts)]
#[instruction(epoch: u64, streamer_key: Pubkey)]
pub struct CloseEpochState<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// Global protocol state - verify admin authority
    #[account(
        seeds = [PROTOCOL_SEED],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Epoch state to close
    #[account(
        mut,
        seeds = [EPOCH_STATE_SEED, &epoch.to_le_bytes(), streamer_key.as_ref()],
        bump,
        close = admin
    )]
    pub epoch_state: Account<'info, EpochState>,
}

pub fn close_epoch_state(
    ctx: Context<CloseEpochState>,
    _epoch: u64,
    _streamer_key: Pubkey,
) -> Result<()> {
    // Safety: only allow closing when all claims are completed
    let es = &ctx.accounts.epoch_state;
    require!(all_claims_completed(es), OracleError::EpochNotFullyClaimed);
    // Account will be closed automatically by Anchor's `close = admin` constraint
    // This recovers the rent to the admin account
    msg!(
        "Closed epoch_state for epoch {} streamer {}",
        _epoch,
        _streamer_key
    );
    Ok(())
}

/// Open variant for permissionless protocol keyed by mint
#[derive(Accounts)]
#[instruction(epoch: u64, streamer_key: Pubkey)]
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
        seeds = [EPOCH_STATE_SEED, &epoch.to_le_bytes(), streamer_key.as_ref(), protocol_state.mint.as_ref()],
        bump,
        close = admin
    )]
    pub epoch_state: Account<'info, EpochState>,
}

pub fn close_epoch_state_open(
    ctx: Context<CloseEpochStateOpen>,
    _epoch: u64,
    _streamer_key: Pubkey,
) -> Result<()> {
    let es = &ctx.accounts.epoch_state;
    require!(all_claims_completed(es), OracleError::EpochNotFullyClaimed);
    msg!(
        "Closed epoch_state_open for epoch {} streamer {}",
        _epoch,
        _streamer_key
    );
    Ok(())
}

/// Emergency path for legacy PDAs created before ProtocolState existed.
/// Admin is hard-gated to a compile-time constant and epoch_state seeds are checked.
#[derive(Accounts)]
#[instruction(epoch: u64, streamer_key: Pubkey)]
pub struct ForceCloseEpochStateLegacy<'info> {
    /// Emergency admin signer
    #[account(mut)]
    pub admin: Signer<'info>,

    /// Target epoch state (legacy: no mint in seeds)
    #[account(
        mut,
        seeds = [EPOCH_STATE_SEED, &epoch.to_le_bytes(), streamer_key.as_ref()],
        bump,
        close = admin
    )]
    pub epoch_state: Account<'info, EpochState>,
}

/// Emergency path (open) with mint in seeds but without ProtocolState requirement.
#[derive(Accounts)]
#[instruction(epoch: u64, streamer_key: Pubkey, mint: Pubkey)]
pub struct ForceCloseEpochStateOpen<'info> {
    /// Emergency admin signer
    #[account(mut)]
    pub admin: Signer<'info>,

    /// Target epoch state (open: includes mint in seeds)
    #[account(
        mut,
        seeds = [EPOCH_STATE_SEED, &epoch.to_le_bytes(), streamer_key.as_ref(), mint.as_ref()],
        bump,
        close = admin
    )]
    pub epoch_state: Account<'info, EpochState>,
}

// Emergency admin (compile-time)
const EMERGENCY_ADMIN_STR: &str = "AmMftc4zHgR4yYfv29awV9Q46emo2aGPFW8utP81CsBv";

pub fn force_close_epoch_state_legacy(
    ctx: Context<ForceCloseEpochStateLegacy>,
    _epoch: u64,
    _streamer_key: Pubkey,
) -> Result<()> {
    let emergency =
        Pubkey::from_str(EMERGENCY_ADMIN_STR).map_err(|_| error!(OracleError::Unauthorized))?;
    require_keys_eq!(ctx.accounts.admin.key(), emergency, OracleError::Unauthorized);
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
    _streamer_key: Pubkey,
    _mint: Pubkey,
) -> Result<()> {
    let emergency =
        Pubkey::from_str(EMERGENCY_ADMIN_STR).map_err(|_| error!(OracleError::Unauthorized))?;
    require_keys_eq!(ctx.accounts.admin.key(), emergency, OracleError::Unauthorized);
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
