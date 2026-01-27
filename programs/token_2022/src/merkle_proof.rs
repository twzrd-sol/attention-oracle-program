use anchor_lang::prelude::Pubkey;
use crate::constants::CUMULATIVE_V2_DOMAIN;
use sha3::{Digest, Keccak256};

fn keccak_hashv(parts: &[&[u8]]) -> [u8; 32] {
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
    // SECURITY: Bounds check to prevent DoS via unbounded proof length
    // Maximum proof depth: 32 levels (2^32 leaves, ~4 billion - more than enough)
    if proof.len() > 32 {
        return false;
    }

    for sibling in proof.iter() {
        let (a, b) = if hash <= *sibling {
            (hash, *sibling)
        } else {
            (*sibling, hash)
        };
        // Pairwise keccak256 over sorted siblings.
        hash = keccak_hashv(&[&a, &b]);
    }
    hash == root
}

/// Computes the cumulative (v2) leaf hash:
/// keccak(domain || channel_cfg || mint || root_seq || wallet || cumulative_total)
pub fn compute_cumulative_leaf(
    channel_config: &Pubkey,
    mint: &Pubkey,
    root_seq: u64,
    wallet: &Pubkey,
    cumulative_total: u64,
) -> [u8; 32] {
    let seq = root_seq.to_le_bytes();
    let total = cumulative_total.to_le_bytes();
    keccak_hashv(&[
        CUMULATIVE_V2_DOMAIN,
        channel_config.as_ref(),
        mint.as_ref(),
        &seq,
        wallet.as_ref(),
        &total,
    ])
}
