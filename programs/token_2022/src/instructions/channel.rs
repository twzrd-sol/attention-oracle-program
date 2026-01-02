//! Channel utilities for the Attention Oracle Protocol.

use anchor_lang::prelude::*;
use sha3::{Digest, Keccak256};

/// Derive a stable subject_id from channel name (lowercase, prefixed with "channel:").
/// Used for PDA derivation across cumulative claims and channel configs.
pub fn derive_subject_id(channel: &str) -> Pubkey {
    let mut lower = channel.as_bytes().to_vec();
    lower.iter_mut().for_each(|b| *b = b.to_ascii_lowercase());
    let hash = keccak_hashv(&[b"channel:", lower.as_slice()]);
    Pubkey::new_from_array(hash)
}

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
