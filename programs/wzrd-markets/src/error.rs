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
}
