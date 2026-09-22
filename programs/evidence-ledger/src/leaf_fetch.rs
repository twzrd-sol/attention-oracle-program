//! reader.fetch.v1 leaf. Off-chain convention. The program only checks a
//! 32-byte digest. This module is not an instruction and is not in the
//! release SBF binary.

use crate::keccak::keccak256;

pub const READER_FETCH_LEAF_DOMAIN: &[u8] = b"TWZRD:READER_FETCH_LEAF_V1";
pub const READER_FETCH_LOG_ID: &[u8] = b"reader.fetch.v1";

pub fn reader_fetch_leaf(
    payer: &[u8; 32],
    resource_hash: &[u8; 32],
    amount_usdc: u64,
    settlement_height: u64,
) -> [u8; 32] {
    keccak256(&[
        READER_FETCH_LEAF_DOMAIN,
        payer,
        resource_hash,
        &amount_usdc.to_le_bytes(),
        &settlement_height.to_le_bytes(),
    ])
}

/// 12 zero bytes then the 20-byte address. The chain does not interpret this.
pub fn payer_from_base_address(addr: &[u8; 20]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[12..].copy_from_slice(addr);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn markdown_hash(markdown: &str) -> [u8; 32] {
        Sha256::digest(markdown.as_bytes()).into()
    }

    fn decode_hex(s: &str) -> [u8; 32] {
        let mut out = [0u8; 32];
        for i in 0..32 {
            out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap();
        }
        out
    }

    const SOLANA_PAYER: [u8; 32] = [0x11; 32];
    const BASE_ADDR: [u8; 20] = [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff, 0x00, 0x11, 0x22, 0x33,
    ];

    #[test]
    fn golden_solana_scrape_matches_fixture() {
        let leaf = reader_fetch_leaf(
            &SOLANA_PAYER,
            &markdown_hash("# fetched\n"),
            5000,
            444_956_973,
        );
        assert_eq!(leaf, decode_hex("7d65918d4d4f4174964914ad9eb4922818131e0613340b9e07adb32956dddd92"));
    }

    #[test]
    fn golden_base_browse_matches_fixture() {
        let leaf = reader_fetch_leaf(
            &payer_from_base_address(&BASE_ADDR),
            &markdown_hash("# fetched\n"),
            50_000,
            21_000_000,
        );
        assert_eq!(leaf, decode_hex("ce5109c69259d21e950896ffd6e6ed2e2ed036649c156bc5bd18e60de5c00655"));
    }

    #[test]
    fn amount_height_and_rail_each_move_the_digest() {
        let resource = markdown_hash("# fetched\n");
        let solana = reader_fetch_leaf(&SOLANA_PAYER, &resource, 5000, 444_956_973);
        assert_ne!(solana, reader_fetch_leaf(&SOLANA_PAYER, &resource, 50_000, 444_956_973));
        assert_ne!(solana, reader_fetch_leaf(&SOLANA_PAYER, &resource, 5000, 21_000_000));
        assert_ne!(solana, reader_fetch_leaf(&payer_from_base_address(&BASE_ADDR), &resource, 5000, 444_956_973));
    }

    #[test]
    fn fetch_digest_is_not_the_stripe_leaf() {
        let fetch = reader_fetch_leaf(&SOLANA_PAYER, &markdown_hash("# fetched\n"), 5000, 444_956_973);
        let stripe = crate::leaf_stripe::stripe_spt_leaf(
            b"cus_agent_spike_001",
            b"pi_3SpikeTest000000000000000",
            b"mcp://twzrd.intel/readiness_card",
            25,
            1_758_480_000,
        );
        assert_ne!(fetch, stripe);
        assert_eq!(READER_FETCH_LOG_ID, b"reader.fetch.v1");
    }
}
