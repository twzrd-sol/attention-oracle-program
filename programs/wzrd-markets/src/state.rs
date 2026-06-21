//! wzrd-markets account state definitions.
//!
//! Phase 0 defines the full state skeleton (structs + `LEN`) but NO handlers
//! beyond `initialize_markets_config`. Each struct documents its PDA seeds and
//! carries audit-derived fields annotated with the finding they address
//! (H-01 finality snapshot, H-02 in-house publisher, AC-5 seed namespacing).
//!
//! Layout discipline (audit lesson on upgradeable structs): every account
//! struct ends with a generous `_reserved` byte array so future fields append
//! without a migration. New fields are taken from `_reserved` and the array
//! shrunk by the same width, keeping the total account size constant.

use anchor_lang::prelude::*;

// ─── PDA seed constants ──────────────────────────────────────────────────────
// Centralized so off-chain derivation (SDK, keepers, resolver) imports the same
// values. These are the namespace for the NEW wzrd-markets program. Per audit
// AC-5, the per-market PDAs (mint / pool / vault) include the market_id (and the
// mint seeds are market-scoped) so markets cannot collide or alias.
pub const MARKETS_CONFIG_SEED: &[u8] = b"markets_config";
pub const MARKET_SEED: &[u8] = b"market";
pub const POOL_SEED: &[u8] = b"pool";
pub const YES_MINT_SEED: &[u8] = b"yes";
pub const NO_MINT_SEED: &[u8] = b"no";
pub const VAULT_SEED: &[u8] = b"vault";
pub const LP_MINT_SEED: &[u8] = b"lp";
pub const MINT_AUTH_SEED: &[u8] = b"mint_auth";
pub const ATTENTION_ROOT_SEED: &[u8] = b"attention_root";

/// Maximum number of in-house attention-root publishers in the allow-list.
/// Matches wzrd-rails' `PayoutAuthorityConfig::MAX_PUBLISHERS`.
pub const MAX_PUBLISHERS: usize = 8;

/// Which attention metric a market resolves against. Stored as a `u8` on the
/// `Market` account; Phase 1 only persists the value (the resolution logic that
/// interprets it lands in Phase 3). Kept as plain consts (not a Rust enum) so an
/// out-of-range value round-trips through Borsh without an aborting deserialize —
/// `create_market` validates the range explicitly.
#[non_exhaustive]
pub struct MarketMetric;

impl MarketMetric {
    /// Average concurrent viewers over the resolution window.
    pub const AVG_VIEWERS: u8 = 0;
    /// Peak concurrent viewers over the resolution window.
    pub const PEAK_VIEWERS: u8 = 1;
    /// Total hours watched over the resolution window.
    pub const HOURS_WATCHED: u8 = 2;
    /// Composite engagement score over the resolution window.
    pub const ENGAGEMENT_SCORE: u8 = 3;

    /// Highest defined metric discriminant (inclusive). `create_market` rejects
    /// `metric > MAX`.
    pub const MAX: u8 = Self::ENGAGEMENT_SCORE;

    /// True if `metric` is a defined `MarketMetric` discriminant.
    pub fn is_valid(metric: u8) -> bool {
        metric <= Self::MAX
    }
}

/// Global program configuration. One instance per deployment, created by
/// `initialize_markets_config` (Phase 0).
///
/// Holds the admin authority, the (fee-exempt) USDC collateral mint, the
/// resolver multisig that backs `resolve_override` (Phase 3), and the in-house
/// attention-root publisher allow-list.
///
/// PDA: `[MARKETS_CONFIG_SEED]`
#[account]
#[derive(Debug)]
pub struct MarketsConfig {
    /// PDA bump.
    pub bump: u8,
    /// Admin authority (config-level). Should be a Squads V4 vault PDA in
    /// production; any signer for devnet/tests.
    pub admin: Pubkey,
    /// USDC collateral mint. DECISION-LOCKED: collateral is fee-exempt USDC
    /// (6 decimals). The market vault holds USDC; per audit L-08/MR-2 the
    /// collateral must be fee-exempt so the AMM's repeated trade cycling does
    /// not compound a Token-2022 transfer fee into a house edge.
    pub usdc_mint: Pubkey,
    /// Resolver multisig. Backs the Phase 3 `resolve_override` fallback for
    /// disputed / missing-data markets (audit H-02: multisig override path).
    pub resolver_multisig: Pubkey,
    /// In-house attention-root publisher allow-list (audit H-02 option (b):
    /// publish in-house, do NOT cross-program-read the immutable AO root).
    /// Capacity is fixed at `MAX_PUBLISHERS`; `LEN` reserves space for the full
    /// vector so the account never needs to grow when publishers are added.
    pub publisher_allowlist: Vec<Pubkey>,
    /// Monotonic market-id counter (Phase 1). `create_market` requires the
    /// caller-supplied `market_id == next_market_id`, then increments this, so
    /// market ids are gap-free and the `[MARKET_SEED, market_id]` PDA can never
    /// collide. CARVED from the original 64-byte `_reserved` (now 56) so the
    /// account LEN is unchanged — no realloc on the existing Phase-0 config.
    pub next_market_id: u64,
    /// Forward-compat reserve. Future config fields are carved from here.
    /// Was `[u8; 64]` in Phase 0; 8 bytes carved for `next_market_id` above.
    pub _reserved: [u8; 56],
}

impl MarketsConfig {
    /// Account size including the 8-byte Anchor discriminator.
    /// 8 disc + 1 bump + 32 admin + 32 usdc_mint + 32 resolver_multisig
    ///   + (4 vec_len + 32*MAX_PUBLISHERS) publisher_allowlist + 8 next_market_id
    ///   + 56 reserved.
    /// (The 8-byte `next_market_id` is carved from the original 64-byte reserve,
    /// which is now 56 — the total LEN is identical to Phase 0, so no realloc.)
    pub const LEN: usize = 8 + 1 + 32 + 32 + 32 + (4 + 32 * MAX_PUBLISHERS) + 8 + 56;

    pub fn publisher_allowed(&self, publisher: &Pubkey) -> bool {
        self.publisher_allowlist.iter().any(|p| p == publisher)
    }
}

/// A single prediction market over a streamer's future attention metric.
///
/// Phase 1 creates this via `create_market`. The resolution root + seq are
/// SNAPSHOTTED here at create-time (audit H-01) rather than re-read at resolve
/// time, so the finality anchor cannot drift after the market opens.
///
/// PDA: `[MARKET_SEED, market_id.to_le_bytes()]`
#[account]
#[derive(Debug)]
pub struct Market {
    /// PDA bump.
    pub bump: u8,
    /// Schema version for forward migrations.
    pub version: u8,
    /// Monotonic market identifier (also the PDA seed input).
    pub market_id: u64,
    /// Wallet that created the market.
    pub creator: Pubkey,
    /// Hash / id of the streamer this market is about.
    pub streamer_ref: [u8; 32],
    /// Which attention metric this market resolves on (enum-as-u8; defined in
    /// Phase 1).
    pub metric: u8,
    /// Threshold value for threshold-style markets (e.g. "avg viewers >= N").
    pub target: u64,
    /// AUDIT H-01: resolution merkle root snapshotted AT CREATE-TIME. The market
    /// resolves against THIS root, never a root re-read later — that re-read was
    /// the original finality bug.
    pub resolution_root: [u8; 32],
    /// AUDIT H-01: sequence number of the snapshotted root, captured alongside
    /// `resolution_root` at create-time for the same finality reason.
    pub resolution_root_seq: u64,
    /// Slot at which the market was created.
    pub created_slot: u64,
    /// AUDIT H-01: hard finality deadline. After this slot a never-resolved
    /// market is eligible for admin pro-rata recovery (Phase 3).
    pub resolve_deadline_slot: u64,
    /// True once `resolve_market` has fixed the outcome.
    pub resolved: bool,
    /// The resolved outcome (meaningful only when `resolved == true`).
    pub outcome: bool,
    /// Total winning-side supply captured at settle, for solvency accounting.
    pub settled_supply: u64,
    /// AUDIT H-01: dispute / challenge window (in slots) that must elapse after
    /// resolution before settlement is final.
    pub dispute_window_slots: u64,
    /// YES outcome mint (Token-2022, fee-free) for this market.
    pub yes_mint: Pubkey,
    /// NO outcome mint (Token-2022, fee-free) for this market.
    pub no_mint: Pubkey,
    /// USDC collateral vault for this market.
    pub vault: Pubkey,
    /// True once `initialize_market_tokens` (Phase 1) has created the mints.
    pub tokens_initialized: bool,
    /// Forward-compat reserve.
    pub _reserved: [u8; 64],
}

impl Market {
    pub const VERSION: u8 = 1;

    /// Account size including the 8-byte Anchor discriminator.
    /// 8 disc + 1 bump + 1 version + 8 market_id + 32 creator + 32 streamer_ref
    ///   + 1 metric + 8 target + 32 resolution_root + 8 resolution_root_seq
    ///   + 8 created_slot + 8 resolve_deadline_slot + 1 resolved + 1 outcome
    ///   + 8 settled_supply + 8 dispute_window_slots + 32 yes_mint + 32 no_mint
    ///   + 32 vault + 1 tokens_initialized + 64 reserved.
    pub const LEN: usize =
        8 + 1 + 1 + 8 + 32 + 32 + 1 + 8 + 32 + 8 + 8 + 8 + 1 + 1 + 8 + 8 + 32 + 32 + 32 + 1 + 64;
}

/// The constant-product (`x * y = k`) pool over a market's YES/NO outcome
/// tokens. This is the moving-odds engine: price(YES) =
/// `no_reserve / (yes_reserve + no_reserve)`, the implied probability.
///
/// Phase 2 creates this via `initialize_pool` and seeds bounding-phase virtual
/// liquidity for thin long-tail streamer markets.
///
/// PDA: `[POOL_SEED, market.key().as_ref()]`
#[account]
#[derive(Debug)]
pub struct Pool {
    /// PDA bump.
    pub bump: u8,
    /// The market this pool belongs to.
    pub market: Pubkey,
    /// YES outcome-token reserve held by the pool.
    pub yes_reserve: u64,
    /// NO outcome-token reserve held by the pool.
    pub no_reserve: u64,
    /// LP mint for this pool (liquidity-provider receipt token).
    pub lp_mint: Pubkey,
    /// Mirror of the LP mint supply, kept on-account for curve math without a
    /// mint reload.
    pub lp_supply: u64,
    /// True while the cold-start bounding phase is active (first trades priced
    /// against the virtual-liquidity floor, before real LP takes over).
    pub bounding_phase_active: bool,
    /// Bootstrap virtual-liquidity floor for thin markets (scope §4 / Path
    /// Protocol cold-start design). Gives the first trades a sane price.
    pub virtual_liquidity: u64,
    /// Forward-compat reserve.
    pub _reserved: [u8; 32],
}

impl Pool {
    /// Account size including the 8-byte Anchor discriminator.
    /// 8 disc + 1 bump + 32 market + 8 yes_reserve + 8 no_reserve + 32 lp_mint
    ///   + 8 lp_supply + 1 bounding_phase_active + 8 virtual_liquidity
    ///   + 32 reserved.
    pub const LEN: usize = 8 + 1 + 32 + 8 + 8 + 32 + 8 + 1 + 8 + 32;
}

/// In-house attention-root publisher state (audit H-02 option (b)).
///
/// Phase 0 defines the struct only — there is NO publish logic yet. Phase 3's
/// `publish_attention_root` will advance `last_published_seq` monotonically,
/// reusing the wzrd-rails listen-payout publisher + allow-list pattern.
///
/// PDA: `[ATTENTION_ROOT_SEED]`
#[account]
#[derive(Debug)]
pub struct AttentionRootConfig {
    /// PDA bump.
    pub bump: u8,
    /// Highest published root sequence number. Monotonic; enforced by the
    /// Phase 3 publish handler.
    pub last_published_seq: u64,
    /// Forward-compat reserve.
    pub _reserved: [u8; 32],
}

impl AttentionRootConfig {
    /// Account size including the 8-byte Anchor discriminator.
    /// 8 disc + 1 bump + 8 last_published_seq + 32 reserved.
    pub const LEN: usize = 8 + 1 + 8 + 32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_constants_are_stable() {
        // Typo guard: seed byte strings are load-bearing across program + SDKs.
        assert_eq!(MARKETS_CONFIG_SEED, b"markets_config");
        assert_eq!(MARKET_SEED, b"market");
        assert_eq!(POOL_SEED, b"pool");
        assert_eq!(YES_MINT_SEED, b"yes");
        assert_eq!(NO_MINT_SEED, b"no");
        assert_eq!(VAULT_SEED, b"vault");
        assert_eq!(LP_MINT_SEED, b"lp");
        assert_eq!(MINT_AUTH_SEED, b"mint_auth");
        assert_eq!(ATTENTION_ROOT_SEED, b"attention_root");
    }

    #[test]
    fn markets_config_len_matches_manual_calc() {
        // 8 + 1 + 32 + 32 + 32 + (4 + 256) + 8 + 56 = 429
        // The Phase-1 `next_market_id` (8 bytes) is carved from the Phase-0
        // 64-byte reserve (now 56), so the total LEN is unchanged — no realloc.
        assert_eq!(MarketsConfig::LEN, 429);
        assert_eq!(MAX_PUBLISHERS, 8);
    }

    #[test]
    fn market_metric_discriminants_are_stable() {
        // Load-bearing across program + SDK + resolver (Phase 3 interprets these).
        assert_eq!(MarketMetric::AVG_VIEWERS, 0);
        assert_eq!(MarketMetric::PEAK_VIEWERS, 1);
        assert_eq!(MarketMetric::HOURS_WATCHED, 2);
        assert_eq!(MarketMetric::ENGAGEMENT_SCORE, 3);
        assert_eq!(MarketMetric::MAX, 3);
        assert!(MarketMetric::is_valid(0));
        assert!(MarketMetric::is_valid(3));
        assert!(!MarketMetric::is_valid(4));
    }

    #[test]
    fn market_len_matches_manual_calc() {
        assert_eq!(Market::LEN, 326);
        assert_eq!(Market::VERSION, 1);
    }

    #[test]
    fn pool_len_matches_manual_calc() {
        assert_eq!(Pool::LEN, 138);
    }

    #[test]
    fn attention_root_config_len_matches_manual_calc() {
        assert_eq!(AttentionRootConfig::LEN, 49);
    }
}
