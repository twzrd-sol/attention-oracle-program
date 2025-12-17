use crate::{
    constants::{CHANNEL_STATE_SEED, PROTOCOL_SEED},
    errors::OracleError,
    state::{ChannelState, ProtocolState},
};
use anchor_lang::accounts::account_loader::AccountLoader;
use anchor_lang::prelude::*;

#[cfg(feature = "legacy")]
use crate::constants::ADMIN_AUTHORITY;

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
    channel_info.resize(0)?;

    msg!("Closed channel_state for subject {}", subject_id);
    Ok(())
}

/// Force close legacy channel state (pre-ProtocolState, hardcoded admin)
#[cfg(feature = "legacy")]
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

#[cfg(feature = "legacy")]
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
    channel_info.resize(0)?;

    msg!("Force closed legacy channel_state for subject {}", subject_id);
    Ok(())
}
