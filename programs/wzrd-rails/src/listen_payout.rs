//! Listen payout allocation leaf schema.
//!
//! This module is the byte-level contract between the server pool-allocation
//! worker, the `claim_listen_payout` IX, frontend proof builders, and public
//! indexers. Keep it boring and exhaustively tested: a one-byte drift here
//! makes every payout proof unverifiable.

use anchor_lang::prelude::*;
use solana_keccak_hasher as keccak;

/// Canonical schema version for Listen payout allocation leaves.
pub const LISTEN_PAYOUT_ALLOCATION_LEAF_SCHEMA_V1: u8 = 1;

/// Backwards-compatible constant name used by P1.2/P1.3 account args.
pub const LISTEN_PAYOUT_LEAF_SCHEMA_V1: u8 = LISTEN_PAYOUT_ALLOCATION_LEAF_SCHEMA_V1;

/// Domain separators for Listen payout hashes.
pub const LISTEN_PAYOUT_ALLOCATION_LEAF_V1_DOMAIN: &[u8] =
    b"wzrd-rails:listen-payout-allocation-leaf:v1";
pub const LISTEN_PAYOUT_ALLOCATION_NODE_V1_DOMAIN: &[u8] =
    b"wzrd-rails:listen-payout-allocation-node:v1";

/// Backwards-compatible constant names used by existing helpers/tests.
pub const LISTEN_PAYOUT_LEAF_V1_DOMAIN: &[u8] = LISTEN_PAYOUT_ALLOCATION_LEAF_V1_DOMAIN;
pub const LISTEN_PAYOUT_NODE_V1_DOMAIN: &[u8] = LISTEN_PAYOUT_ALLOCATION_NODE_V1_DOMAIN;

/// A single claimable Listen payout allocation leaf.
///
/// This leaf represents scarce funded inventory allocated to a wallet from a
/// payout pool. It is not a direct reward for one listening session. The
/// listening sessions that justify the allocation are committed through
/// `supporting_session_ids_root`.
///
/// Canonical byte encoding for `hash()`:
///
/// ```text
/// domain ||
/// schema_version:u8 ||
/// pool_id:32_bytes ||
/// window_id:u64_le ||
/// leaf_index:u32_le ||
/// allocation_id:16_bytes ||
/// wallet_pubkey:32_bytes ||
/// amount_ccm:u64_le ||
/// supporting_session_ids_root:32_bytes ||
/// metadata_hash:32_bytes ||
/// salt:16_bytes
/// ```
///
/// Notes:
/// - `pool_id` identifies the funded reward pool that owns this inventory.
/// - `window_id` is a monotonic daily UTC id chosen by the server.
/// - `leaf_index` is bound into the hash so bitmap replay protection can key
///   by `(window_id, leaf_index)` without letting the same leaf claim multiple
///   indexes.
/// - `allocation_id` is the server-side allocation UUID bytes, not a UTF-8
///   string.
/// - `amount_ccm` is in CCM Token-2022 base units.
/// - `supporting_session_ids_root` commits to the listen arcs/sessions used to
///   justify the allocation. It does not make those sessions directly payable.
/// - `metadata_hash` commits to the public, canonical off-chain metadata JSON
///   served for indexers. Use all-zero only for test fixtures.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PayoutAllocationLeafV1 {
    pub schema_version: u8,
    pub pool_id: [u8; 32],
    pub window_id: u64,
    pub leaf_index: u32,
    pub allocation_id: [u8; 16],
    pub wallet_pubkey: Pubkey,
    pub amount_ccm: u64,
    pub supporting_session_ids_root: [u8; 32],
    pub metadata_hash: [u8; 32],
    pub salt: [u8; 16],
}

impl PayoutAllocationLeafV1 {
    pub const CANONICAL_LEN: usize = 1 + 32 + 8 + 4 + 16 + 32 + 8 + 32 + 32 + 16;

    pub fn new(
        pool_id: [u8; 32],
        window_id: u64,
        leaf_index: u32,
        allocation_id: [u8; 16],
        wallet_pubkey: Pubkey,
        amount_ccm: u64,
        supporting_session_ids_root: [u8; 32],
        metadata_hash: [u8; 32],
        salt: [u8; 16],
    ) -> Self {
        Self {
            schema_version: LISTEN_PAYOUT_ALLOCATION_LEAF_SCHEMA_V1,
            pool_id,
            window_id,
            leaf_index,
            allocation_id,
            wallet_pubkey,
            amount_ccm,
            supporting_session_ids_root,
            metadata_hash,
            salt,
        }
    }

    pub fn canonical_bytes(&self) -> [u8; Self::CANONICAL_LEN] {
        let mut out = [0u8; Self::CANONICAL_LEN];
        let mut offset = 0usize;

        out[offset] = self.schema_version;
        offset += 1;

        out[offset..offset + 32].copy_from_slice(&self.pool_id);
        offset += 32;

        out[offset..offset + 8].copy_from_slice(&self.window_id.to_le_bytes());
        offset += 8;

        out[offset..offset + 4].copy_from_slice(&self.leaf_index.to_le_bytes());
        offset += 4;

        out[offset..offset + 16].copy_from_slice(&self.allocation_id);
        offset += 16;

        out[offset..offset + 32].copy_from_slice(self.wallet_pubkey.as_ref());
        offset += 32;

        out[offset..offset + 8].copy_from_slice(&self.amount_ccm.to_le_bytes());
        offset += 8;

        out[offset..offset + 32].copy_from_slice(&self.supporting_session_ids_root);
        offset += 32;

        out[offset..offset + 32].copy_from_slice(&self.metadata_hash);
        offset += 32;

        out[offset..offset + 16].copy_from_slice(&self.salt);

        out
    }

    pub fn hash(&self) -> [u8; 32] {
        let bytes = self.canonical_bytes();
        keccak::hashv(&[LISTEN_PAYOUT_LEAF_V1_DOMAIN, bytes.as_ref()]).to_bytes()
    }
}

/// Backwards-compatible alias for P1.2/P1.3 callers while the instruction
/// names remain `publish_listen_payout_root` and `claim_listen_payout`.
pub type PayoutLeafV1 = PayoutAllocationLeafV1;

/// Sorted-pair merkle node hash for Listen payout trees.
pub fn listen_payout_node_hash_v1(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let (first, second) = if left <= right {
        (left.as_slice(), right.as_slice())
    } else {
        (right.as_slice(), left.as_slice())
    };
    keccak::hashv(&[LISTEN_PAYOUT_NODE_V1_DOMAIN, first, second]).to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_leaf() -> PayoutLeafV1 {
        PayoutAllocationLeafV1::new(
            [0x51; 32],
            20260426,
            7,
            [
                0x5f, 0x04, 0x3b, 0x37, 0xe9, 0xce, 0x4e, 0xbc, 0x8d, 0xd4, 0x3a, 0x20, 0x5b, 0x7d,
                0x8f, 0xdb,
            ],
            Pubkey::new_from_array([0x87; 32]),
            42_000_000,
            [0xaa; 32],
            [0xbb; 32],
            [
                0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe, 0xef, 0xcd, 0xab, 0x89, 0x67, 0x45,
                0x23, 0x01,
            ],
        )
    }

    #[test]
    fn payout_leaf_v1_constants_are_stable() {
        assert_eq!(LISTEN_PAYOUT_ALLOCATION_LEAF_SCHEMA_V1, 1);
        assert_eq!(
            LISTEN_PAYOUT_LEAF_V1_DOMAIN,
            b"wzrd-rails:listen-payout-allocation-leaf:v1"
        );
        assert_eq!(
            LISTEN_PAYOUT_NODE_V1_DOMAIN,
            b"wzrd-rails:listen-payout-allocation-node:v1"
        );
        assert_eq!(PayoutAllocationLeafV1::CANONICAL_LEN, 181);
    }

    #[test]
    fn payout_leaf_v1_canonical_bytes_are_little_endian_and_ordered() {
        let leaf = fixture_leaf();
        let bytes = leaf.canonical_bytes();
        assert_eq!(bytes.len(), PayoutAllocationLeafV1::CANONICAL_LEN);
        assert_eq!(bytes[0], LISTEN_PAYOUT_ALLOCATION_LEAF_SCHEMA_V1);
        assert_eq!(&bytes[1..33], &[0x51; 32]);
        assert_eq!(&bytes[33..41], &20260426u64.to_le_bytes());
        assert_eq!(&bytes[41..45], &7u32.to_le_bytes());
        assert_eq!(&bytes[45..61], &leaf.allocation_id);
        assert_eq!(&bytes[61..93], leaf.wallet_pubkey.as_ref());
        assert_eq!(&bytes[93..101], &42_000_000u64.to_le_bytes());
        assert_eq!(&bytes[101..133], &[0xaa; 32]);
        assert_eq!(&bytes[133..165], &[0xbb; 32]);
        assert_eq!(&bytes[165..181], &leaf.salt);
    }

    #[test]
    fn payout_leaf_v1_hash_has_golden_vector() {
        assert_eq!(
            fixture_leaf().hash(),
            [
                178, 31, 103, 10, 88, 220, 121, 170, 11, 115, 75, 253, 25, 64, 86, 247, 146, 38,
                217, 223, 66, 78, 43, 197, 183, 95, 41, 33, 39, 117, 4, 92,
            ]
        );
    }

    /// Determinism baseline. All-zero leaf produces a stable hash that
    /// the off-chain Rust mirror in `wzrd-final/crates/types` MUST match
    /// byte-for-byte. If this hash changes, the canonical byte order or
    /// domain separator drifted somewhere — fix before shipping.
    ///
    /// Mirrors `wzrd_types::listen::tests::vector_all_zero` (PR #215 in
    /// twzrd-sol/wzrd-final).
    #[test]
    fn payout_leaf_v1_vector_all_zero() {
        let leaf = PayoutAllocationLeafV1 {
            schema_version: 1,
            pool_id: [0u8; 32],
            window_id: 0,
            leaf_index: 0,
            allocation_id: [0u8; 16],
            wallet_pubkey: Pubkey::new_from_array([0u8; 32]),
            amount_ccm: 0,
            supporting_session_ids_root: [0u8; 32],
            metadata_hash: [0u8; 32],
            salt: [0u8; 16],
        };
        // GOLDEN HASH — locked 2026-04-26 allocation-leaf supersession across both repos.
        // wzrd-final/crates/types: vector_all_zero golden hash (hex):
        //   b59898f0d710f4a0460bd21102e42d40470289c63a3f9230bc9938ab62e6d5f5
        assert_eq!(
            leaf.hash(),
            [
                0xb5, 0x98, 0x98, 0xf0, 0xd7, 0x10, 0xf4, 0xa0, 0x46, 0x0b, 0xd2, 0x11, 0x02, 0xe4,
                0x2d, 0x40, 0x47, 0x02, 0x89, 0xc6, 0x3a, 0x3f, 0x92, 0x30, 0xbc, 0x99, 0x38, 0xab,
                0x62, 0xe6, 0xd5, 0xf5,
            ]
        );
    }

    #[test]
    fn payout_leaf_v1_binds_replay_and_metadata_fields() {
        let base = fixture_leaf();

        let mut changed_pool = base;
        changed_pool.pool_id[0] ^= 1;
        assert_ne!(base.hash(), changed_pool.hash());

        let mut changed_index = base;
        changed_index.leaf_index += 1;
        assert_ne!(base.hash(), changed_index.hash());

        let mut changed_allocation = base;
        changed_allocation.allocation_id[0] ^= 1;
        assert_ne!(base.hash(), changed_allocation.hash());

        let mut changed_supporting_sessions = base;
        changed_supporting_sessions.supporting_session_ids_root[0] ^= 1;
        assert_ne!(base.hash(), changed_supporting_sessions.hash());

        let mut changed_metadata = base;
        changed_metadata.metadata_hash[0] ^= 1;
        assert_ne!(base.hash(), changed_metadata.hash());
    }

    #[test]
    fn listen_payout_node_hash_v1_sorts_pair_inputs() {
        let left = [0x11; 32];
        let right = [0x22; 32];
        assert_eq!(
            listen_payout_node_hash_v1(&left, &right),
            listen_payout_node_hash_v1(&right, &left)
        );
        assert_eq!(
            listen_payout_node_hash_v1(&left, &right),
            [
                220, 194, 170, 23, 24, 189, 186, 44, 81, 215, 161, 75, 90, 191, 16, 246, 128, 31,
                244, 82, 199, 91, 117, 114, 158, 59, 190, 90, 115, 216, 234, 184,
            ]
        );
    }
}
