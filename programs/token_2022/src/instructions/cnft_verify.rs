use anchor_lang::prelude::*;
use sha3::{Digest, Keccak256};

/// Verify cNFT receipt from TWZRD L1
/// Production implementation would integrate with mpl-bubblegum
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CnftReceiptProof {
    /// Leaf owner (must match claimer)
    pub owner: Pubkey,
    /// Leaf delegate (usually same as owner)
    pub delegate: Pubkey,
    /// Leaf index in tree
    pub leaf_index: u32,
    /// Merkle proof
    pub proof: Vec<[u8; 32]>,
    /// Metadata hash (for verification)
    pub metadata_hash: [u8; 32],
}

/// Verify cNFT receipt ownership and metadata
pub fn verify_cnft_receipt(
    receipt_proof: &CnftReceiptProof,
    claimer: &Pubkey,
    expected_channel: &str,
    expected_epoch: u64,
) -> Result<()> {
    use crate::errors::OracleError;

    // Step 1: Verify ownership
    require!(receipt_proof.owner == *claimer, OracleError::InvalidProof);

    // Step 2: Verify metadata hash matches expected channel/epoch
    let expected_hash = compute_metadata_hash(expected_channel, expected_epoch);
    require!(
        receipt_proof.metadata_hash == expected_hash,
        OracleError::InvalidProof
    );

    msg!(
        "cNFT verified: owner={}, channel={}, epoch={}",
        claimer,
        expected_channel,
        expected_epoch
    );

    Ok(())
}

/// Compute metadata hash for cNFT verification
/// Hash = keccak256("twzrd:" || channel || ":" || epoch)
fn compute_metadata_hash(channel: &str, epoch: u64) -> [u8; 32] {
    let mut preimage = Vec::new();
    preimage.extend_from_slice(b"twzrd:");
    preimage.extend_from_slice(channel.as_bytes());
    preimage.extend_from_slice(b":");
    preimage.extend_from_slice(&epoch.to_le_bytes());
    let mut hasher = Keccak256::new();
    hasher.update(&preimage);
    let out = hasher.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out[..32]);
    arr
}

/// Verify merkle proof (used for Bubblegum tree verification)
/// This is a simplified version - production should use mpl-bubblegum CPI
pub fn verify_merkle_proof(leaf: &[u8; 32], proof: &[[u8; 32]], root: &[u8; 32]) -> bool {
    let mut current = *leaf;

    for sibling in proof {
        // Sorted pair hashing
        let (first, second) = if current <= *sibling {
            (current, *sibling)
        } else {
            (*sibling, current)
        };
        let mut hasher = Keccak256::new();
        hasher.update(&first);
        hasher.update(&second);
        let out = hasher.finalize();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&out[..32]);
        current = arr;
    }

    current == *root
}

// CnftError moved to main errors.rs module to avoid duplicate error_code macro

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_hash() {
        let hash1 = compute_metadata_hash("xqc", 12345);
        let hash2 = compute_metadata_hash("xqc", 12345);
        let hash3 = compute_metadata_hash("lacy", 12345);

        // Same inputs = same hash
        assert_eq!(hash1, hash2);
        // Different inputs = different hash
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_merkle_proof() {
        // Simple 2-leaf tree
        let leaf1 = [1u8; 32];
        let leaf2 = [2u8; 32];

        let (first, second) = if leaf1 <= leaf2 {
            (leaf1, leaf2)
        } else {
            (leaf2, leaf1)
        };

        let mut combined = Vec::new();
        combined.extend_from_slice(&first);
        combined.extend_from_slice(&second);
        let root = keccak::hash(&combined).to_bytes();

        // Verify leaf1 with leaf2 as sibling
        let proof = vec![leaf2];
        assert!(verify_merkle_proof(&leaf1, &proof, &root));

        // Invalid proof
        let bad_proof = vec![[0u8; 32]];
        assert!(!verify_merkle_proof(&leaf1, &bad_proof, &root));
    }
}
