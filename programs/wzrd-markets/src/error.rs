//! wzrd-markets custom program errors.
//!
//! Starter set for Phase 0. New variants are added alongside the IX that raises
//! them (Phase 1-3). Each variant carries the precondition that failed — never
//! a generic "something went wrong." Explicit discriminants mirror wzrd-rails so
//! error codes stay stable across SDK regeneration.

use anchor_lang::prelude::*;

#[error_code]
pub enum MarketsError {
    #[msg("Unauthorized: signer is not the configured admin / resolver.")]
    Unauthorized = 0,

    #[msg("Market is not in a state that permits this operation.")]
    InvalidMarketState = 1,

    #[msg("Market has already been resolved.")]
    MarketAlreadyResolved = 2,

    #[msg("Arithmetic overflow / underflow in curve or accounting math.")]
    MathOverflow = 3,

    #[msg("Swap output fell below the caller's minimum-out slippage bound.")]
    SlippageExceeded = 4,

    #[msg("Operation violates the bounding-phase (cold-start) constraints.")]
    BoundingPhaseViolation = 5,

    // ─── Phase 1: market lifecycle + complete-set rail ───────────────────────
    #[msg("Amount must be greater than zero.")]
    ZeroAmount = 6,

    #[msg("Market tokens (YES/NO mints + vault) have already been initialized.")]
    MarketAlreadyHasTokens = 7,

    #[msg("Market tokens have not been initialized yet; call initialize_market_tokens first.")]
    TokensNotInitialized = 8,

    #[msg("Market is resolved; complete-set mint/redeem is pre-resolution only.")]
    MarketResolved = 9,

    #[msg("Redeemer holds fewer outcome tokens than the requested redeem amount.")]
    InsufficientOutcomeBalance = 10,

    #[msg("market_id is not the next sequential id expected by config.")]
    InvalidMarketId = 11,

    #[msg("resolve_deadline_slot must be strictly greater than the current slot.")]
    DeadlineInPast = 12,

    #[msg("resolution_root must be non-zero (a published attention root is required).")]
    ZeroResolutionRoot = 13,

    #[msg("metric is not a defined MarketMetric discriminant.")]
    InvalidMetric = 14,
}
