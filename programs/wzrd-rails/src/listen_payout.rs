//! Listen payout leaf schema.
//!
//! This module is the byte-level contract between the server payout worker,
//! the future `claim_listen_payout` IX, frontend proof builders, and public
//! indexers. Keep it boring and exhaustively tested: a one-byte drift here
//! makes every payout proof unverifiable.

use anchor_lang::prelude::*;
use solana_keccak_hasher as keccak;

/// Canonical schema version for Listen payout leaves.
pub const LISTEN_PAYOUT_LEAF_SCHEMA_V1: u8 = 1;

/// Domain separators for Listen payout hashes.
pub const LISTEN_PAYOUT_LEAF_V1_DOMAIN: &[u8] = b"wzrd-rails:listen-payout-leaf:v1";
pub const LISTEN_PAYOUT_NODE_V1_DOMAIN: &[u8] = b"wzrd-rails:listen-payout-node:v1";

/// A single claimable Listen payout leaf.
///
/// Canonical byte encoding for `hash()`:
///
/// ```text
/// domain ||
/// schema_version:u8 ||
/// window_id:u64_le ||
/// leaf_index:u32_le ||
/// session_id:16_bytes ||
/// wallet_pubkey:32_bytes ||
/// amount_ccm:u64_le ||
/// session_merkle_root:32_bytes ||
/// metadata_hash:32_bytes ||
/// salt:16_bytes
/// ```
///
/// Notes:
/// - `window_id` is a monotonic daily UTC id chosen by the server.
/// - `leaf_index` is bound into the hash so bitmap replay protection can key
///   by `(window_id, leaf_index)` without letting the same leaf claim multiple
///   indexes.
/// - `session_id` uses RFC 4122 UUID bytes, not a UTF-8 string.
/// - `amount_ccm` is in CCM Token-2022 base units.
/// - `session_merkle_root` is the finalized listening-session merkle root.
/// - `metadata_hash` commits to the public, canonical off-chain metadata JSON
///   served for indexers. Use all-zero only for test fixtures.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PayoutLeafV1 {
    pub schema_version: u8,
    pub window_id: u64,
    pub leaf_index: u32,
    pub session_id: [u8; 16],
    pub wallet_pubkey: Pubkey,
    pub amount_ccm: u64,
    pub session_merkle_root: [u8; 32],
    pub metadata_hash: [u8; 32],
    pub salt: [u8; 16],
}

impl PayoutLeafV1 {
    pub const CANONICAL_LEN: usize = 1 + 8 + 4 + 16 + 32 + 8 + 32 + 32 + 16;

    pub fn new(
        window_id: u64,
        leaf_index: u32,
        session_id: [u8; 16],
        wallet_pubkey: Pubkey,
        amount_ccm: u64,
        session_merkle_root: [u8; 32],
        metadata_hash: [u8; 32],
        salt: [u8; 16],
    ) -> Self {
        Self {
            schema_version: LISTEN_PAYOUT_LEAF_SCHEMA_V1,
            window_id,
            leaf_index,
            session_id,
            wallet_pubkey,
            amount_ccm,
            session_merkle_root,
            metadata_hash,
            salt,
        }
    }

    pub fn canonical_bytes(&self) -> [u8; Self::CANONICAL_LEN] {
        let mut out = [0u8; Self::CANONICAL_LEN];
        let mut offset = 0usize;

        out[offset] = self.schema_version;
        offset += 1;

        out[offset..offset + 8].copy_from_slice(&self.window_id.to_le_bytes());
        offset += 8;

        out[offset..offset + 4].copy_from_slice(&self.leaf_index.to_le_bytes());
        offset += 4;

        out[offset..offset + 16].copy_from_slice(&self.session_id);
        offset += 16;

        out[offset..offset + 32].copy_from_slice(self.wallet_pubkey.as_ref());
        offset += 32;

        out[offset..offset + 8].copy_from_slice(&self.amount_ccm.to_le_bytes());
        offset += 8;

        out[offset..offset + 32].copy_from_slice(&self.session_merkle_root);
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
        PayoutLeafV1::new(
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
        assert_eq!(LISTEN_PAYOUT_LEAF_SCHEMA_V1, 1);
        assert_eq!(
            LISTEN_PAYOUT_LEAF_V1_DOMAIN,
            b"wzrd-rails:listen-payout-leaf:v1"
        );
        assert_eq!(
            LISTEN_PAYOUT_NODE_V1_DOMAIN,
            b"wzrd-rails:listen-payout-node:v1"
        );
        assert_eq!(PayoutLeafV1::CANONICAL_LEN, 149);
    }

    #[test]
    fn payout_leaf_v1_canonical_bytes_are_little_endian_and_ordered() {
        let leaf = fixture_leaf();
        let bytes = leaf.canonical_bytes();
        assert_eq!(bytes.len(), PayoutLeafV1::CANONICAL_LEN);
        assert_eq!(bytes[0], LISTEN_PAYOUT_LEAF_SCHEMA_V1);
        assert_eq!(&bytes[1..9], &20260426u64.to_le_bytes());
        assert_eq!(&bytes[9..13], &7u32.to_le_bytes());
        assert_eq!(&bytes[13..29], &leaf.session_id);
        assert_eq!(&bytes[29..61], leaf.wallet_pubkey.as_ref());
        assert_eq!(&bytes[61..69], &42_000_000u64.to_le_bytes());
        assert_eq!(&bytes[69..101], &[0xaa; 32]);
        assert_eq!(&bytes[101..133], &[0xbb; 32]);
        assert_eq!(&bytes[133..149], &leaf.salt);
    }

    #[test]
    fn payout_leaf_v1_hash_has_golden_vector() {
        assert_eq!(
            fixture_leaf().hash(),
            [
                2, 89, 94, 76, 141, 232, 179, 31, 210, 49, 70, 250, 140, 120, 143, 70, 117, 137, 7,
                32, 54, 37, 151, 230, 228, 15, 114, 151, 166, 236, 33, 152,
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
        let leaf = PayoutLeafV1 {
            schema_version: 1,
            window_id: 0,
            leaf_index: 0,
            session_id: [0u8; 16],
            wallet_pubkey: Pubkey::new_from_array([0u8; 32]),
            amount_ccm: 0,
            session_merkle_root: [0u8; 32],
            metadata_hash: [0u8; 32],
            salt: [0u8; 16],
        };
        // GOLDEN HASH — locked 2026-04-26 across both repos.
        // wzrd-final/crates/types: vector_all_zero golden hash (hex):
        //   f6465e5c70c29a36b730f784ae207fb381b193c64d437234877e933020515280
        assert_eq!(
            leaf.hash(),
            [
                0xf6, 0x46, 0x5e, 0x5c, 0x70, 0xc2, 0x9a, 0x36, 0xb7, 0x30, 0xf7, 0x84, 0xae, 0x20,
                0x7f, 0xb3, 0x81, 0xb1, 0x93, 0xc6, 0x4d, 0x43, 0x72, 0x34, 0x87, 0x7e, 0x93, 0x30,
                0x20, 0x51, 0x52, 0x80,
            ]
        );
    }

    #[test]
    fn payout_leaf_v1_binds_replay_and_metadata_fields() {
        let base = fixture_leaf();

        let mut changed_index = base;
        changed_index.leaf_index += 1;
        assert_ne!(base.hash(), changed_index.hash());

        let mut changed_session_root = base;
        changed_session_root.session_merkle_root[0] ^= 1;
        assert_ne!(base.hash(), changed_session_root.hash());

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
                143, 89, 102, 153, 137, 122, 72, 169, 140, 139, 202, 40, 42, 37, 154, 40, 27, 219,
                222, 168, 52, 187, 16, 147, 249, 239, 116, 57, 220, 168, 108, 173,
            ]
        );
    }
}
