#![allow(clippy::too_many_arguments)]

//! # ChannelVault
//!
//! Liquid staking wrapper for Attention Oracle channel stakes.
//! Mints vLOFI (standard SPL) tokens in exchange for staked CCM (Token-2022).

use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

pub use constants::*;
pub use errors::*;
pub use events::*;
pub use instructions::*;
pub use state::*;

declare_id!("5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ");

#[program]
pub mod channel_vault {
    use super::*;

    // -------------------------------------------------------------------------
    // Vault Lifecycle
    // -------------------------------------------------------------------------

    /// Initialize a new vault for a specific channel.
    pub fn initialize_vault(
        ctx: Context<InitializeVault>,
        min_deposit: u64,
        lock_duration_slots: u64,
        withdraw_queue_slots: u64,
    ) -> Result<()> {
        instructions::initialize::handler(ctx, min_deposit, lock_duration_slots, withdraw_queue_slots)
    }

    // -------------------------------------------------------------------------
    // User Actions
    // -------------------------------------------------------------------------

    /// Deposit CCM and receive vLOFI shares.
    pub fn deposit(
        ctx: Context<Deposit>,
        amount: u64,
        min_shares: u64,
    ) -> Result<()> {
        instructions::deposit::handler(ctx, amount, min_shares)
    }

    /// Request withdrawal - burns vLOFI, starts withdrawal queue.
    pub fn request_withdraw(
        ctx: Context<RequestWithdraw>,
        shares: u64,
        min_amount: u64,
    ) -> Result<()> {
        instructions::redeem::request_withdraw(ctx, shares, min_amount)
    }

    /// Complete withdrawal after queue period.
    pub fn complete_withdraw(ctx: Context<CompleteWithdraw>) -> Result<()> {
        instructions::redeem::complete_withdraw(ctx)
    }

    /// Instant redemption - exit with 20% penalty, from buffer/reserve.
    /// Only available when Oracle stake is locked.
    /// Does NOT touch Oracle - draws from vault's liquidity buffer.
    pub fn instant_redeem(ctx: Context<InstantRedeem>, shares: u64, min_amount: u64) -> Result<()> {
        instructions::redeem::instant_redeem(ctx, shares, min_amount)
    }

    /// ADMIN ONLY: Emergency unstake from Oracle with 20% penalty.
    /// WARNING: Affects ALL shareholders. Only for catastrophic scenarios.
    pub fn admin_emergency_unstake<'info>(
        ctx: Context<'_, '_, '_, 'info, AdminEmergencyUnstake<'info>>,
    ) -> Result<()> {
        instructions::redeem::admin_emergency_unstake(ctx)
    }

    // -------------------------------------------------------------------------
    // Permissionless Operations
    // -------------------------------------------------------------------------

    /// Compound pending deposits into Oracle stake.
    /// Anyone can call this (keeper incentive).
    pub fn compound<'info>(
        ctx: Context<'_, '_, '_, 'info, Compound<'info>>,
    ) -> Result<()> {
        instructions::compound::handler(ctx)
    }

    // -------------------------------------------------------------------------
    // Admin
    // -------------------------------------------------------------------------

    /// Pause the vault (admin only).
    pub fn pause(ctx: Context<AdminAction>) -> Result<()> {
        instructions::admin::pause(ctx)
    }

    /// Resume the vault (admin only).
    pub fn resume(ctx: Context<AdminAction>) -> Result<()> {
        instructions::admin::resume(ctx)
    }

    /// Update admin authority.
    pub fn update_admin(ctx: Context<AdminAction>, new_admin: Pubkey) -> Result<()> {
        instructions::admin::update_admin(ctx, new_admin)
    }

    /// Sync oracle position state from on-chain UserChannelStake.
    /// Fixes drift between local VaultOraclePosition and Oracle state.
    pub fn sync_oracle_position(ctx: Context<SyncOraclePosition>) -> Result<()> {
        instructions::admin::sync_oracle_position(ctx)
    }

    /// Close an empty vault and reclaim rent.
    /// Only callable when vault has no shares, deposits, or active positions.
    pub fn close_vault(ctx: Context<CloseVault>) -> Result<()> {
        instructions::close::handler(ctx)
    }

    /// Set vLOFI token metadata (name, symbol, URI) via Metaplex.
    /// Creates metadata on first call, updates on subsequent calls.
    pub fn set_vlofi_metadata(
        ctx: Context<SetVlofiMetadata>,
        name: String,
        symbol: String,
        uri: String,
    ) -> Result<()> {
        instructions::metadata::handler(ctx, name, symbol, uri)
    }
}
