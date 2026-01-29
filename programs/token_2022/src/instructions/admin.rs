use crate::{
    constants::PROTOCOL_SEED,
    errors::OracleError,
    events::{AdminTransferred, ProtocolPaused, PublisherUpdated},
    state::ProtocolState,
};
use anchor_lang::prelude::*;

/// Update the allowlisted publisher (singleton protocol_state)
#[derive(Accounts)]
pub struct UpdatePublisher<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_publisher(ctx: Context<UpdatePublisher>, new_publisher: Pubkey) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    let old_publisher = state.publisher;
    state.publisher = new_publisher;

    emit!(PublisherUpdated {
        admin: ctx.accounts.admin.key(),
        old_publisher,
        new_publisher,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

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
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_publisher_open(
    ctx: Context<UpdatePublisherOpen>,
    new_publisher: Pubkey,
) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    let old_publisher = state.publisher;
    state.publisher = new_publisher;

    emit!(PublisherUpdated {
        admin: ctx.accounts.admin.key(),
        old_publisher,
        new_publisher,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Emergency pause/unpause (singleton protocol_state)
#[derive(Accounts)]
pub struct SetPaused<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_paused(ctx: Context<SetPaused>, paused: bool) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.paused = paused;

    emit!(ProtocolPaused {
        admin: ctx.accounts.admin.key(),
        paused,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

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
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_paused_open(ctx: Context<SetPausedOpen>, paused: bool) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.paused = paused;

    emit!(ProtocolPaused {
        admin: ctx.accounts.admin.key(),
        paused,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

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
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_admin_open(ctx: Context<UpdateAdminOpen>, new_admin: Pubkey) -> Result<()> {
    require!(new_admin != Pubkey::default(), OracleError::InvalidPubkey);
    let state = &mut ctx.accounts.protocol_state;
    let old_admin = state.admin;
    state.admin = new_admin;

    emit!(AdminTransferred {
        old_admin,
        new_admin,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Transfer admin authority (singleton variant)
#[derive(Accounts)]
pub struct UpdateAdmin<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_admin(ctx: Context<UpdateAdmin>, new_admin: Pubkey) -> Result<()> {
    require!(new_admin != Pubkey::default(), OracleError::InvalidPubkey);
    let state = &mut ctx.accounts.protocol_state;
    let old_admin = state.admin;
    state.admin = new_admin;

    emit!(AdminTransferred {
        old_admin,
        new_admin,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

// =============================================================================
// TREASURY WITHDRAW - REMOVED
// =============================================================================
// Admin withdrawal capability was removed after Initial Liquidity Offering.
// Treasury is now locked to claim-based distribution only.
// See: https://solscan.io/tx/L53wKdRPTYKCwR1DJJQjFr34SYsCzjqcyNgXP7BbZAV7Yasz7bDwqP2no6ozm7tLVMawUcADGhZPXRNe4wQajeh
// =============================================================================

// =============================================================================
// SET TREASURY (Fee Destination Owner)
// =============================================================================

#[event]
pub struct TreasuryUpdated {
    pub admin: Pubkey,
    pub old_treasury: Pubkey,
    pub new_treasury: Pubkey,
    pub mint: Pubkey,
    pub timestamp: i64,
}

/// Update the treasury wallet (fee destination owner).
/// The treasury field stores the OWNER of the fee destination token account.
/// harvest_fees will send withheld fees to ATA(treasury, mint).
#[derive(Accounts)]
pub struct SetTreasury<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_treasury(ctx: Context<SetTreasury>, new_treasury: Pubkey) -> Result<()> {
    require!(new_treasury != Pubkey::default(), OracleError::InvalidPubkey);
    let state = &mut ctx.accounts.protocol_state;
    let old_treasury = state.treasury;
    state.treasury = new_treasury;

    emit!(TreasuryUpdated {
        admin: ctx.accounts.admin.key(),
        old_treasury,
        new_treasury,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!("Treasury updated: {} -> {}", old_treasury, new_treasury);

    Ok(())
}
