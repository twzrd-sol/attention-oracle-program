#![cfg(feature = "localtest")]
//! reader.fetch.v1 encoder. Not an instruction.
//!
//! Run: cargo test --release --features localtest --test reader_fetch_leaf

use evidence_ledger::leaf_fetch::{
    reader_fetch_leaf, CLOCK_BLOCK_NUMBER, CLOCK_SLOT, READER_FETCH_LOG_ID,
};
use sha2::{Digest, Sha256};

fn markdown_hash(markdown: &[u8]) -> [u8; 32] {
    let dig = Sha256::digest(markdown);
    let mut out = [0u8; 32];
    out.copy_from_slice(&dig);
    out
}

fn hex_leaf(s: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap();
    }
    out
}

#[test]
fn fixture_leaf_is_stable() {
    let raw = include_str!("fixtures/reader_fetch_v1.json");
    let markdown = b"# example\n\npaid fetch body\n";
    assert!(raw.contains("# example\\n\\npaid fetch body\\n"));
    let resource = markdown_hash(markdown);
    let mut payer = [0u8; 32];
    payer[31] = 7;
    let leaf = reader_fetch_leaf(payer, resource, 5000, CLOCK_SLOT, 446_606_971);
    let expected = hex_leaf("b6e48bf0b8fda803cb60e262cc491ac5b3cb6edd97c806f236a32bcd56d60041");
    assert_eq!(leaf, expected);
    assert!(raw.contains("b6e48bf0b8fda803cb60e262cc491ac5b3cb6edd97c806f236a32bcd56d60041"));
    assert!(raw.contains("\"time_field\": \"slot\""));
    assert_eq!(READER_FETCH_LOG_ID, b"reader.fetch.v1");
}

#[test]
fn base_block_number_is_not_a_solana_slot() {
    let resource = markdown_hash(b"same body");
    let mut solana_payer = [0u8; 32];
    solana_payer[0] = 9;
    let base_payer = {
        let mut p = [0u8; 32];
        p[12..].copy_from_slice(&[
            0x14, 0xdf, 0x77, 0x2b, 0xd4, 0x96, 0xbb, 0xb7, 0xf4, 0x9b, 0xc3, 0xe9, 0x92, 0xce,
            0x13, 0xb2, 0xc4, 0x41, 0x17, 0x7f,
        ]);
        p
    };
    let slot_leaf = reader_fetch_leaf(solana_payer, resource, 5000, CLOCK_SLOT, 100);
    let block_leaf = reader_fetch_leaf(base_payer, resource, 5000, CLOCK_BLOCK_NUMBER, 100);
    assert_ne!(slot_leaf, block_leaf);
    assert_ne!(
        reader_fetch_leaf(base_payer, resource, 50_000, CLOCK_BLOCK_NUMBER, 100),
        block_leaf
    );
}
