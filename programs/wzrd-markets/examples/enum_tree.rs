//! Off-chain enum-tree merkle helper for continuous counterparty pricing.
//!
//! Builds the FULL-ENUMERATION resolution tree for a score-threshold market:
//! one leaf per possible score `s` in `0..=10_000` bps, each leaf committing
//! `observed_value = s` and `outcome = if s >= target_bps { YES } else { NO }`.
//! The market is created with this tree's root as `resolution_root`; at
//! resolution time the publisher proves the single realized leaf `s*`.
//!
//! ANTI-DRIFT MANDATE: this binary re-implements NOTHING. Every hash comes
//! from `wzrd_markets::resolution` — the exact module `resolve_market` runs
//! on-chain:
//!   * leaf bytes + hash:  `MarketsResolutionLeafV1::{new, hash}`
//!   * node hash:          `markets_resolution_node_hash_v1` (sorted-pair,
//!                          node-domain-separated keccak)
//!   * proof fold:         `compute_root_from_proof` (the verifier's own fold)
//!
//! TREE-SHAPE CONVENTION: identical to the repo's existing tree builder
//! (`programs/wzrd-rails/tests/listen_payout_e2e.rs::build_merkle_tree`):
//! pair left-to-right per level; an ODD trailing node is PROMOTED unchanged
//! to the next level (no padding, no duplication). The on-chain verifier is a
//! pure sibling fold, so the builder and verifier agree by construction — the
//! proof for any leaf folds to the root via `compute_root_from_proof`.
//!
//! Usage:
//!   cargo run -p wzrd-markets --example enum_tree
//!     [market_id] [counterparty_ref_hex(64 chars)] [window_id] [metric] [target_bps] [s_star]
//!   (no args = the sample market: 1, 0x22*32, 20260807, 3, 7000, s*=8500)

use wzrd_markets::resolution::{
    compute_root_from_proof, markets_resolution_node_hash_v1, outcome, MarketsResolutionLeafV1,
    MARKETS_MAX_PROOF_LEN, MARKETS_RESOLUTION_LEAF_SCHEMA_V1,
};

/// Golden vector from `resolution.rs::tests::fixture_leaf_hash_has_golden_vector`.
/// market_id=0x0102030405060708, streamer_ref=[0x51;32], window_id=20260622,
/// metric=1, observed_value=14752, outcome=YES.
const GOLDEN_FIXTURE_HASH: [u8; 32] = [
    118, 169, 238, 39, 126, 239, 202, 120, 158, 245, 146, 147, 228, 200, 204, 213, 70, 41, 204,
    249, 48, 196, 140, 154, 82, 159, 41, 99, 108, 67, 243, 58,
];

/// Full enum range: scores 0..=10000 bps → 10_001 leaves.
const MAX_SCORE_BPS: u64 = 10_000;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Build one enum leaf using resolution.rs's OWN constructor (which stamps
/// `schema_version = MARKETS_RESOLUTION_LEAF_SCHEMA_V1`).
fn enum_leaf(
    market_id: u64,
    counterparty_ref: [u8; 32],
    window_id: u64,
    metric: u8,
    target_bps: u64,
    s: u64,
) -> MarketsResolutionLeafV1 {
    let o = if s >= target_bps {
        outcome::YES
    } else {
        outcome::NO
    };
    MarketsResolutionLeafV1::new(market_id, counterparty_ref, window_id, metric, s, o)
}

/// Merkle tree over leaf hashes — the repo convention (mirrors
/// `listen_payout_e2e.rs::build_merkle_tree` byte-for-byte in structure,
/// with the MARKETS node hash): odd trailing node promotes unchanged.
/// Returns all levels (level 0 = leaves, last level = [root]).
fn build_levels(leaf_hashes: &[[u8; 32]]) -> Vec<Vec<[u8; 32]>> {
    assert!(!leaf_hashes.is_empty());
    let mut levels = vec![leaf_hashes.to_vec()];
    while levels.last().unwrap().len() > 1 {
        let previous = levels.last().unwrap();
        let mut next = Vec::with_capacity(previous.len().div_ceil(2));
        for pair in previous.chunks(2) {
            if pair.len() == 2 {
                next.push(markets_resolution_node_hash_v1(&pair[0], &pair[1]));
            } else {
                next.push(pair[0]); // odd node promotes unchanged
            }
        }
        levels.push(next);
    }
    levels
}

/// Sibling proof for `leaf_idx` — same walk as the rails e2e builder: at each
/// level take the pair sibling if it exists (promoted nodes contribute no
/// sibling at that level).
fn proof_for(levels: &[Vec<[u8; 32]>], leaf_idx: usize) -> Vec<[u8; 32]> {
    let mut idx = leaf_idx;
    let mut proof = Vec::new();
    for level in levels.iter().take(levels.len() - 1) {
        let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
        if let Some(sibling) = level.get(sibling_idx) {
            proof.push(*sibling);
        }
        idx /= 2;
    }
    proof
}

fn parse_ref(s: &str) -> [u8; 32] {
    let s = s.trim_start_matches("0x");
    assert_eq!(s.len(), 64, "counterparty_ref must be 64 hex chars");
    let mut out = [0u8; 32];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(chunk).unwrap(), 16)
            .expect("invalid hex in counterparty_ref");
    }
    out
}

fn main() {
    // ─── STEP 0: golden-vector proof that we are byte-identical to resolution.rs ───
    // We call resolution.rs's OWN `hash()` on the fixture inputs and require it to
    // equal the golden hash locked in resolution.rs's tests. If this ever fails,
    // the on-chain module itself drifted — refuse to build anything.
    let fixture = MarketsResolutionLeafV1::new(
        0x0102030405060708,
        [0x51; 32],
        20260622,
        1,      // metric
        14_752, // observed_value
        outcome::YES,
    );
    let fixture_hash = fixture.hash();
    assert_eq!(
        fixture_hash, GOLDEN_FIXTURE_HASH,
        "GOLDEN VECTOR MISMATCH — resolution.rs leaf hashing drifted; STOP"
    );
    assert_eq!(fixture.schema_version, MARKETS_RESOLUTION_LEAF_SCHEMA_V1);
    println!("[golden] fixture leaf hash : 0x{}", hex(&fixture_hash));
    println!(
        "[golden] resolution.rs golden: 0x{}",
        hex(&GOLDEN_FIXTURE_HASH)
    );
    println!("[golden] MATCH — leaf serialization + keccak are byte-identical to resolution.rs\n");

    // ─── args (defaults = the sample market) ───
    let args: Vec<String> = std::env::args().collect();
    let market_id: u64 = args.get(1).map_or(1, |v| v.parse().expect("market_id u64"));
    let counterparty_ref: [u8; 32] = args.get(2).map_or([0x22; 32], |v| parse_ref(v));
    let window_id: u64 = args
        .get(3)
        .map_or(20260807, |v| v.parse().expect("window_id u64"));
    let metric: u8 = args.get(4).map_or(3, |v| v.parse().expect("metric u8"));
    let target_bps: u64 = args
        .get(5)
        .map_or(7000, |v| v.parse().expect("target_bps u64"));
    let s_star: u64 = args.get(6).map_or(8500, |v| v.parse().expect("s_star u64"));
    assert!(target_bps <= MAX_SCORE_BPS, "target_bps out of range");
    assert!(s_star <= MAX_SCORE_BPS, "s_star out of range");

    println!("[market] market_id        = {market_id}");
    println!("[market] counterparty_ref = 0x{}", hex(&counterparty_ref));
    println!("[market] window_id        = {window_id}");
    println!("[market] metric           = {metric}");
    println!("[market] target_bps       = {target_bps} (outcome YES iff s >= target)");

    // ─── STEP 1: build all 10_001 enum leaves + hashes via resolution.rs ───
    let leaf_hashes: Vec<[u8; 32]> = (0..=MAX_SCORE_BPS)
        .map(|s| {
            enum_leaf(
                market_id,
                counterparty_ref,
                window_id,
                metric,
                target_bps,
                s,
            )
            .hash()
        })
        .collect();
    println!("[tree]   leaves           = {}", leaf_hashes.len());

    // ─── STEP 2: merkle root (repo odd-promote convention, markets node hash) ───
    let levels = build_levels(&leaf_hashes);
    let root = levels.last().unwrap()[0];
    println!(
        "[tree]   levels           = {} (max proof len {})",
        levels.len(),
        levels.len() - 1
    );
    println!("[tree]   resolution_root  = 0x{}\n", hex(&root));

    // ─── STEP 3: proof for the realized score s* ───
    let leaf_star = enum_leaf(
        market_id,
        counterparty_ref,
        window_id,
        metric,
        target_bps,
        s_star,
    );
    let leaf_star_hash = leaf_star.hash();
    let proof = proof_for(&levels, s_star as usize);
    assert!(
        proof.len() <= MARKETS_MAX_PROOF_LEN,
        "proof exceeds on-chain MARKETS_MAX_PROOF_LEN"
    );

    // ─── STEP 4: verify with resolution.rs's OWN verifier fold ───
    let computed = compute_root_from_proof(leaf_star_hash, &proof);
    assert_eq!(
        computed, root,
        "PROOF FAILED to fold to root via resolution.rs::compute_root_from_proof"
    );
    // Negative control: a tampered sibling must NOT fold to the root.
    if !proof.is_empty() {
        let mut tampered = proof.clone();
        tampered[0][0] ^= 1;
        assert_ne!(
            compute_root_from_proof(leaf_star_hash, &tampered),
            root,
            "tampered proof folded to root — convention broken"
        );
    }

    println!("[proof]  s*               = {s_star}");
    println!(
        "[proof]  outcome          = {} ({})",
        leaf_star.outcome,
        if leaf_star.outcome == outcome::YES {
            "YES"
        } else {
            "NO"
        }
    );
    println!("[proof]  leaf hash        = 0x{}", hex(&leaf_star_hash));
    println!(
        "[proof]  siblings         = {} (cap {})",
        proof.len(),
        MARKETS_MAX_PROOF_LEN
    );
    for (i, sib) in proof.iter().enumerate() {
        println!("[proof]  sibling[{i:>2}]      = 0x{}", hex(sib));
    }
    println!("[proof]  VERIFIED — compute_root_from_proof(leaf, proof) == resolution_root\n");

    // ─── caller-facing summary ───
    println!("== create_market arg ==");
    println!("resolution_root = 0x{}", hex(&root));
    println!("\n== resolve_market args (realized s* = {s_star}) ==");
    println!("window_id       = {window_id}");
    println!("observed_value  = {s_star}");
    println!("outcome         = {}", leaf_star.outcome);
    println!(
        "proof           = [{}]",
        proof
            .iter()
            .map(|s| format!("0x{}", hex(s)))
            .collect::<Vec<_>>()
            .join(", ")
    );
}
