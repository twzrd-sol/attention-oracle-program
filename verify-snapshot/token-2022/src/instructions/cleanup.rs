use crate::{
    constants::{EPOCH_STATE_SEED, PROTOCOL_SEED},
    errors::ProtocolError,
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
        constraint = admin.key() == protocol_state.admin @ ProtocolError::Unauthorized,
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
    epoch: u64,
    streamer_key: Pubkey,
) -> Result<()> {
    let es = &ctx.accounts.epoch_state;
    let now = Clock::get()?.unix_timestamp;
    require!(
        now - es.timestamp >= crate::constants::EPOCH_FORCE_CLOSE_GRACE_SECS,
        ProtocolError::EpochNotExpired
    );
    msg!("Closed epoch_state for epoch {} streamer {}", epoch, streamer_key);
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
        constraint = admin.key() == protocol_state.admin @ ProtocolError::Unauthorized,
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
    epoch: u64,
    streamer_key: Pubkey,
) -> Result<()> {
    let es = &ctx.accounts.epoch_state;
    let now = Clock::get()?.unix_timestamp;
    require!(
        now - es.timestamp >= crate::constants::EPOCH_FORCE_CLOSE_GRACE_SECS,
        ProtocolError::EpochNotExpired
    );
    msg!("Closed epoch_state_open for epoch {} streamer {}", epoch, streamer_key);
    Ok(())
}

// emergency and legacy force-close paths removed for v1
