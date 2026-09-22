#![cfg(feature = "localtest")]
//! reader.fetch.v1 encoder against the checked-in fixture.
//!
//! Run: cargo test --release --features localtest --test reader_fetch_leaf

use evidence_ledger::leaf_fetch::{payer_from_base_address, reader_fetch_leaf, READER_FETCH_LOG_ID};
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
fn fixture_cases_match_the_shipped_encoder() {
    let raw = include_str!("fixtures/reader_fetch_v1.json");
    assert!(raw.contains("\"log_id\": \"reader.fetch.v1\""));
    assert!(raw.contains("# fetched\\n"));
    let resource = markdown_hash(b"# fetched\n");
    let solana = [0x11u8; 32];
    let base = payer_from_base_address(&[
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff, 0x00, 0x11, 0x22, 0x33,
    ]);
    let scrape = reader_fetch_leaf(&solana, &resource, 5000, 444_956_973);
    let browse = reader_fetch_leaf(&base, &resource, 50_000, 21_000_000);
    assert_eq!(
        scrape,
        hex_leaf("7d65918d4d4f4174964914ad9eb4922818131e0613340b9e07adb32956dddd92")
    );
    assert_eq!(
        browse,
        hex_leaf("ce5109c69259d21e950896ffd6e6ed2e2ed036649c156bc5bd18e60de5c00655")
    );
    assert!(raw.contains("7d65918d4d4f4174964914ad9eb4922818131e0613340b9e07adb32956dddd92"));
    assert!(raw.contains("ce5109c69259d21e950896ffd6e6ed2e2ed036649c156bc5bd18e60de5c00655"));
    assert_ne!(scrape, browse);
    assert_eq!(READER_FETCH_LOG_ID, b"reader.fetch.v1");
}
