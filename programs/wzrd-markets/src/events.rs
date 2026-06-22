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

/// Emitted by `add_publisher` / `remove_publisher` (Phase 3 allow-list admin).
/// `added` is true on insert, false on removal. `count` is the post-op size of
/// the allow-list.
#[event]
pub struct PublisherAllowlistChanged {
    pub config: Pubkey,
    pub publisher: Pubkey,
    pub added: bool,
    pub count: u8,
    pub slot: u64,
}

/// Emitted by `publish_attention_root` (Phase 3). One per resolution window. The
/// off-chain builder (cross-repo contract, conventions v1) is expected to read
/// `window_id` + `merkle_root` + `leaf_count` back from this event / account.
#[event]
pub struct AttentionRootPublished {
    pub window_id: u64,
    pub merkle_root: [u8; 32],
    pub leaf_count: u32,
    pub schema_version: u8,
    pub seq: u64,
    pub publisher: Pubkey,
    pub published_at_slot: u64,
}

/// Emitted by `resolve_market` (Phase 3). The outcome is encoded per
/// `resolution::outcome` (0=NO, 1=YES, 2=INVALID). `observed_value` is the metric
/// value the resolution leaf committed. `settle_unlock_slot` is when the dispute
/// window closes and `settle` becomes legal.
#[event]
pub struct MarketResolved {
    pub market: Pubkey,
    pub market_id: u64,
    pub outcome: u8,
    pub observed_value: u64,
    pub resolved_at_slot: u64,
    pub settle_unlock_slot: u64,
}

/// Emitted by `extend_dispute_window` (Phase 3). The one-shot admin extension;
/// `new_settle_unlock_slot` is the post-extension unlock slot.
#[event]
pub struct DisputeWindowExtended {
    pub market: Pubkey,
    pub market_id: u64,
    pub old_settle_unlock_slot: u64,
    pub new_settle_unlock_slot: u64,
    pub slot: u64,
}

/// Emitted by `settle` (Phase 3). `winner` is the winning outcome (0=NO, 1=YES);
/// `amount` winning-outcome tokens were burned and `amount` USDC paid to the
/// settler — the lockstep that preserves `vault >= winning_supply` (audit MR-1).
#[event]
pub struct Settled {
    pub market: Pubkey,
    pub market_id: u64,
    pub winner: u8,
    pub amount: u64,
    pub settler: Pubkey,
}

/// Emitted by `resolve_override` (Phase 3). The multisig corrected a contested
/// resolution pre-settle. `old_outcome`/`new_outcome` are encoded per
/// `resolution::outcome`. `new_settle_unlock_slot` reflects the restarted
/// re-dispute window.
#[event]
pub struct ResolutionOverridden {
    pub market: Pubkey,
    pub market_id: u64,
    pub old_outcome: u8,
    pub new_outcome: u8,
    pub new_settle_unlock_slot: u64,
    pub slot: u64,
}

/// Emitted by `sweep_residual` (Phase 3). Remaining vault dust swept to the
/// treasury after all winning (or, for INVALID, all) supply was settled/redeemed.
#[event]
pub struct ResidualSwept {
    pub market: Pubkey,
    pub market_id: u64,
    pub amount: u64,
    pub recipient: Pubkey,
    pub slot: u64,
}

/// Emitted by `close_market` (Phase 3). The Market account was closed and its
/// rent returned after full settlement + sweep.
#[event]
pub struct MarketClosed {
    pub market: Pubkey,
    pub market_id: u64,
    pub rent_recipient: Pubkey,
    pub slot: u64,
}

/// Emitted by `set_admin` (audit C-02 step 1). The current admin proposed a new
/// admin; not effective until `accept_admin`. `pending_admin == Pubkey::default()`
/// means a pending rotation was cancelled.
#[event]
pub struct AdminRotationProposed {
    pub config: Pubkey,
    pub current_admin: Pubkey,
    pub pending_admin: Pubkey,
    pub slot: u64,
}

/// Emitted by `accept_admin` (audit C-02 step 2). The proposed admin accepted;
/// `admin` is now `new_admin` and the pending slot was cleared.
#[event]
pub struct AdminRotated {
    pub config: Pubkey,
    pub old_admin: Pubkey,
    pub new_admin: Pubkey,
    pub slot: u64,
}
