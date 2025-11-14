use crate::{constants::PROTOCOL_SEED, errors::ProtocolError, state::ProtocolState};
use anchor_lang::prelude::*;

/// Update the allowlisted publisher (singleton protocol_state)
#[derive(Accounts)]
pub struct UpdatePublisher<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ ProtocolError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_publisher(ctx: Context<UpdatePublisher>, new_publisher: Pubkey) -> Result<()> {
    // Reject accidental zero key
    require!(new_publisher != Pubkey::default(), ProtocolError::InvalidPubkey);
    let state = &mut ctx.accounts.protocol_state;
    state.publisher = new_publisher;
    Ok(())
}

/// Update the allowlisted publisher (open variant keyed by mint)
#[derive(Accounts)]
pub struct UpdatePublisherOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ ProtocolError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_publisher_open(
    ctx: Context<UpdatePublisherOpen>,
    new_publisher: Pubkey,
) -> Result<()> {
    // Reject accidental zero key
    require!(new_publisher != Pubkey::default(), ProtocolError::InvalidPubkey);
    let state = &mut ctx.accounts.protocol_state;
    state.publisher = new_publisher;
    Ok(())
}

/// Set receipt requirement policy (singleton protocol_state)
#[derive(Accounts)]
pub struct SetPolicy<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ ProtocolError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_policy(ctx: Context<SetPolicy>, require_receipt: bool) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.require_receipt = require_receipt;
    Ok(())
}

/// Set receipt requirement policy (open variant keyed by mint)
#[derive(Accounts)]
pub struct SetPolicyOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ ProtocolError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_policy_open(ctx: Context<SetPolicyOpen>, require_receipt: bool) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.require_receipt = require_receipt;
    Ok(())
}

/// Protocol pause/unpause (singleton protocol_state)
#[derive(Accounts)]
pub struct SetPaused<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ ProtocolError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_paused(ctx: Context<SetPaused>, paused: bool) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.paused = paused;
    Ok(())
}

/// Protocol pause/unpause (open variant keyed by mint)
#[derive(Accounts)]
pub struct SetPausedOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ ProtocolError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_paused_open(ctx: Context<SetPausedOpen>, paused: bool) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.paused = paused;
    Ok(())
}

/// Transfer admin authority (open variant keyed by mint)
/// Used for migrating to hardware wallet or new admin key
#[derive(Accounts)]
pub struct UpdateAdminOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ ProtocolError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_admin_open(ctx: Context<UpdateAdminOpen>, new_admin: Pubkey) -> Result<()> {
    // Reject accidental zero key
    require!(new_admin != Pubkey::default(), ProtocolError::InvalidPubkey);
    let state = &mut ctx.accounts.protocol_state;
    state.admin = new_admin;
    Ok(())
}

/// Transfer admin authority (singleton variant)
#[derive(Accounts)]
pub struct UpdateAdmin<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ ProtocolError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_admin(ctx: Context<UpdateAdmin>, new_admin: Pubkey) -> Result<()> {
    // Reject accidental zero key
    require!(new_admin != Pubkey::default(), ProtocolError::InvalidPubkey);
    let state = &mut ctx.accounts.protocol_state;
    state.admin = new_admin;
    Ok(())
}

/// Close a ChannelState account and recover rent
/// This is used to clean up "ghost" accounts created with incorrect PDA derivations
/// or to retire old channel accounts that are no longer needed.
///
/// Security: Only the protocol admin (ProtocolState.admin) can close accounts.
#[derive(Accounts)]
pub struct CloseChannelState<'info> {
    /// Admin authority (must match ProtocolState.admin)
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Protocol state for authorization check
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = authority.key() == protocol_state.admin @ ProtocolError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel state account to close
    /// The close constraint transfers all lamports to rent_receiver and marks account as closed
    #[account(
        mut,
        close = rent_receiver,
    )]
    pub channel_state: AccountLoader<'info, crate::state::ChannelState>,

    /// Rent receiver (typically the authority, but can be specified)
    /// CHECK: Can be any account, recipient is chosen by admin
    #[account(mut)]
    pub rent_receiver: AccountInfo<'info>,
}

pub fn close_channel_state(ctx: Context<CloseChannelState>) -> Result<()> {
    let lamports = ctx.accounts.channel_state.to_account_info().lamports();

    msg!("Closing ChannelState account");
    msg!("  Account: {}", ctx.accounts.channel_state.key());
    msg!("  Rent recovered: {} lamports (~{} SOL)", lamports, lamports as f64 / 1_000_000_000.0);
    msg!("  Receiver: {}", ctx.accounts.rent_receiver.key());

    // Anchor's close constraint handles:
    // 1. Transfer all lamports to rent_receiver
    // 2. Zero out account data
    // 3. Set discriminator to CLOSED_ACCOUNT_DISCRIMINATOR

    Ok(())
}
