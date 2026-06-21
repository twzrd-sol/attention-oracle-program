//! Constant-product (`x * y = k`) curve math for the YES/NO outcome-token pool.
//!
//! ## Attribution
//!
//! The constant-product math in this module is vendored from
//! **`raydium-io/raydium-cp-swap`** (Raydium Constant Product Swap), licensed
//! **Apache-2.0**. Vendored 2026-06-21.
//!
//! Source files ported:
//!   - `constant_product.rs` (the `(x+dx)(y-dy)=xy` swap math + LP conversion)
//!   - `calculator.rs` (the `RoundDirection` / `TradeDirection` /
//!     `TradingTokenResult` types + the audited curve-value-non-decrease
//!     proptest helpers)
//!   - `checked_math.rs` (the `CheckedCeilDiv` ceiling-division trait)
//!
//! ## Why fork this and not reimplement
//!
//! The `curve_value_does_not_decrease_from_{swap,deposit,withdraw}` proptests
//! are the audited correctness properties of the AMM. Forking Raydium's
//! battle-tested, audited implementation (rather than writing the math fresh)
//! is the whole reason these properties hold; the tests MUST stay green.
//!
//! ## Hardening applied during the port
//!
//! Raydium's swap functions returned via `.unwrap()`. On-chain Solana code must
//! never panic on bad input, so every arithmetic step is checked here and the
//! swap functions return `Option<_>`. Callers (Phase 2 swap handlers) map
//! `None` to a program error. The math itself is byte-for-byte preserved.

pub mod calculator;
pub mod checked_math;
pub mod constant_product;

pub use calculator::{RoundDirection, TradeDirection, TradingTokenResult};
pub use checked_math::CheckedCeilDiv;
pub use constant_product::ConstantProductCurve;
