#![allow(clippy::too_many_arguments)]

//! # ChannelVault
//!
//! Liquid staking wrapper for Attention Oracle channel stakes.
//! Mints vLOFI (standard SPL) tokens in exchange for staked CCM (Token-2022).

use anchor_lang::prelude::*;

#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;

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

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "Channel Vault",
    project_url: "https://github.com/twzrd-sol/attention-oracle-program",
    contacts: "email:security@twzrd.xyz",
    policy: "https://github.com/twzrd-sol/attention-oracle-program/blob/main/SECURITY.md",
    preferred_languages: "en",
    source_code: "https://github.com/twzrd-sol/attention-oracle-program"
}

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
    /// min_ccm_amount: slippage protection - reverts if actual received < min
    pub fn complete_withdraw(
        ctx: Context<CompleteWithdraw>,
        request_id: u64,
        min_ccm_amount: u64,
    ) -> Result<()> {
        instructions::redeem::complete_withdraw(ctx, request_id, min_ccm_amount)
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

    /// Emergency withdrawal when Oracle is unresponsive (no compound in 7+ days).
    ///
    /// For users with pending WithdrawRequests who cannot complete normally
    /// because the Oracle is dead/frozen. Pays from buffer with 20% penalty.
    /// This is a last-resort exit that requires no Oracle cooperation.
    pub fn emergency_timeout_withdraw(
        ctx: Context<EmergencyTimeoutWithdraw>,
        request_id: u64,
        min_ccm_amount: u64,
    ) -> Result<()> {
        instructions::redeem::emergency_timeout_withdraw(ctx, request_id, min_ccm_amount)
    }

    // -------------------------------------------------------------------------
    // Permissionless Operations
    // -------------------------------------------------------------------------

    /// Compound pending deposits into Oracle stake.
    /// Anyone can call this (keeper incentive).
    pub fn compound<'info>(
        ctx: Context<'_, '_, 'info, 'info, Compound<'info>>,
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

    /// Update withdrawal queue duration in slots (admin only).
    pub fn update_withdraw_queue_slots(
        ctx: Context<AdminAction>,
        new_withdraw_queue_slots: u64,
    ) -> Result<()> {
        instructions::admin::update_withdraw_queue_slots(ctx, new_withdraw_queue_slots)
    }

    /// Update lock duration in slots (admin only).
    pub fn update_lock_duration_slots(
        ctx: Context<AdminAction>,
        new_lock_duration_slots: u64,
    ) -> Result<()> {
        instructions::admin::update_lock_duration_slots(ctx, new_lock_duration_slots)
    }

    /// Inject capital to recover from insolvency (admin only).
    ///
    /// Use when pending_withdrawals > assets, making the vault unable to honor
    /// withdrawal requests. Admin transfers CCM from their account to restore solvency.
    /// Injected funds go to pending_deposits and will be staked on next compound.
    pub fn inject_capital(ctx: Context<InjectCapital>, amount: u64) -> Result<()> {
        instructions::admin::inject_capital(ctx, amount)
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

    /// Migrate oracle position for vaults deployed before oracle tracking.
    /// Admin-only, idempotent. After migration, use sync_oracle_position
    /// to reconcile with on-chain Oracle state.
    pub fn migrate_oracle_position(ctx: Context<MigrateOraclePosition>) -> Result<()> {
        instructions::migrate_oracle_position::handler(ctx)
    }

    // -------------------------------------------------------------------------
    // Exchange Rate Oracle
    // -------------------------------------------------------------------------

    /// Initialize the exchange rate oracle PDA for a vault.
    /// One-time setup, admin-only. After init, compound() updates it automatically.
    pub fn initialize_exchange_rate(ctx: Context<InitializeExchangeRate>) -> Result<()> {
        instructions::exchange_rate::initialize_exchange_rate(ctx)
    }
}
