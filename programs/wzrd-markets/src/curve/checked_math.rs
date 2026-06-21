//! Checked ceiling division for the constant-product curve.
//!
//! Vendored from `raydium-io/raydium-cp-swap` (`libraries/src/utils/checked_math.rs`,
//! Apache-2.0). Vendored 2026-06-21.
//!
//! Raydium's original file also constructed `U128`/`U256` big-int types via
//! `uint::construct_uint!`. The on-chain constant-product math in this module
//! operates purely on `u128`, so the big-int types are intentionally NOT
//! vendored here — they would add an on-chain dependency for no benefit. The
//! host-only proptest helpers that need wider arithmetic vendor their own U256
//! locally (see `calculator.rs`, `#[cfg(test)]`).

/// Perform a checked ceiling division.
///
/// `ceil_div(a, b)` returns `ceil(a / b)` without ever panicking: division by
/// zero and the (impossible for `u128`) overflow of the `+1` both surface as
/// `None`.
pub trait CheckedCeilDiv: Sized {
    /// Perform ceiling division, returning `None` on divide-by-zero or overflow.
    fn checked_ceil_div(&self, rhs: Self) -> Option<Self>;
}

impl CheckedCeilDiv for u128 {
    fn checked_ceil_div(&self, rhs: Self) -> Option<Self> {
        let mut quotient = self.checked_div(rhs)?;
        let remainder = self.checked_rem(rhs)?;
        if remainder != 0 {
            quotient = quotient.checked_add(1)?;
        }
        Some(quotient)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ceil_div_rounds_up_on_remainder() {
        assert_eq!(7u128.checked_ceil_div(2), Some(4));
        assert_eq!(8u128.checked_ceil_div(2), Some(4));
        assert_eq!(1u128.checked_ceil_div(3), Some(1));
    }

    #[test]
    fn ceil_div_exact_division_does_not_round() {
        assert_eq!(10u128.checked_ceil_div(5), Some(2));
        assert_eq!(0u128.checked_ceil_div(5), Some(0));
    }

    #[test]
    fn ceil_div_by_zero_is_none() {
        assert_eq!(5u128.checked_ceil_div(0), None);
    }
}
