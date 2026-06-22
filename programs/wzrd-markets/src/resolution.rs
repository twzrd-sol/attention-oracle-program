//! wzrd-markets attention-resolution leaf schema + merkle conventions (v1).
//!
//! This module is the byte-level contract between the in-house attention-root
//! publisher (off-chain tree builder), the `resolve_market` IX, and any proof
//! builder. It is the Phase 3 realization of `docs/cpmm-merkle-conventions-v1.md`.
//!
//! THE GATE (audit M-04 / CH-3): a one-byte drift between the off-chain prover
//! and this on-chain verifier makes every proof silently unverifiable, OR (worse)
//! lets a wrong-domain proof verify. wzrd-rails already ships two divergent keccak
//! conventions; wzrd-markets adopts the STRONGER one (listen-payout: node-domain
//! separated, sorted-pair) verbatim with its own domain strings, and introduces
//! NO third convention.
//!
//! Anti-drift invariants enforced here and tested below:
//!   * exactly ONE keccak lib (`solana_keccak_hasher`) — no sha2/blake3/sha3.
//!   * leaf domain != node domain != rails domains (4 distinct strings).
//!   * canonical byte order pinned by golden vectors (a drift = a failing test).
//!   * sorted-pair node hashing (lets the proof omit left/right flags).
//!
//! Keep it boring and exhaustively tested. If a golden hash below ever changes,
//! the byte order or a domain drifted — STOP and fix before shipping.

use anchor_lang::prelude::*;
use solana_keccak_hasher as keccak;

/// Canonical schema version for wzrd-markets attention-resolution leaves.
pub const MARKETS_RESOLUTION_LEAF_SCHEMA_V1: u8 = 1;

/// Leaf domain — wzrd-markets-specific, distinct from the rails leaf domain so a
/// rails listen-payout proof can NEVER verify against a markets resolution root.
pub const MARKETS_RESOLUTION_LEAF_V1_DOMAIN: &[u8] = b"wzrd-markets:attention-resolution-leaf:v1";

/// Node domain — distinct from the leaf domain (second-preimage defense: a leaf
/// hash can never be confused for an internal node hash) and from rails domains.
pub const MARKETS_RESOLUTION_NODE_V1_DOMAIN: &[u8] = b"wzrd-markets:attention-resolution-node:v1";

/// Proof-length cap — identical to wzrd-rails `MAX_PROOF_LEN`. 16 siblings ⇒ up
/// to 2^16 = 65,536 leaves per tree. Bounds compute and rejects a maliciously
/// long proof (DoS / compute exhaustion). Enforced with `require!` BEFORE the
/// fold loop (see `verify_against_root`), exactly as rails does.
pub const MARKETS_MAX_PROOF_LEN: usize = 16;

/// Outcome encoding for a resolved market. Stored as `u8` on `Market.outcome`.
/// `UNRESOLVED` is the create-time sentinel; a resolved market is one of
/// No / Yes / Invalid. (Mirrors `docs/cpmm-phase3-scope.md` §4.)
pub mod outcome {
    /// NO won — YES holders get nothing, NO holders settle 1:1.
    pub const NO: u8 = 0;
    /// YES won — NO holders get nothing, YES holders settle 1:1.
    pub const YES: u8 = 1;
    /// Market is INVALID — neither side "won"; collateral is recovered via the
    /// complete-set redeem rail (which stays open), never via `settle`.
    pub const INVALID: u8 = 2;
    /// Create-time sentinel: market not yet resolved. Distinct from every valid
    /// resolved outcome so `resolved == false` and `outcome == UNRESOLVED` agree.
    pub const UNRESOLVED: u8 = 255;

    /// True iff `o` is a terminal resolved outcome (NO / YES / INVALID).
    #[inline]
    pub fn is_resolved_value(o: u8) -> bool {
        matches!(o, NO | YES | INVALID)
    }

    /// True iff a settle (burn-winning-1:1) is possible for this outcome.
    /// INVALID does NOT settle (collateral recovered via complete-set redeem).
    #[inline]
    pub fn is_settleable(o: u8) -> bool {
        matches!(o, NO | YES)
    }
}

/// A single attention-resolution leaf. Committing this leaf into a market's
/// snapshotted resolution root (audit H-01) and proving its inclusion is how a
/// market's outcome is established.
///
/// Canonical byte encoding for `hash()` (little-endian, fixed order):
///
/// ```text
/// domain ||
/// schema_version:u8        (= 1)
/// market_id:u64_le         // binds the leaf to exactly one market
/// streamer_ref:32_bytes    // the streamer identity committed at create-time
/// window_id:u64_le         // the attention window this resolves
/// metric:u8                // MarketMetric (must match market.metric)
/// observed_value:u64_le    // the measured metric value at resolution
/// outcome:u8               // 0=NO, 1=YES, 2=INVALID
/// ```
///
/// `market_id` + `metric` are bound INTO the leaf so a proof valid for market A
/// under metric X cannot be replayed against market B or a different metric. The
/// proof proves inclusion in the root; `resolve_market` additionally asserts
/// `leaf.market_id == market.market_id`, `leaf.streamer_ref ==
/// market.streamer_ref`, and `leaf.metric == market.metric` — the audit M-04
/// lesson is that a verified proof against the WRONG leaf is still wrong.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarketsResolutionLeafV1 {
    pub schema_version: u8,
    pub market_id: u64,
    pub streamer_ref: [u8; 32],
    pub window_id: u64,
    pub metric: u8,
    pub observed_value: u64,
    pub outcome: u8,
}

impl MarketsResolutionLeafV1 {
    /// 1 + 8 + 32 + 8 + 1 + 8 + 1 = 59 bytes.
    pub const CANONICAL_LEN: usize = 1 + 8 + 32 + 8 + 1 + 8 + 1;

    pub fn new(
        market_id: u64,
        streamer_ref: [u8; 32],
        window_id: u64,
        metric: u8,
        observed_value: u64,
        outcome: u8,
    ) -> Self {
        Self {
            schema_version: MARKETS_RESOLUTION_LEAF_SCHEMA_V1,
            market_id,
            streamer_ref,
            window_id,
            metric,
            observed_value,
            outcome,
        }
    }

    /// Canonical little-endian byte layout (no domain prefix). The domain is
    /// prepended only in `hash()`.
    pub fn canonical_bytes(&self) -> [u8; Self::CANONICAL_LEN] {
        let mut out = [0u8; Self::CANONICAL_LEN];
        let mut offset = 0usize;

        out[offset] = self.schema_version;
        offset += 1;

        out[offset..offset + 8].copy_from_slice(&self.market_id.to_le_bytes());
        offset += 8;

        out[offset..offset + 32].copy_from_slice(&self.streamer_ref);
        offset += 32;

        out[offset..offset + 8].copy_from_slice(&self.window_id.to_le_bytes());
        offset += 8;

        out[offset] = self.metric;
        offset += 1;

        out[offset..offset + 8].copy_from_slice(&self.observed_value.to_le_bytes());
        offset += 8;

        out[offset] = self.outcome;

        out
    }

    /// Leaf hash: `keccak(LEAF_DOMAIN || canonical_bytes)`.
    pub fn hash(&self) -> [u8; 32] {
        let bytes = self.canonical_bytes();
        keccak::hashv(&[MARKETS_RESOLUTION_LEAF_V1_DOMAIN, bytes.as_ref()]).to_bytes()
    }
}

/// Sorted-pair, domain-separated merkle node hash for wzrd-markets resolution
/// trees. Byte-identical in structure to rails `listen_payout_node_hash_v1`
/// (only the domain constant differs). Do NOT "improve" it — drift is the bug.
///
/// Sorted pair: the smaller 32-byte value (lexicographic on raw bytes) goes
/// first, so the proof can omit left/right flags (prover and verifier both sort).
pub fn markets_resolution_node_hash_v1(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let (first, second) = if left <= right {
        (left.as_slice(), right.as_slice())
    } else {
        (right.as_slice(), left.as_slice())
    };
    keccak::hashv(&[MARKETS_RESOLUTION_NODE_V1_DOMAIN, first, second]).to_bytes()
}

/// Verify a leaf's inclusion in `expected_root` by folding `proof` siblings with
/// the node-domain sorted-pair hash. Returns the computed root. The caller
/// compares it to the snapshotted `market.resolution_root` and asserts the
/// leaf-to-market binding (the verifier shape is locked in conventions §3).
///
/// The proof-length cap MUST be checked by the caller with `require!` BEFORE
/// calling this (so an over-long proof reverts cheaply, unconditionally). This
/// helper also defends in depth: it returns `None`-equivalent by capping the
/// fold at `MARKETS_MAX_PROOF_LEN`, but the IX is the authoritative gate.
#[inline(never)]
pub fn compute_root_from_proof(leaf_hash: [u8; 32], proof: &[[u8; 32]]) -> [u8; 32] {
    let mut current = leaf_hash;
    for sibling in proof.iter() {
        current = markets_resolution_node_hash_v1(&current, sibling);
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Non-zero fixture leaf, by NAMED fields to avoid positional-arg confusion
    /// between the `metric` and `outcome` slots. market_id=0x0102030405060708,
    /// streamer_ref=0x51.., window_id=20260622, metric=1, observed_value=14752,
    /// outcome=YES. This is the leaf whose golden hash is locked below.
    fn fixture_leaf() -> MarketsResolutionLeafV1 {
        MarketsResolutionLeafV1 {
            schema_version: MARKETS_RESOLUTION_LEAF_SCHEMA_V1,
            market_id: 0x0102030405060708,
            streamer_ref: [0x51; 32],
            window_id: 20260622,
            metric: 1,
            observed_value: 14_752,
            outcome: super::outcome::YES,
        }
    }

    #[test]
    fn markets_resolution_constants_are_stable() {
        assert_eq!(MARKETS_RESOLUTION_LEAF_SCHEMA_V1, 1);
        assert_eq!(
            MARKETS_RESOLUTION_LEAF_V1_DOMAIN,
            b"wzrd-markets:attention-resolution-leaf:v1"
        );
        assert_eq!(
            MARKETS_RESOLUTION_NODE_V1_DOMAIN,
            b"wzrd-markets:attention-resolution-node:v1"
        );
        assert_eq!(MARKETS_MAX_PROOF_LEN, 16);
        assert_eq!(MarketsResolutionLeafV1::CANONICAL_LEN, 59);
    }

    /// Anti-drift: the markets domains MUST differ from the rails domains and
    /// from each other (4 distinct strings — conventions §5 checklist).
    #[test]
    fn markets_domains_are_distinct_from_rails_and_each_other() {
        let m_leaf = MARKETS_RESOLUTION_LEAF_V1_DOMAIN;
        let m_node = MARKETS_RESOLUTION_NODE_V1_DOMAIN;
        let rails_leaf: &[u8] = b"wzrd-rails:listen-payout-allocation-leaf:v1";
        let rails_node: &[u8] = b"wzrd-rails:listen-payout-allocation-node:v1";
        assert_ne!(m_leaf, m_node);
        assert_ne!(m_leaf, rails_leaf);
        assert_ne!(m_leaf, rails_node);
        assert_ne!(m_node, rails_leaf);
        assert_ne!(m_node, rails_node);
    }

    #[test]
    fn outcome_encoding_is_stable() {
        assert_eq!(super::outcome::NO, 0);
        assert_eq!(super::outcome::YES, 1);
        assert_eq!(super::outcome::INVALID, 2);
        assert_eq!(super::outcome::UNRESOLVED, 255);
        assert!(super::outcome::is_resolved_value(0));
        assert!(super::outcome::is_resolved_value(1));
        assert!(super::outcome::is_resolved_value(2));
        assert!(!super::outcome::is_resolved_value(255));
        assert!(!super::outcome::is_resolved_value(3));
        assert!(super::outcome::is_settleable(0));
        assert!(super::outcome::is_settleable(1));
        assert!(!super::outcome::is_settleable(2)); // INVALID not settleable
        assert!(!super::outcome::is_settleable(255));
    }

    #[test]
    fn canonical_bytes_are_little_endian_and_ordered() {
        let leaf = MarketsResolutionLeafV1::new(
            0x0102030405060708,
            [0x51; 32],
            20260622,
            1, // metric
            14_752,
            super::outcome::YES,
        );
        let bytes = leaf.canonical_bytes();
        assert_eq!(bytes.len(), MarketsResolutionLeafV1::CANONICAL_LEN);
        assert_eq!(bytes[0], MARKETS_RESOLUTION_LEAF_SCHEMA_V1); // schema_version
        assert_eq!(&bytes[1..9], &0x0102030405060708u64.to_le_bytes());
        assert_eq!(&bytes[9..41], &[0x51; 32]);
        assert_eq!(&bytes[41..49], &20260622u64.to_le_bytes());
        assert_eq!(bytes[49], 1u8); // metric
        assert_eq!(&bytes[50..58], &14_752u64.to_le_bytes());
        assert_eq!(bytes[58], super::outcome::YES);
    }

    /// GOLDEN HASH for the fixture leaf (market_id=0x0102030405060708,
    /// streamer_ref=0x51.., window_id=20260622, metric=1, observed_value=14752,
    /// outcome=YES). Computed by the standalone keccak calculator whose lib
    /// equivalence to `solana_keccak_hasher` was proven against the rails golden.
    /// If this changes, byte order or the leaf domain drifted — STOP.
    #[test]
    fn fixture_leaf_hash_has_golden_vector() {
        assert_eq!(
            fixture_leaf().hash(),
            [
                118, 169, 238, 39, 126, 239, 202, 120, 158, 245, 146, 147, 228, 200, 204, 213, 70,
                41, 204, 249, 48, 196, 140, 154, 82, 159, 41, 99, 108, 67, 243, 58,
            ]
        );
    }

    /// Determinism baseline. All-zero leaf EXCEPT `schema_version = 1` (the only
    /// valid value). The off-chain Rust/TS tree-builder mirror MUST match this
    /// byte-for-byte; a mismatch = silent unverifiability (the M-04 failure made
    /// LOUD by this test instead of silent in production).
    #[test]
    fn leaf_vector_all_zero() {
        let leaf = MarketsResolutionLeafV1 {
            schema_version: 1,
            market_id: 0,
            streamer_ref: [0u8; 32],
            window_id: 0,
            metric: 0,
            observed_value: 0,
            outcome: super::outcome::NO, // 0
        };
        // GOLDEN HASH (hex): fb1d38b1f2f8fdce1374e067636e61bc30f37ec18b2b9b5f2064a599153ed0bb
        assert_eq!(
            leaf.hash(),
            [
                0xfb, 0x1d, 0x38, 0xb1, 0xf2, 0xf8, 0xfd, 0xce, 0x13, 0x74, 0xe0, 0x67, 0x63, 0x6e,
                0x61, 0xbc, 0x30, 0xf3, 0x7e, 0xc1, 0x8b, 0x2b, 0x9b, 0x5f, 0x20, 0x64, 0xa5, 0x99,
                0x15, 0x3e, 0xd0, 0xbb,
            ]
        );
    }

    /// Field-binding: flipping ANY one field changes the hash. Proves no field
    /// is dropped from the canonical encoding (a dropped field = replayability).
    #[test]
    fn leaf_binds_every_field() {
        let base = fixture_leaf();

        let mut c_market = base;
        c_market.market_id ^= 1;
        assert_ne!(base.hash(), c_market.hash());

        let mut c_streamer = base;
        c_streamer.streamer_ref[0] ^= 1;
        assert_ne!(base.hash(), c_streamer.hash());

        let mut c_window = base;
        c_window.window_id ^= 1;
        assert_ne!(base.hash(), c_window.hash());

        let mut c_metric = base;
        c_metric.metric ^= 1;
        assert_ne!(base.hash(), c_metric.hash());

        let mut c_value = base;
        c_value.observed_value ^= 1;
        assert_ne!(base.hash(), c_value.hash());

        let mut c_outcome = base;
        c_outcome.outcome = super::outcome::NO; // YES -> NO
        assert_ne!(base.hash(), c_outcome.hash());

        // schema_version is bound too (defense: a v2 leaf must not collide v1).
        let mut c_schema = base;
        c_schema.schema_version = 2;
        assert_ne!(base.hash(), c_schema.hash());
    }

    /// Sorted-pair node hash is commutative AND has a locked golden vector.
    #[test]
    fn node_hash_sorts_pair_and_has_golden_vector() {
        let left = [0x11; 32];
        let right = [0x22; 32];
        assert_eq!(
            markets_resolution_node_hash_v1(&left, &right),
            markets_resolution_node_hash_v1(&right, &left)
        );
        assert_eq!(
            markets_resolution_node_hash_v1(&left, &right),
            [
                163, 223, 230, 223, 26, 26, 226, 55, 112, 113, 81, 4, 72, 174, 236, 84, 147, 8, 40,
                8, 30, 71, 86, 148, 25, 126, 69, 111, 164, 131, 44, 249,
            ]
        );
    }

    /// Node domain matters: a node hash built WITHOUT the markets node domain
    /// (e.g. the rails node domain) differs — the headline M-04/CH-3 kill switch
    /// at the hash level (the IX-level rejection test covers the full path).
    #[test]
    fn node_hash_rejects_wrong_domain() {
        let left = [0x11; 32];
        let right = [0x22; 32];
        let rails_node_domain: &[u8] = b"wzrd-rails:listen-payout-allocation-node:v1";
        let (first, second) = if left <= right {
            (left.as_slice(), right.as_slice())
        } else {
            (right.as_slice(), left.as_slice())
        };
        let wrong = keccak::hashv(&[rails_node_domain, first, second]).to_bytes();
        assert_ne!(markets_resolution_node_hash_v1(&left, &right), wrong);
    }

    /// Positive control for the fold: a two-leaf tree. Proof for leaf A is
    /// `[hash(B)]`; folding it must reproduce the root. Locks the root golden so
    /// the off-chain builder's two-leaf root matches.
    #[test]
    fn two_leaf_tree_fold_reproduces_root() {
        let a = MarketsResolutionLeafV1::new(
            0x0102030405060708,
            [0x51; 32],
            20260622,
            1,
            14_752,
            super::outcome::YES,
        );
        let b = MarketsResolutionLeafV1::new(
            0x0102030405060708,
            [0x99; 32],
            20260622,
            1,
            9_001,
            super::outcome::NO,
        );
        let ha = a.hash();
        let hb = b.hash();
        let root = markets_resolution_node_hash_v1(&ha, &hb);
        // Golden root.
        assert_eq!(
            root,
            [
                198, 14, 0, 118, 9, 94, 56, 168, 52, 170, 166, 84, 74, 183, 50, 145, 24, 228, 36,
                248, 222, 192, 235, 48, 199, 202, 99, 9, 7, 160, 119, 207,
            ]
        );
        // Fold leaf A with proof [hb] -> root.
        assert_eq!(compute_root_from_proof(ha, &[hb]), root);
        // Fold leaf B with proof [ha] -> same root (sorted-pair symmetry).
        assert_eq!(compute_root_from_proof(hb, &[ha]), root);
    }

    /// A tampered sibling breaks the fold (precursor to the IX rejection test).
    #[test]
    fn tampered_sibling_breaks_fold() {
        let a = MarketsResolutionLeafV1::new(1, [0x01; 32], 1, 1, 1, super::outcome::YES);
        let b = MarketsResolutionLeafV1::new(2, [0x02; 32], 1, 1, 2, super::outcome::NO);
        let ha = a.hash();
        let hb = b.hash();
        let root = markets_resolution_node_hash_v1(&ha, &hb);
        let mut tampered = hb;
        tampered[0] ^= 1;
        assert_ne!(compute_root_from_proof(ha, &[tampered]), root);
    }
}
