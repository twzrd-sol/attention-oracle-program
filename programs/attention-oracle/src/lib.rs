#![allow(ambiguous_glob_reexports)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::too_many_arguments, clippy::missing_errors_doc)]
#![allow(
    clippy::doc_markdown,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value
)]
#![allow(clippy::manual_div_ceil)]
#![allow(
    clippy::items_after_statements,
    clippy::needless_for_each,
    clippy::needless_borrow
)]
#![allow(
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    clippy::uninlined_format_args
)]
#![allow(
    clippy::or_fun_call,
    clippy::explicit_iter_loop,
    clippy::used_underscore_binding
)]
#![allow(
    clippy::needless_borrows_for_generic_args,
    clippy::cast_possible_truncation,
    clippy::cast_lossless
)]
#![allow(clippy::no_effect_underscore_binding, clippy::pub_underscore_fields)]
#![allow(
    clippy::too_long_first_doc_paragraph,
    clippy::unnecessary_cast,
    clippy::len_zero
)]
#![allow(
    clippy::wildcard_imports,
    clippy::missing_const_for_fn,
    clippy::use_self
)]

//! # Liquid Attention Protocol
//!
//! Permissionless attention markets on Solana.
//! DEPOSIT (USDC) → MINT (vLOFI) → MATURE (attention accrual) → RESOLVE → SETTLE (CCM)

use anchor_lang::prelude::*;

#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
#[cfg(feature = "strategy")]
pub mod klend;
pub mod merkle_proof;
pub mod state;
pub mod token_transfer;

pub use constants::*;
pub use errors::*;
pub use events::*;
pub use instructions::*;
pub use merkle_proof::*;
pub use state::*;
pub use token_transfer::*;

declare_id!("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "Liquid Attention Protocol",
    project_url: "https://github.com/twzrd-sol/wzrd-final",
    contacts: "email:security@twzrd.xyz",
    policy: "https://github.com/twzrd-sol/wzrd-final/blob/main/SECURITY.md",
    preferred_languages: "en",
    source_code: "https://github.com/twzrd-sol/wzrd-final"
}

#[program]
pub mod token_2022 {
    use super::*;

    // =========================================================================
    // Global Root (V4) — Oracle Heartbeat
    // Single root shared across all markets. Resolves attention data.
    // =========================================================================

    pub fn initialize_global_root(ctx: Context<InitializeGlobalRoot>) -> Result<()> {
        instructions::global::initialize_global_root(ctx)
    }

    pub fn publish_global_root(
        ctx: Context<PublishGlobalRoot>,
        root_seq: u64,
        root: [u8; 32],
        dataset_hash: [u8; 32],
    ) -> Result<()> {
        instructions::global::publish_global_root(ctx, root_seq, root, dataset_hash)
    }

    pub fn claim_global<'info>(
        ctx: Context<'_, '_, '_, 'info, ClaimGlobal<'info>>,
        root_seq: u64,
        cumulative_total: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::global::claim_global(ctx, root_seq, cumulative_total, proof)
    }

    pub fn claim_global_sponsored<'info>(
        ctx: Context<'_, '_, '_, 'info, ClaimGlobalSponsored<'info>>,
        root_seq: u64,
        cumulative_total: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::global::claim_global_sponsored(ctx, root_seq, cumulative_total, proof)
    }

    pub fn claim_global_v2<'info>(
        ctx: Context<'_, '_, '_, 'info, ClaimGlobal<'info>>,
        root_seq: u64,
        base_yield: u64,
        attention_bonus: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::global::claim_global_v2(ctx, root_seq, base_yield, attention_bonus, proof)
    }

    pub fn claim_global_sponsored_v2<'info>(
        ctx: Context<'_, '_, '_, 'info, ClaimGlobalSponsored<'info>>,
        root_seq: u64,
        base_yield: u64,
        attention_bonus: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::global::claim_global_sponsored_v2(
            ctx,
            root_seq,
            base_yield,
            attention_bonus,
            proof,
        )
    }

    // =========================================================================
    // Attention Markets — Oracle-Resolved Binary Markets (Phase 2)
    // =========================================================================

    #[cfg(feature = "prediction_markets")]
    pub fn create_market(
        ctx: Context<CreateMarket>,
        market_id: u64,
        creator_wallet: Pubkey,
        metric: u8,
        target: u64,
        resolution_root_seq: u64,
    ) -> Result<()> {
        instructions::markets::create_market(
            ctx,
            market_id,
            creator_wallet,
            metric,
            target,
            resolution_root_seq,
        )
    }

    #[cfg(feature = "prediction_markets")]
    pub fn initialize_market_tokens_v2(ctx: Context<InitializeMarketTokensV2>) -> Result<()> {
        instructions::markets::initialize_market_tokens_v2(ctx)
    }

    #[cfg(feature = "prediction_markets")]
    pub fn mint_shares<'info>(
        ctx: Context<'_, '_, '_, 'info, MintShares<'info>>,
        amount: u64,
    ) -> Result<()> {
        instructions::markets::mint_shares(ctx, amount)
    }

    #[cfg(feature = "prediction_markets")]
    pub fn redeem_shares<'info>(
        ctx: Context<'_, '_, '_, 'info, RedeemShares<'info>>,
        shares: u64,
    ) -> Result<()> {
        instructions::markets::redeem_shares(ctx, shares)
    }

    #[cfg(feature = "prediction_markets")]
    pub fn resolve_market(
        ctx: Context<ResolveMarket>,
        cumulative_total: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::markets::resolve_market(ctx, cumulative_total, proof)
    }

    #[cfg(feature = "prediction_markets")]
    pub fn settle<'info>(
        ctx: Context<'_, '_, '_, 'info, Settle<'info>>,
        shares: u64,
    ) -> Result<()> {
        instructions::markets::settle(ctx, shares)
    }

    #[cfg(feature = "prediction_markets")]
    pub fn sweep_residual<'info>(
        ctx: Context<'_, '_, '_, 'info, SweepResidual<'info>>,
    ) -> Result<()> {
        instructions::markets::sweep_residual(ctx)
    }

    #[cfg(feature = "prediction_markets")]
    pub fn close_market(ctx: Context<CloseMarket>) -> Result<()> {
        instructions::markets::close_market(ctx)
    }

    #[cfg(feature = "prediction_markets")]
    pub fn close_market_mints(ctx: Context<CloseMarketMints>, market_id: u64) -> Result<()> {
        instructions::markets::close_market_mints(ctx, market_id)
    }

    // =========================================================================
    // Market Vault — USDC Deposit, Attention Oracle, Settlement
    // The core product loop: DEPOSIT -> MATURE -> RESOLVE -> SETTLE
    // =========================================================================

    /// One-time protocol initialization. Sets admin, publisher, treasury, oracle, CCM mint.
    pub fn initialize_protocol_state(
        ctx: Context<InitializeProtocolState>,
        publisher: Pubkey,
        treasury: Pubkey,
        oracle_authority: Pubkey,
        ccm_mint: Pubkey,
    ) -> Result<()> {
        instructions::vault::initialize_protocol_state(
            ctx,
            publisher,
            treasury,
            oracle_authority,
            ccm_mint,
        )
    }

    /// Create a market vault with USDC deposit token and vLOFI receipt token.
    pub fn initialize_market_vault(
        ctx: Context<InitializeMarketVault>,
        market_id: u64,
    ) -> Result<()> {
        instructions::vault::initialize_market_vault(ctx, market_id)
    }

    /// Grow existing MarketVault PDA from 137 to 153 bytes (Phase 2 NAV fields).
    /// Admin-only. No-op if already at target size.
    pub fn realloc_market_vault(ctx: Context<ReallocMarketVault>, market_id: u64) -> Result<()> {
        instructions::vault::realloc_market_vault(ctx, market_id)
    }

    /// Deposit USDC into a market vault, receive vLOFI 1:1.
    pub fn deposit_market(ctx: Context<DepositMarket>, market_id: u64, amount: u64) -> Result<()> {
        instructions::vault::deposit_market(ctx, market_id, amount)
    }

    /// Oracle pushes attention multiplier to a user's market position.
    pub fn update_attention(
        ctx: Context<UpdateAttention>,
        market_id: u64,
        user_pubkey: Pubkey,
        multiplier_bps: u64,
    ) -> Result<()> {
        instructions::vault::update_attention(ctx, market_id, user_pubkey, multiplier_bps)
    }

    /// Update NAV (Net Asset Value) per vLOFI share on MarketVault.
    /// Called by oracle authority each rebalance cycle.
    /// nav_per_share_bps must remain within [10_000, 50_000] and be non-decreasing.
    pub fn update_nav(
        ctx: Context<UpdateNav>,
        market_id: u64,
        nav_per_share_bps: u64,
    ) -> Result<()> {
        instructions::vault::update_nav(ctx, market_id, nav_per_share_bps)
    }

    /// Deprecated direct-claim path. Returns `ClaimYieldDeprecated`.
    /// CCM distribution is merkle-claim only via claim_global / claim_global_v2.
    pub fn claim_yield(ctx: Context<ClaimYield>, market_id: u64) -> Result<()> {
        instructions::vault::claim_yield(ctx, market_id)
    }

    /// Burn vLOFI, reclaim USDC principal from reserve, and close the position.
    /// CCM is not minted here; users claim CCM through merkle proofs.
    pub fn settle_market(ctx: Context<SettleMarket>, market_id: u64) -> Result<()> {
        instructions::vault::settle_market(ctx, market_id)
    }

    // =========================================================================
    // Token-2022 Transfer Fee Harvesting — Revenue Infrastructure
    // =========================================================================

    #[cfg(feature = "channel_staking")]
    pub fn initialize_fee_config(
        ctx: Context<InitializeFeeConfig>,
        basis_points: u16,
        treasury_fee_bps: u16,
        creator_fee_bps: u16,
        tier_multipliers: [u32; 6],
    ) -> Result<()> {
        instructions::governance::initialize_fee_config(
            ctx,
            basis_points,
            treasury_fee_bps,
            creator_fee_bps,
            tier_multipliers,
        )
    }

    /// Harvest withheld fees from user/LP token accounts and move to treasury ATA.
    /// Permissionless — anyone can trigger. Source accounts passed via remaining_accounts.
    pub fn harvest_fees<'info>(
        ctx: Context<'_, '_, 'info, 'info, HarvestFees<'info>>,
    ) -> Result<()> {
        instructions::governance::harvest_and_distribute_fees(ctx)
    }

    /// Withdraw accumulated withheld fees from the mint account to treasury ATA.
    /// Permissionless — anyone can trigger.
    pub fn withdraw_fees_from_mint(ctx: Context<WithdrawFeesFromMint>) -> Result<()> {
        instructions::governance::withdraw_fees_from_mint(ctx)
    }

    #[cfg(feature = "strategy")]
    pub fn initialize_strategy_vault(
        ctx: Context<InitializeStrategyVault>,
        reserve_ratio_bps: u16,
        utilization_cap_bps: u16,
        operator_authority: Pubkey,
        klend_program: Pubkey,
        klend_reserve: Pubkey,
        klend_lending_market: Pubkey,
        ctoken_ata: Pubkey,
    ) -> Result<()> {
        instructions::strategy::initialize_strategy_vault(
            ctx,
            reserve_ratio_bps,
            utilization_cap_bps,
            operator_authority,
            klend_program,
            klend_reserve,
            klend_lending_market,
            ctoken_ata,
        )
    }

    #[cfg(feature = "strategy")]
    pub fn deploy_to_strategy(ctx: Context<DeployToStrategy>, amount: u64) -> Result<()> {
        instructions::strategy::deploy_to_strategy(ctx, amount)
    }

    #[cfg(feature = "strategy")]
    pub fn withdraw_from_strategy(ctx: Context<WithdrawFromStrategy>, amount: u64) -> Result<()> {
        instructions::strategy::withdraw_from_strategy(ctx, amount)
    }

    #[cfg(feature = "strategy")]
    pub fn harvest_strategy_yield(ctx: Context<HarvestStrategyYield>) -> Result<()> {
        instructions::strategy::harvest_strategy_yield(ctx)
    }

    #[cfg(feature = "strategy")]
    pub fn emergency_unwind(ctx: Context<EmergencyUnwind>) -> Result<()> {
        instructions::strategy::emergency_unwind(ctx)
    }

    pub fn route_treasury(
        ctx: Context<RouteTreasury>,
        amount: u64,
        min_reserve: u64,
    ) -> Result<()> {
        instructions::governance::route_treasury(ctx, amount, min_reserve)
    }
    // =========================================================================
    // Switchboard Price Feed Bridge — Permissionless cranker pattern
    // =========================================================================

    #[cfg(feature = "price_feed")]
    /// Admin creates a new price feed PDA (e.g. "SOL/USD").
    pub fn initialize_price_feed(
        ctx: Context<InitializePriceFeed>,
        label: [u8; 32],
        updater: Pubkey,
        max_staleness_slots: u64,
    ) -> Result<()> {
        instructions::price_feed::initialize_price_feed(ctx, label, updater, max_staleness_slots)
    }

    #[cfg(feature = "price_feed")]
    /// Registered cranker pushes a Switchboard-sourced price on-chain.
    /// Enforces 20% max deviation guard.
    pub fn update_price(ctx: Context<UpdatePrice>, label: [u8; 32], price: i64) -> Result<()> {
        instructions::price_feed::update_price(ctx, label, price)
    }

    #[cfg(feature = "price_feed")]
    /// Authority rotates the cranker key for a price feed.
    pub fn set_price_updater(
        ctx: Context<SetPriceUpdater>,
        label: [u8; 32],
        new_updater: Pubkey,
    ) -> Result<()> {
        instructions::price_feed::set_price_updater(ctx, label, new_updater)
    }

    // =========================================================================
    // Access Control
    // =========================================================================

    /// Set the treasury wallet (fee destination owner).
    pub fn set_treasury(ctx: Context<SetTreasury>, new_treasury: Pubkey) -> Result<()> {
        instructions::admin::set_treasury(ctx, new_treasury)
    }

    // =========================================================================
    // Channel Staking — Core operations (Phase 2)
    // =========================================================================

    #[cfg(feature = "channel_staking")]
    pub fn create_channel_config_v2(
        ctx: Context<CreateChannelConfigV2>,
        subject: Pubkey,
        authority: Pubkey,
        creator_wallet: Pubkey,
        creator_fee_bps: u16,
    ) -> Result<()> {
        instructions::admin::create_channel_config_v2(
            ctx,
            subject,
            authority,
            creator_wallet,
            creator_fee_bps,
        )
    }

    #[cfg(feature = "channel_staking")]
    pub fn initialize_stake_pool(ctx: Context<InitializeStakePool>) -> Result<()> {
        instructions::staking::initialize_stake_pool(ctx)
    }

    #[cfg(feature = "channel_staking")]
    pub fn stake_channel(
        ctx: Context<StakeChannel>,
        amount: u64,
        lock_duration: u64,
    ) -> Result<()> {
        instructions::staking::stake_channel(ctx, amount, lock_duration)
    }

    #[cfg(feature = "channel_staking")]
    pub fn unstake_channel(ctx: Context<UnstakeChannel>) -> Result<()> {
        instructions::staking::unstake_channel(ctx)
    }

    #[cfg(feature = "channel_staking")]
    pub fn claim_channel_rewards(ctx: Context<ClaimChannelRewards>) -> Result<()> {
        instructions::staking::claim_channel_rewards(ctx)
    }

    // =========================================================================
    // Channel Staking — Admin & Lifecycle (Phase 2)
    // =========================================================================

    #[cfg(feature = "channel_staking")]
    pub fn set_reward_rate(ctx: Context<SetRewardRate>, new_rate: u64) -> Result<()> {
        instructions::staking::set_reward_rate(ctx, new_rate)
    }

    #[cfg(feature = "channel_staking")]
    pub fn emergency_unstake_channel(ctx: Context<EmergencyUnstakeChannel>) -> Result<()> {
        instructions::staking::emergency_unstake_channel(ctx)
    }

    #[cfg(feature = "channel_staking")]
    pub fn admin_shutdown_pool(ctx: Context<AdminShutdownPool>, reason: String) -> Result<()> {
        instructions::staking::admin_shutdown_pool(ctx, reason)
    }

    #[cfg(feature = "channel_staking")]
    pub fn admin_recover_pool(ctx: Context<AdminRecoverPool>) -> Result<()> {
        instructions::staking::admin_recover_pool(ctx)
    }

    #[cfg(feature = "channel_staking")]
    pub fn close_stake_pool(ctx: Context<CloseStakePool>) -> Result<()> {
        instructions::staking::close_stake_pool(ctx)
    }

    /// Realloc the legacy 141-byte ProtocolState PDA (["protocol", mint]) to 173 bytes.
    /// Inserts the oracle_authority field so RouteTreasury can deserialize it.
    /// Admin-only, one-shot migration.
    pub fn realloc_legacy_protocol(ctx: Context<ReallocLegacyProtocol>) -> Result<()> {
        instructions::governance::realloc_legacy_protocol(ctx)
    }

    pub fn admin_fix_ccm_authority(ctx: Context<AdminFixCcmAuthority>) -> Result<()> {
        instructions::governance::admin_fix_ccm_authority(ctx)
    }
}
