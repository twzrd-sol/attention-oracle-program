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

// TODO(Phase 1): MarketTokensInitialized { market, yes_mint, no_mint, mint_auth }
// TODO(Phase 1): CompleteSetMinted { market, user, collateral_in, shares_out }
// TODO(Phase 1): CompleteSetRedeemed { market, user, shares_in, collateral_out }
// TODO(Phase 2): LiquidityAdded { pool, provider, yes_in, no_in, lp_minted }
// TODO(Phase 2): LiquidityRemoved { pool, provider, lp_burned, yes_out, no_out }
// TODO(Phase 2): Swapped { pool, trader, side, amount_in, amount_out, new_yes_reserve, new_no_reserve }
// TODO(Phase 3): AttentionRootPublished { seq, root, published_by, slot }
// TODO(Phase 3): MarketResolved { market, outcome, resolved_slot }
// TODO(Phase 3): MarketSettled { market, user, shares_burned, collateral_paid }
// TODO(Phase 3): MarketResolvedByOverride { market, outcome, multisig, slot }
// TODO(Phase 3): ResidualSwept / MarketClosed
