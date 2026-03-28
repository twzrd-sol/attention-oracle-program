use crate::constants::{GLOBAL_V4_DOMAIN, GLOBAL_V5_DOMAIN};
use anchor_lang::prelude::Pubkey;
use sha3::{Digest, Keccak256};

pub fn keccak_hashv(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    for p in parts {
        hasher.update(p);
    }
    let out = hasher.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out[..32]);
    arr
}

pub fn verify_proof(proof: &[[u8; 32]], mut hash: [u8; 32], root: [u8; 32]) -> bool {
    if proof.len() > 32 {
        return false;
    }
    for sibling in proof.iter() {
        let (a, b) = if hash <= *sibling {
            (hash, *sibling)
        } else {
            (*sibling, hash)
        };
        hash = keccak_hashv(&[&a, &b]);
    }
    hash == root
}

/// Computes the simple global (v4) leaf hash — per-user totals, no channel scope:
/// keccak(domain || mint || root_seq || wallet || cumulative_total)
pub fn compute_global_leaf(
    mint: &Pubkey,
    root_seq: u64,
    wallet: &Pubkey,
    cumulative_total: u64,
) -> [u8; 32] {
    let seq = root_seq.to_le_bytes();
    let total = cumulative_total.to_le_bytes();
    keccak_hashv(&[
        GLOBAL_V4_DOMAIN,
        mint.as_ref(),
        &seq,
        wallet.as_ref(),
        &total,
    ])
}

/// Computes the v5 global leaf hash with decomposed reward components.
/// keccak(domain || mint || root_seq || wallet || base_yield || attention_bonus)
pub fn compute_global_leaf_v5(
    mint: &Pubkey,
    root_seq: u64,
    wallet: &Pubkey,
    base_yield: u64,
    attention_bonus: u64,
) -> [u8; 32] {
    keccak_hashv(&[
        GLOBAL_V5_DOMAIN,
        mint.as_ref(),
        &root_seq.to_le_bytes(),
        wallet.as_ref(),
        &base_yield.to_le_bytes(),
        &attention_bonus.to_le_bytes(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_proof_empty_proof_is_root() {
        let leaf = [42u8; 32];
        assert!(verify_proof(&[], leaf, leaf));
    }

    #[test]
    fn verify_proof_empty_proof_mismatch() {
        let leaf = [42u8; 32];
        let root = [0u8; 32];
        assert!(!verify_proof(&[], leaf, root));
    }

    #[test]
    fn verify_proof_rejects_oversized_proof() {
        let proof = vec![[0u8; 32]; 33];
        let leaf = [1u8; 32];
        let root = [1u8; 32];
        assert!(!verify_proof(&proof, leaf, root));
    }

    #[test]
    fn verify_proof_single_sibling() {
        let leaf = [1u8; 32];
        let sibling = [2u8; 32];
        // Compute expected root: hash(min(leaf, sibling), max(leaf, sibling))
        let (a, b) = if leaf <= sibling {
            (leaf, sibling)
        } else {
            (sibling, leaf)
        };
        let expected_root = keccak_hashv(&[&a, &b]);
        assert!(verify_proof(&[sibling], leaf, expected_root));
    }

    #[test]
    fn verify_proof_wrong_root_fails() {
        let leaf = [1u8; 32];
        let sibling = [2u8; 32];
        let wrong_root = [99u8; 32];
        assert!(!verify_proof(&[sibling], leaf, wrong_root));
    }

    #[test]
    fn compute_global_leaf_deterministic() {
        let mint = Pubkey::new_unique();
        let wallet = Pubkey::new_unique();
        let a = compute_global_leaf(&mint, 1, &wallet, 1000);
        let b = compute_global_leaf(&mint, 1, &wallet, 1000);
        assert_eq!(a, b);
    }

    #[test]
    fn compute_global_leaf_v5_deterministic() {
        let mint = Pubkey::new_unique();
        let wallet = Pubkey::new_unique();
        let a = compute_global_leaf_v5(&mint, 1, &wallet, 1000, 200);
        let b = compute_global_leaf_v5(&mint, 1, &wallet, 1000, 200);
        assert_eq!(a, b);
    }

    #[test]
    fn compute_global_leaf_different_inputs() {
        let mint = Pubkey::new_unique();
        let wallet = Pubkey::new_unique();
        let a = compute_global_leaf(&mint, 1, &wallet, 1000);
        let b = compute_global_leaf(&mint, 1, &wallet, 1001);
        assert_ne!(a, b);
    }

    #[test]
    fn compute_global_leaf_v5_different_inputs() {
        let mint = Pubkey::new_unique();
        let wallet = Pubkey::new_unique();
        let a = compute_global_leaf_v5(&mint, 1, &wallet, 1000, 200);
        let b = compute_global_leaf_v5(&mint, 1, &wallet, 1000, 201);
        assert_ne!(a, b);
    }

    #[test]
    fn compute_global_leaf_different_seq() {
        let mint = Pubkey::new_unique();
        let wallet = Pubkey::new_unique();
        let a = compute_global_leaf(&mint, 1, &wallet, 1000);
        let b = compute_global_leaf(&mint, 2, &wallet, 1000);
        assert_ne!(a, b);
    }
}
