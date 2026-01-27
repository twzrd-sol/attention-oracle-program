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

pub fn compute_leaf(claimer: &Pubkey, index: u32, amount: u64, id: &str) -> [u8; 32] {
    // NOTE: Off-chain must mirror this exact hashing scheme.
    let idx = index.to_le_bytes();
    let amt = amount.to_le_bytes();
    let id_bytes = id.as_bytes();
    keccak_hashv(&[claimer.as_ref(), &idx, &amt, id_bytes])
}

pub fn verify_proof(proof: &[[u8; 32]], mut hash: [u8; 32], root: [u8; 32]) -> bool {
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
