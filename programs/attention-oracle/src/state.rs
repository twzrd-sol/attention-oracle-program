//! On-chain state definitions for the Liquid Attention Protocol.

use crate::constants::CUMULATIVE_ROOT_HISTORY;
use anchor_lang::prelude::*;

// =============================================================================
// PROTOCOL STATE
// =============================================================================

/// Global protocol state (singleton per mint)
#[account]
pub struct ProtocolState {
    pub is_initialized: bool,
    pub version: u8,
    pub admin: Pubkey,
    pub publisher: Pubkey,
    pub treasury: Pubkey,
    pub oracle_authority: Pubkey,
    pub mint: Pubkey,
    pub paused: bool,
    /// Legacy field (no longer enforced).
    pub require_receipt: bool,
    pub bump: u8,
}

impl ProtocolState {
    pub const LEN: usize = 8 + 1 + 1 + 32 + 32 + 32 + 32 + 32 + 1 + 1 + 1;
}

/// Fee configuration (PDA account)
#[account]
pub struct FeeConfig {
    pub basis_points: u16,
    pub max_fee: u64,
    pub drip_threshold: u64,
    pub treasury_fee_bps: u16,
    pub creator_fee_bps: u16,
    pub tier_multipliers: [u32; 6],
    pub bump: u8,
}

impl FeeConfig {
    pub const LEN: usize = 8 + 2 + 8 + 8 + 2 + 2 + (4 * 6) + 1;
}

// =============================================================================
// ROOT ENTRIES (shared by global + channel roots)
// =============================================================================

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default)]
pub struct RootEntry {
    pub seq: u64,
    pub root: [u8; 32],
    pub dataset_hash: [u8; 32],
    pub published_slot: u64,
}

impl RootEntry {
    pub const LEN: usize = 8 + 32 + 32 + 8;
}

// =============================================================================
// CHANNEL CONFIG (V2) — Phase 2 (staking)
// =============================================================================

#[cfg(feature = "channel_staking")]
#[account]
pub struct ChannelConfigV2 {
    pub version: u8,
    pub bump: u8,
    pub mint: Pubkey,
    pub subject: Pubkey,
    pub authority: Pubkey,
    pub latest_root_seq: u64,
    pub cutover_epoch: u64,
    pub creator_wallet: Pubkey,
    pub creator_fee_bps: u16,
    pub _padding: [u8; 6],
    pub roots: [RootEntry; CUMULATIVE_ROOT_HISTORY],
}

#[cfg(feature = "channel_staking")]
impl ChannelConfigV2 {
    pub const LEN: usize =
        8 + 1 + 1 + 32 + 32 + 32 + 8 + 8 + 32 + 2 + 6 + (RootEntry::LEN * CUMULATIVE_ROOT_HISTORY);
}

// =============================================================================
// GLOBAL ROOT (V4 CLAIMS)
// =============================================================================

#[account]
pub struct GlobalRootConfig {
    pub version: u8,
    pub bump: u8,
    pub mint: Pubkey,
    pub latest_root_seq: u64,
    pub roots: [RootEntry; CUMULATIVE_ROOT_HISTORY],
}

impl GlobalRootConfig {
    pub const LEN: usize = 8 + 1 + 1 + 32 + 8 + (RootEntry::LEN * CUMULATIVE_ROOT_HISTORY);
}

#[account]
pub struct ClaimStateGlobal {
    pub version: u8,
    pub bump: u8,
    pub mint: Pubkey,
    pub wallet: Pubkey,
    pub claimed_total: u64,
    pub last_claim_seq: u64,
}

impl ClaimStateGlobal {
    pub const LEN: usize = 8 + 1 + 1 + 32 + 32 + 8 + 8;
}

// =============================================================================
// CREATOR MARKETS (Phase 2)
// =============================================================================

#[cfg(feature = "prediction_markets")]
#[account]
pub struct MarketState {
    pub version: u8,
    pub bump: u8,
    pub metric: u8,
    pub resolved: bool,
    pub outcome: bool,
    pub tokens_initialized: bool,
    pub _padding: [u8; 2],
    pub market_id: u64,
    pub mint: Pubkey,
    pub authority: Pubkey,
    pub creator_wallet: Pubkey,
    pub target: u64,
    pub resolution_root_seq: u64,
    pub resolution_cumulative_total: u64,
    pub created_slot: u64,
    pub resolved_slot: u64,
    pub vault: Pubkey,
    pub yes_mint: Pubkey,
    pub no_mint: Pubkey,
    pub mint_authority: Pubkey,
}

#[cfg(feature = "prediction_markets")]
impl MarketState {
    pub const LEN: usize =
        8 + 1 + 1 + 1 + 1 + 1 + 1 + 2 + 8 + 32 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 32 + 32 + 32 + 32;
}

// =============================================================================
// MARKET VAULT — USDC deposits for attention markets
// =============================================================================

/// Per-market vault for USDC deposits.
/// PDA: ["market_vault", protocol_state, market_id_bytes]
///
/// Option C layout (Phase 2): two new fields for NAV-based LP receipt accounting.
///   nav_per_share_bps: 10_000 = 1 USDC/vLOFI at genesis. Floats up as Kamino yield accrues.
///   last_nav_update_slot: 0 = never updated (falls back to 1:1 settlement).
///
/// Realloc: 137 → 153 bytes (+16). Requires one Squads proposal.
#[account]
pub struct MarketVault {
    pub bump: u8,
    pub market_id: u64,
    pub deposit_mint: Pubkey,
    pub vlofi_mint: Pubkey,
    pub vault_ata: Pubkey,
    pub total_deposited: u64,
    pub total_shares: u64,
    pub created_slot: u64,
    /// NAV per vLOFI share in BPS. 10_000 = 1:1. Written by oracle authority.
    /// Zero before Phase 2 realloc — treat as 10_000 (1:1) for settlement.
    pub nav_per_share_bps: u64,
    /// Solana slot when nav_per_share_bps was last written. 0 = never.
    pub last_nav_update_slot: u64,
}

impl MarketVault {
    /// Phase 1 size (pre-realloc): 8+1+8+32+32+32+8+8+8 = 137 bytes.
    pub const LEN_V1: usize = 8 + 1 + 8 + 32 + 32 + 32 + 8 + 8 + 8;
    /// Phase 2 size (post-realloc): +16 bytes for nav_per_share_bps + last_nav_update_slot.
    pub const LEN: usize = Self::LEN_V1 + 8 + 8;
}

/// Per-user position in a specific market.
/// PDA: ["market_position", market_vault, user]
///
/// vLOFI is global + fungible (tradeable, lendable on Kamino/marginfi).
/// This PDA is the "Proof of Attention" anchor — tracks which market the
/// user's capital is deployed in and receives the oracle's attention multiplier.
#[account]
pub struct UserMarketPosition {
    pub bump: u8,
    pub user: Pubkey,
    pub market_vault: Pubkey,
    pub deposited_amount: u64,
    pub shares_minted: u64,
    /// 0 = unresolved, set by oracle at resolution (0-50000 = 0x-5.0x)
    pub attention_multiplier_bps: u64,
    pub settled: bool,
    pub entry_slot: u64,
    /// Legacy cumulative direct-claim accounting (kept for backward compatibility).
    /// New CCM distribution is merkle-claim based.
    pub cumulative_claimed: u64,
}

impl UserMarketPosition {
    pub const LEN: usize = 8 + 1 + 32 + 32 + 8 + 8 + 8 + 1 + 8 + 8;
}

// =============================================================================
// STRATEGY VAULT (Phase 2) — Kamino K-Lend Only
// =============================================================================

/// Per-market Kamino lending strategy. Deploys idle USDC from MarketVault into
/// Kamino K-Lend (`KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD`) and receives
/// cTokens (cUSDC) as receipt. Single-strategy — no multi-protocol routing.
///
/// PDA: ["strategy_vault", market_vault]
///
/// Accounting model: `deployed_amount` tracks raw USDC sent to Kamino.
/// NAV = ctoken_balance × total_supply / mint_total_supply (computed off-chain
/// by the rebalancer). Yield = NAV - deployed_amount.
///
/// The on-chain program never reads Kamino's Reserve account directly — the
/// rebalancer does that off-chain and decides when to deploy/withdraw.
#[cfg(feature = "strategy")]
#[account]
pub struct StrategyVault {
    pub version: u8,
    pub bump: u8,
    /// 0 = active, 1 = emergency (no deploys, unwind pending)
    pub status: u8,
    // ── Policy ──────────────────────────────────────────────────────────
    /// Minimum % of total_managed that must remain in vault_usdc_ata (e.g. 3000 = 30%).
    pub reserve_ratio_bps: u16,
    /// Maximum % of total_managed that can be deployed (e.g. 5000 = 50%).
    pub utilization_cap_bps: u16,
    // ── Authorities ─────────────────────────────────────────────────────
    pub protocol_state: Pubkey,
    pub market_vault: Pubkey,
    /// USDC mint (EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v on mainnet).
    pub deposit_mint: Pubkey,
    /// Multisig admin — controls initialize + emergency_unwind.
    pub admin_authority: Pubkey,
    /// Oracle/operator — controls deploy, withdraw, harvest.
    pub operator_authority: Pubkey,
    // ── Kamino-specific addresses (pinned at init, immutable) ───────────
    /// Kamino K-Lend program: `KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD`
    pub klend_program: Pubkey,
    /// Kamino USDC Reserve account (8,616 bytes).
    pub klend_reserve: Pubkey,
    /// Kamino main lending market.
    pub klend_lending_market: Pubkey,
    /// cToken (cUSDC) ATA owned by the MarketVault PDA. Receipt tokens live here.
    pub ctoken_ata: Pubkey,
    // ── Bookkeeping ─────────────────────────────────────────────────────
    /// Raw USDC lamports deployed to Kamino (authoritative on-chain truth).
    pub deployed_amount: u64,
    /// USDC amount pending withdrawal in emergency mode.
    pub pending_withdraw_amount: u64,
    /// Cumulative USDC yield harvested to treasury.
    pub harvested_yield_amount: u64,
    pub last_deploy_slot: u64,
    pub last_withdraw_slot: u64,
    pub last_harvest_slot: u64,
}

#[cfg(feature = "strategy")]
impl StrategyVault {
    // disc(8) + version(1) + bump(1) + status(1) + reserve_ratio(2) + util_cap(2)
    // + 9 pubkeys(288) + 6 u64s(48) = 351
    pub const LEN: usize = 8 + 1 + 1 + 1 + 2 + 2 + (32 * 9) + (8 * 6);
}

// =============================================================================
// PRICE FEED — Switchboard bridge (permissionless cranker)
// =============================================================================

/// External price feed written by a registered cranker (Switchboard bridge).
/// PDA: ["price_feed", &label]
#[account]
pub struct PriceFeedState {
    pub bump: u8,
    pub version: u8,
    /// 32-byte label (e.g. padded "SOL/USD"). Used as PDA seed.
    pub label: [u8; 32],
    /// Admin who created this feed (can rotate updater).
    pub authority: Pubkey,
    /// Cranker key allowed to push price updates.
    pub updater: Pubkey,
    /// Latest price in 6-decimal fixed-point (e.g. 150_000_000 = $150.00).
    pub price: i64,
    pub last_update_slot: u64,
    pub last_update_ts: i64,
    /// Max slots before this feed is considered stale.
    pub max_staleness_slots: u64,
    pub num_updates: u64,
}

impl PriceFeedState {
    // disc(8) + bump(1) + version(1) + label(32) + authority(32) + updater(32)
    // + price(8) + last_update_slot(8) + last_update_ts(8) + max_staleness(8) + num_updates(8)
    pub const LEN: usize = 8 + 1 + 1 + 32 + 32 + 32 + 8 + 8 + 8 + 8 + 8;
}

// =============================================================================
// CHANNEL STAKING (Phase 2)
// =============================================================================

#[cfg(feature = "channel_staking")]
#[account]
pub struct ChannelStakePool {
    pub bump: u8,
    pub channel: Pubkey,
    pub mint: Pubkey,
    pub vault: Pubkey,
    pub total_staked: u64,
    pub total_weighted: u64,
    pub staker_count: u64,
    pub acc_reward_per_share: u128,
    pub last_reward_slot: u64,
    pub reward_per_slot: u64,
    pub is_shutdown: bool,
}

#[cfg(feature = "channel_staking")]
impl ChannelStakePool {
    pub const LEN: usize = 162;
}

#[cfg(feature = "channel_staking")]
#[account]
pub struct UserChannelStake {
    pub bump: u8,
    pub user: Pubkey,
    pub channel: Pubkey,
    pub amount: u64,
    pub start_slot: u64,
    pub lock_end_slot: u64,
    pub multiplier_bps: u64,
    pub nft_mint: Pubkey,
    pub reward_debt: u128,
    pub pending_rewards: u64,
}

#[cfg(feature = "channel_staking")]
impl UserChannelStake {
    pub const LEN: usize = 161;
}
