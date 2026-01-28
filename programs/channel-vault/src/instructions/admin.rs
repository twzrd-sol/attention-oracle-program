//! Admin instructions for vault management.

use anchor_lang::prelude::*;

use crate::constants::VAULT_SEED;
use crate::errors::VaultError;
use crate::events::{AdminUpdated, VaultPaused, VaultResumed};
use crate::state::ChannelVault;

#[derive(Accounts)]
pub struct AdminAction<'info> {
    #[account(
        constraint = admin.key() == vault.admin @ VaultError::Unauthorized
    )]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [VAULT_SEED, vault.channel_config.as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, ChannelVault>,
}

/// Pause the vault (stops deposits).
pub fn pause(ctx: Context<AdminAction>) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    vault.paused = true;

    emit!(VaultPaused {
        vault: vault.key(),
        admin: ctx.accounts.admin.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!("Vault paused");
    Ok(())
}

/// Resume the vault.
pub fn resume(ctx: Context<AdminAction>) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    vault.paused = false;

    emit!(VaultResumed {
        vault: vault.key(),
        admin: ctx.accounts.admin.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!("Vault resumed");
    Ok(())
}

/// Update admin authority.
pub fn update_admin(ctx: Context<AdminAction>, new_admin: Pubkey) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    let old_admin = vault.admin;
    vault.admin = new_admin;

    emit!(AdminUpdated {
        vault: vault.key(),
        old_admin,
        new_admin,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!("Admin updated: {} -> {}", old_admin, new_admin);
    Ok(())
}
