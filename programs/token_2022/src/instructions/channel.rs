//! Channel utilities for the Attention Oracle Protocol.

use anchor_lang::prelude::*;

use crate::merkle_proof::keccak_hashv;

/// Derive a stable subject_id from channel name (lowercase, prefixed with "channel:").
/// Used for PDA derivation across cumulative claims and channel configs.
pub fn derive_subject_id(channel: &str) -> Pubkey {
    let mut lower = channel.as_bytes().to_vec();
    lower.iter_mut().for_each(|b| *b = b.to_ascii_lowercase());
    let hash = keccak_hashv(&[b"channel:", lower.as_slice()]);
    Pubkey::new_from_array(hash)
}
