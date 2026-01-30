//! Admin instructions for vault management.

use anchor_lang::prelude::*;

use crate::constants::{VAULT_ORACLE_POSITION_SEED, VAULT_SEED};
use crate::errors::VaultError;
use crate::events::{AdminUpdated, OraclePositionSynced, VaultPaused, VaultResumed};
use crate::state::{ChannelVault, VaultOraclePosition};

use token_2022::UserChannelStake;

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
    require!(
        new_admin != Pubkey::default(),
        VaultError::InvalidPubkey
    );

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

// ---------------------------------------------------------------------------
// Sync Oracle Position
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct SyncOraclePosition<'info> {
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

    #[account(
        mut,
        seeds = [VAULT_ORACLE_POSITION_SEED, vault.key().as_ref()],
        bump = vault_oracle_position.bump,
    )]
    pub vault_oracle_position: Account<'info, VaultOraclePosition>,

    /// Oracle UserChannelStake for this vault.
    #[account(
        constraint = oracle_user_stake.user == vault.key() @ VaultError::InvalidOracleAccount,
        constraint = oracle_user_stake.channel == vault.channel_config @ VaultError::InvalidOracleAccount,
    )]
    pub oracle_user_stake: Account<'info, UserChannelStake>,
}

/// Sync vault oracle position state from the Oracle's UserChannelStake.
/// Fixes drift between local VaultOraclePosition and on-chain Oracle state.
pub fn sync_oracle_position(ctx: Context<SyncOraclePosition>) -> Result<()> {
    let position = &mut ctx.accounts.vault_oracle_position;
    let stake = &ctx.accounts.oracle_user_stake;

    position.is_active = stake.amount > 0;
    position.stake_amount = stake.amount;
    position.lock_end_slot = stake.lock_end_slot;
    position.oracle_nft_mint = stake.nft_mint;
    position.oracle_user_stake = ctx.accounts.oracle_user_stake.key();

    // Correct vault.total_staked to match Oracle truth
    let vault = &mut ctx.accounts.vault;
    vault.total_staked = stake.amount;

    emit!(OraclePositionSynced {
        vault: vault.key(),
        admin: ctx.accounts.admin.key(),
        is_active: position.is_active,
        stake_amount: position.stake_amount,
        lock_end_slot: position.lock_end_slot,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "Oracle position synced: active={}, amount={}, total_staked corrected",
        position.is_active,
        position.stake_amount
    );
    Ok(())
}
