//! Migration instruction to add VaultOraclePosition to existing vaults.
//!
//! Context: VaultOraclePosition was added in commit 7983b67 (Jan 30, 2026)
//! AFTER the 16 mainnet vaults were deployed. This instruction allows
//! retroactive initialization of oracle position accounts.

use anchor_lang::prelude::*;

use crate::constants::{VAULT_ORACLE_POSITION_SEED, VAULT_SEED};
use crate::errors::VaultError;
use crate::events::OraclePositionMigrated;
use crate::state::{ChannelVault, VaultOraclePosition};

#[derive(Accounts)]
pub struct MigrateOraclePosition<'info> {
    /// Vault admin must sign (prevents unauthorized migration).
    #[account(
        mut,
        constraint = admin.key() == vault.admin @ VaultError::Unauthorized
    )]
    pub admin: Signer<'info>,

    /// Vault account (must already exist).
    #[account(
        mut,
        seeds = [VAULT_SEED, vault.channel_config.as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, ChannelVault>,

    /// Oracle position account to initialize.
    /// CRITICAL: This account must NOT exist yet (init will fail if it does).
    #[account(
        init,
        payer = admin,
        space = VaultOraclePosition::LEN,
        seeds = [VAULT_ORACLE_POSITION_SEED, vault.key().as_ref()],
        bump
    )]
    pub vault_oracle_position: Account<'info, VaultOraclePosition>,

    pub system_program: Program<'info, System>,
}

/// Initialize VaultOraclePosition for vaults deployed before oracle tracking.
///
/// Safety:
///   - Only callable by vault admin
///   - Idempotent (fails gracefully if position already exists)
///   - Sets is_active=false, stake_amount=0 (vault.total_staked remains source of truth)
///
/// After migration, use sync_oracle_position to reconcile with on-chain Oracle state.
pub fn handler(ctx: Context<MigrateOraclePosition>) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let position = &mut ctx.accounts.vault_oracle_position;

    // Initialize with safe defaults
    position.vault = vault.key();
    position.is_active = false;
    position.stake_amount = 0;
    position.lock_end_slot = 0;
    position.oracle_nft_mint = Pubkey::default();
    position.oracle_user_stake = Pubkey::default();
    position.bump = ctx.bumps.vault_oracle_position;

    emit!(OraclePositionMigrated {
        vault: vault.key(),
        admin: ctx.accounts.admin.key(),
        oracle_position: position.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "Oracle position migrated for vault {} (channel: {})",
        vault.key(),
        vault.channel_config
    );

    Ok(())
}
