//! Curve calculator types for the constant-product outcome-token pool.
//!
//! Vendored from `raydium-io/raydium-cp-swap` (`libraries/src/curve/calculator.rs`,
//! Apache-2.0). Vendored 2026-06-21.
//!
//! Only the types the constant-product core needs are ported:
//! `RoundDirection`, `TradeDirection`, and `TradingTokenResult`. Raydium's
//! fee-bearing `SwapResult` / `CurveCalculator` swap surface and the LaunchLab
//! calculators are intentionally dropped — fees and the swap-handler surface
//! are Phase 1-3 concerns and outcome-token mints here are fee-free.

/// The direction of a trade. Outcome-token pools are symmetric (YES <-> NO),
/// but the direction is preserved from the upstream curve so the audited
/// invariant helpers keep their original shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TradeDirection {
    /// Input token 0, output token 1.
    ZeroForOne,
    /// Input token 1, output token 0.
    OneForZero,
}

impl TradeDirection {
    /// Given a trade direction, gives the opposite direction of the trade, so
    /// A to B becomes B to A, and vice versa.
    pub fn opposite(&self) -> TradeDirection {
        match self {
            TradeDirection::ZeroForOne => TradeDirection::OneForZero,
            TradeDirection::OneForZero => TradeDirection::ZeroForOne,
        }
    }
}

/// The direction to round. Used for pool-token to trading-token conversions to
/// avoid losing value on any deposit or withdrawal.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundDirection {
    /// Floor the value, ie. 1.9 => 1, 1.1 => 1, 1.5 => 1.
    Floor,
    /// Ceiling the value, ie. 1.9 => 2, 1.1 => 2, 1.5 => 2.
    Ceiling,
}

/// Encodes the result of converting LP tokens to the two underlying trading
/// tokens (the both-sided deposit/withdraw amounts).
#[derive(Debug, PartialEq, Eq)]
pub struct TradingTokenResult {
    /// Amount of token 0 (YES reserve side).
    pub token_0_amount: u128,
    /// Amount of token 1 (NO reserve side).
    pub token_1_amount: u128,
}

/// Test helpers for the constant-product curve.
///
/// These are the audited correctness properties that justify forking Raydium's
/// math: a swap, deposit, or withdraw must never decrease the pool's value.
///
/// Two faithfulness-preserving simplifications vs the upstream Raydium helpers:
///   1. A minimal `U256` is vendored locally (via `uint::construct_uint!`,
///      exactly as raydium-cp-swap's own `checked_math.rs` does) instead of
///      pulling `spl_math::uint::U256`.
///   2. The withdraw check compares pool value via a squared U256 inequality
///      instead of `spl_math::precise_number::PreciseNumber::sqrt`. This is
///      algebraically equivalent: Raydium asserts
///      `sqrt(new_a*new_b) * supply >= sqrt(a*b) * new_supply`, and because
///      both sides are non-negative this holds iff
///      `(new_a*new_b) * supply^2 >= (a*b) * new_supply^2`. No epsilon fudge is
///      needed: the squared form is exact (no sqrt rounding to compensate for).
#[cfg(test)]
pub mod test {
    use super::*;
    use crate::curve::constant_product::ConstantProductCurve;
    use proptest::prelude::*;

    uint::construct_uint! {
        /// 256-bit unsigned integer for overflow-free curve-value comparisons.
        pub struct U256(4);
    }

    /// Test function checking that a swap never reduces the overall value of
    /// the pool.
    ///
    /// Since curve calculations use unsigned integers, there is potential for
    /// truncation at some point, meaning a potential for value to be lost in
    /// either direction if too much is given to the swapper.
    ///
    /// This test guarantees that the value will never decrease from a trade.
    pub fn check_curve_value_from_swap(
        source_token_amount: u128,
        swap_source_amount: u128,
        swap_destination_amount: u128,
        trade_direction: TradeDirection,
    ) {
        let destination_amount_swapped = ConstantProductCurve::swap_base_input_without_fees(
            source_token_amount,
            swap_source_amount,
            swap_destination_amount,
        )
        .expect("swap_base_input_without_fees overflowed on in-range inputs");

        let (swap_token_0_amount, swap_token_1_amount) = match trade_direction {
            TradeDirection::ZeroForOne => (swap_source_amount, swap_destination_amount),
            TradeDirection::OneForZero => (swap_destination_amount, swap_source_amount),
        };
        // Widen to U256: the post-swap product can exceed u128 for u64-scale
        // reserves (~2^64 * ~2^64 fits u128, but we stay defensive).
        let previous_value = U256::from(swap_token_0_amount) * U256::from(swap_token_1_amount);

        let new_swap_source_amount = swap_source_amount
            .checked_add(source_token_amount)
            .expect("new source overflow");
        let new_swap_destination_amount = swap_destination_amount
            .checked_sub(destination_amount_swapped)
            .expect("new destination underflow");
        let (swap_token_0_amount, swap_token_1_amount) = match trade_direction {
            TradeDirection::ZeroForOne => (new_swap_source_amount, new_swap_destination_amount),
            TradeDirection::OneForZero => (new_swap_destination_amount, new_swap_source_amount),
        };

        let new_value = U256::from(swap_token_0_amount) * U256::from(swap_token_1_amount);
        assert!(new_value >= previous_value);
    }

    /// Test function checking that a deposit never reduces the value of pool
    /// tokens.
    ///
    /// The following inequality must hold for each side:
    ///   new_token / new_pool_token_supply >= token / pool_token_supply
    /// which reduces to:
    ///   new_token * pool_token_supply >= token * new_pool_token_supply
    ///
    /// These numbers can be just slightly above u64 after the deposit, so the
    /// multiplication can exceed u128. We bump these up to U256.
    pub fn check_pool_value_from_deposit(
        lp_token_amount: u128,
        lp_token_supply: u128,
        swap_token_0_amount: u128,
        swap_token_1_amount: u128,
    ) {
        let deposit_result = ConstantProductCurve::lp_tokens_to_trading_tokens(
            lp_token_amount,
            lp_token_supply,
            swap_token_0_amount,
            swap_token_1_amount,
            RoundDirection::Ceiling,
        )
        .expect("lp_tokens_to_trading_tokens (ceiling) returned None");
        let new_swap_token_0_amount = swap_token_0_amount + deposit_result.token_0_amount;
        let new_swap_token_1_amount = swap_token_1_amount + deposit_result.token_1_amount;
        let new_lp_token_supply = lp_token_supply + lp_token_amount;

        let lp_token_supply = U256::from(lp_token_supply);
        let new_lp_token_supply = U256::from(new_lp_token_supply);
        let swap_token_0_amount_u = U256::from(swap_token_0_amount);
        let new_swap_token_0_amount_u = U256::from(new_swap_token_0_amount);
        let swap_token_1_amount_u = U256::from(swap_token_1_amount);
        let new_swap_token_1_amount_u = U256::from(new_swap_token_1_amount);

        assert!(
            new_swap_token_0_amount_u * lp_token_supply
                >= swap_token_0_amount_u * new_lp_token_supply
        );
        assert!(
            new_swap_token_1_amount_u * lp_token_supply
                >= swap_token_1_amount_u * new_lp_token_supply
        );
    }

    /// Test function checking that a withdraw never reduces the value of pool
    /// tokens.
    ///
    /// Raydium's reference asserts (using a square-root pool value):
    ///   new_pool_value * pool_token_supply >= pool_value * new_pool_token_supply
    /// where `pool_value = sqrt(token_a * token_b)`.
    ///
    /// Squaring both non-negative sides gives the exact, sqrt-free form used
    /// here:
    ///   (new_a*new_b) * supply^2 >= (a*b) * new_supply^2
    pub fn check_pool_value_from_withdraw(
        lp_token_amount: u128,
        lp_token_supply: u128,
        swap_token_0_amount: u128,
        swap_token_1_amount: u128,
    ) {
        let withdraw_result = ConstantProductCurve::lp_tokens_to_trading_tokens(
            lp_token_amount,
            lp_token_supply,
            swap_token_0_amount,
            swap_token_1_amount,
            RoundDirection::Floor,
        )
        .expect("lp_tokens_to_trading_tokens (floor) returned None");
        let new_swap_token_0_amount = swap_token_0_amount - withdraw_result.token_0_amount;
        let new_swap_token_1_amount = swap_token_1_amount - withdraw_result.token_1_amount;
        let new_pool_token_supply = lp_token_supply - lp_token_amount;

        // value^2 = a * b (the squared normalized pool value).
        let value_sq = U256::from(swap_token_0_amount) * U256::from(swap_token_1_amount);
        let new_value_sq =
            U256::from(new_swap_token_0_amount) * U256::from(new_swap_token_1_amount);

        let supply = U256::from(lp_token_supply);
        let new_supply = U256::from(new_pool_token_supply);

        // new_value^2 * supply^2 >= value^2 * new_supply^2
        // (the squared form of: new_value * supply >= value * new_supply)
        let lhs = new_value_sq * supply * supply;
        let rhs = value_sq * new_supply * new_supply;
        assert!(lhs >= rhs);
    }

    prop_compose! {
        /// Generate a `(total, intermediate)` pair where `1 <= intermediate < total`.
        pub fn total_and_intermediate(max_value: u64)(total in 1..max_value)
                        (intermediate in 1..total, total in Just(total))
                        -> (u64, u64) {
           (total, intermediate)
       }
    }
}
