use crate::{constants::PROTOCOL_SEED, errors::MiloError, state::ProtocolState};
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
        constraint = admin.key() == protocol_state.admin @ MiloError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_publisher(ctx: Context<UpdatePublisher>, new_publisher: Pubkey) -> Result<()> {
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
        constraint = admin.key() == protocol_state.admin @ MiloError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_publisher_open(
    ctx: Context<UpdatePublisherOpen>,
    new_publisher: Pubkey,
) -> Result<()> {
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
        constraint = admin.key() == protocol_state.admin @ MiloError::Unauthorized,
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
        constraint = admin.key() == protocol_state.admin @ MiloError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_policy_open(ctx: Context<SetPolicyOpen>, require_receipt: bool) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.require_receipt = require_receipt;
    Ok(())
}

/// Emergency pause/unpause (singleton protocol_state)
#[derive(Accounts)]
pub struct SetPaused<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ MiloError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_paused(ctx: Context<SetPaused>, paused: bool) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.paused = paused;
    Ok(())
}

/// Emergency pause/unpause (open variant keyed by mint)
#[derive(Accounts)]
pub struct SetPausedOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ MiloError::Unauthorized,
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
        constraint = admin.key() == protocol_state.admin @ MiloError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_admin_open(ctx: Context<UpdateAdminOpen>, new_admin: Pubkey) -> Result<()> {
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
        constraint = admin.key() == protocol_state.admin @ MiloError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_admin(ctx: Context<UpdateAdmin>, new_admin: Pubkey) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.admin = new_admin;
    Ok(())
}
