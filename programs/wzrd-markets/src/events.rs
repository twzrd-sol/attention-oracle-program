//! wzrd-markets events.
//!
//! Phase 0 emits only `MarketsConfigInitialized`. `MarketCreated` and
//! `PoolInitialized` are defined now (Phase 1/2 will emit them) so the off-chain
//! indexer schema can be drafted ahead of the handlers. Remaining lifecycle
//! events are stubbed as TODOs against their phase.

use anchor_lang::prelude::*;

/// Emitted by `initialize_markets_config` (Phase 0). One per deployment.
#[event]
pub struct MarketsConfigInitialized {
    pub config: Pubkey,
    pub admin: Pubkey,
    pub usdc_mint: Pubkey,
    pub resolver_multisig: Pubkey,
    pub slot: u64,
}

/// Emitted by `create_market` (Phase 1). The resolution root + seq are
/// snapshotted at create-time per audit H-01, so they appear here as the
/// committed finality anchor.
#[event]
pub struct MarketCreated {
    pub market: Pubkey,
    pub market_id: u64,
    pub creator: Pubkey,
    pub streamer_ref: [u8; 32],
    pub metric: u8,
    pub target: u64,
    pub resolution_root: [u8; 32],
    pub resolution_root_seq: u64,
    pub resolve_deadline_slot: u64,
    pub slot: u64,
}

/// Emitted by `initialize_pool` (Phase 2). The constant-product YES/NO pool over
/// the market's outcome tokens, optionally seeded with bounding-phase virtual
/// liquidity for thin markets.
#[event]
pub struct PoolInitialized {
    pub market: Pubkey,
    pub pool: Pubkey,
    pub lp_mint: Pubkey,
    pub yes_reserve: u64,
    pub no_reserve: u64,
    pub virtual_liquidity: u64,
    pub slot: u64,
}

/// Emitted by `initialize_market_tokens` (Phase 1). The per-market YES/NO
/// Token-2022 mints + the USDC collateral vault + the mint-authority PDA that
/// signs the complete-set rail's mint/burn.
#[event]
pub struct TokensInitialized {
    pub market: Pubkey,
    pub market_id: u64,
    pub yes_mint: Pubkey,
    pub no_mint: Pubkey,
    pub vault: Pubkey,
    pub mint_authority: Pubkey,
    pub slot: u64,
}

/// Emitted by `mint_complete_set` (Phase 1). `deposit_amount` is what the caller
/// asked to deposit; `net_amount` is the audit-MR-1 before/after-sampled USDC the
/// vault actually received (== `deposit_amount` for fee-exempt USDC, kept
/// distinct as defense-in-depth). Exactly `net_amount` YES AND NO were minted.
#[event]
pub struct CompleteSetMinted {
    pub market: Pubkey,
    pub market_id: u64,
    pub depositor: Pubkey,
    pub deposit_amount: u64,
    pub net_amount: u64,
}

/// Emitted by `redeem_complete_set` (Phase 1). `amount` YES AND NO were burned
/// from the redeemer and `amount` USDC returned from the vault — the inverse of
/// the fixed-par mint, preserving `vault == yes_supply == no_supply`.
#[event]
pub struct CompleteSetRedeemed {
    pub market: Pubkey,
    pub market_id: u64,
    pub redeemer: Pubkey,
    pub amount: u64,
}

/// Emitted by `add_liquidity` (Phase 2). `yes_in`/`no_in` are the REAL amounts
/// transferred into the pool (bounded by the scarcer side at the current ratio);
/// `lp_minted` is the proportional LP share. `bounding_phase_active` reports the
/// post-add cold-start state (it flips false once both real reserves >= V).
#[event]
pub struct LiquidityAdded {
    pub pool: Pubkey,
    pub provider: Pubkey,
    pub yes_in: u64,
    pub no_in: u64,
    pub lp_minted: u64,
    pub yes_reserve: u64,
    pub no_reserve: u64,
    pub lp_supply: u64,
    pub bounding_phase_active: bool,
}

/// Emitted by `remove_liquidity` (Phase 2). `lp_burned` LP tokens were burned for
/// `yes_out`/`no_out` outcome tokens, floor-rounded so the pool never overpays
/// (LP keeps <= pro-rata; dust stays in the pool).
#[event]
pub struct LiquidityRemoved {
    pub pool: Pubkey,
    pub provider: Pubkey,
    pub lp_burned: u64,
    pub yes_out: u64,
    pub no_out: u64,
    pub yes_reserve: u64,
    pub no_reserve: u64,
    pub lp_supply: u64,
}

/// Emitted by `swap` (Phase 2). `direction` is `0 = YesToNo`, `1 = NoToYes`.
/// `implied_no_price_bps` is the post-swap implied probability of NO scaled to
/// basis points. In the CPMM-prediction model an outcome's price is the OPPOSITE
/// reserve over the total (buying an outcome depletes its reserve → scarcer →
/// pricier), so price(NO) = `yes_reserve * 10_000 / (yes_reserve + no_reserve)`
/// over the REAL reserves (price moves; the virtual floor only shifts the swap
/// calculation, never the real reserves the price is read from).
#[event]
pub struct Swapped {
    pub pool: Pubkey,
    pub trader: Pubkey,
    pub direction: u8,
    pub amount_in: u64,
    pub amount_out: u64,
    pub yes_reserve: u64,
    pub no_reserve: u64,
    pub implied_no_price_bps: u64,
}

// TODO(Phase 3): AttentionRootPublished { seq, root, published_by, slot }
// TODO(Phase 3): MarketResolved { market, outcome, resolved_slot }
// TODO(Phase 3): MarketSettled { market, user, shares_burned, collateral_paid }
// TODO(Phase 3): MarketResolvedByOverride { market, outcome, multisig, slot }
// TODO(Phase 3): ResidualSwept / MarketClosed
