//! Stripe / SPT leaf preimage for the evidence-ledger proof surface.
//!
//! Off-chain convention only. The program verifies 32-byte digests; this module
//! defines how a Stripe-shaped receipt becomes one. No Stripe API, no spend,
//! no trust refuse.

use crate::keccak::keccak256;

/// Domain tag for the Stripe SPT leaf domain (`log_id` = `stripe.agentic.spt.v1`).
pub const STRIPE_SPT_LEAF_DOMAIN: &[u8] = b"TWZRD:STRIPE_SPT_LEAF_V1";

/// `log_id` string for `init_ledger` of this leaf domain.
pub const STRIPE_SPT_LOG_ID: &[u8] = b"stripe.agentic.spt.v1";

/// Payer binding: keccak of a stable Stripe customer / agent ref.
pub fn stripe_payer_hash(customer_ref: &[u8]) -> [u8; 32] {
    keccak256(&[b"stripe:customer:", customer_ref])
}

/// Resource binding: payment_intent id plus the resource the agent bought.
pub fn stripe_resource_hash(payment_intent_id: &[u8], resource_id: &[u8]) -> [u8; 32] {
    keccak256(&[b"stripe:pi:", payment_intent_id, &[0u8], resource_id])
}

/// Canonical leaf digest matching LEDGER.md Stripe / SPT section.
pub fn stripe_spt_leaf(
    customer_ref: &[u8],
    payment_intent_id: &[u8],
    resource_id: &[u8],
    amount_cents: u64,
    created_unix: u64,
) -> [u8; 32] {
    let payer = stripe_payer_hash(customer_ref);
    let resource = stripe_resource_hash(payment_intent_id, resource_id);
    keccak256(&[
        STRIPE_SPT_LEAF_DOMAIN,
        &payer,
        &resource,
        &amount_cents.to_le_bytes(),
        &created_unix.to_le_bytes(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checked-in fixture (also under `tests/fixtures/stripe_spt_leaf_v1.json`).
    fn spike_fixture_leaf() -> [u8; 32] {
        stripe_spt_leaf(
            b"cus_agent_spike_001",
            b"pi_3SpikeTest000000000000000",
            b"mcp://twzrd.intel/readiness_card",
            25,
            1_758_480_000,
        )
    }

    #[test]
    fn fixture_leaf_is_stable() {
        let leaf = spike_fixture_leaf();
        let expected: [u8; 32] = [
            0x9f,
            0x63,
            0xb5,
            0xeb,
            0x2a,
            0x19,
            0xc6,
            0x23,
            0xf2,
            0x64,
            0x7c,
            0xce,
            0x74,
            0xfe,
            0x3e,
            0xee,
            0x74,
            0xac,
            0x42,
            0xa2,
            0xdf,
            0x30,
            0x87,
            0x35,
            0x9f,
            0x31,
            0x9d,
            0x67,
            0x9b,
            0x0e,
            0x69,
            0x04,
        ];
        assert_eq!(leaf, expected);
    }

    #[test]
    fn amount_or_resource_change_moves_leaf() {
        let base = spike_fixture_leaf();
        let other_amount = stripe_spt_leaf(
            b"cus_agent_spike_001",
            b"pi_3SpikeTest000000000000000",
            b"mcp://twzrd.intel/readiness_card",
            26,
            1_758_480_000,
        );
        let other_resource = stripe_spt_leaf(
            b"cus_agent_spike_001",
            b"pi_3SpikeTest000000000000000",
            b"mcp://twzrd.intel/directory",
            25,
            1_758_480_000,
        );
        assert_ne!(base, other_amount);
        assert_ne!(base, other_resource);
    }
}
