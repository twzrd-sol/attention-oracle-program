//! Attention Oracle Rust SDK
//!
//! Type-safe Rust client for interacting with the Attention Oracle program.
//!
//! # Features
//!
//! - Type-safe instruction builders
//! - PDA derivation helpers
//! - Merkle proof verification
//! - Account deserialization
//!
//! # Example
//!
//! ```no_run
//! use attention_oracle_sdk::{AttentionOracleClient, PassportTier};
//! use solana_sdk::pubkey::Pubkey;
//!
//! let program_id = attention_oracle_sdk::ID;
//! let user = Pubkey::new_unique();
//! let (passport_pda, bump) = AttentionOracleClient::derive_passport_pda(&user, &program_id);
//! ```

use solana_program::pubkey::Pubkey;
use sha3::{Digest, Keccak256};
use thiserror::Error;

/// Attention Oracle program ID (mainnet)
pub const ID: Pubkey = solana_program::pubkey!("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");

pub mod instructions;
pub mod state;
pub mod utils;

pub use instructions::*;
pub use state::*;
pub use utils::*;

/// Passport tier levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PassportTier {
    Unverified = 0,
    Emerging = 1,
    Active = 2,
    Established = 3,
    Featured = 4,
    Elite = 5,
    Legendary = 6,
}

/// Errors
#[derive(Error, Debug)]
pub enum AttentionOracleError {
    #[error("Invalid merkle proof")]
    InvalidProof,

    #[error("PDA derivation failed")]
    PdaDerivationFailed,

    #[error("Account not found")]
    AccountNotFound,
}

/// Main client for Attention Oracle
pub struct AttentionOracleClient;

impl AttentionOracleClient {
    /// Derive passport PDA
    pub fn derive_passport_pda(user: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"passport", user.as_ref()], program_id)
    }

    /// Derive channel PDA
    pub fn derive_channel_pda(channel_id: &str, program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"channel", channel_id.as_bytes()], program_id)
    }

    /// Derive epoch PDA
    pub fn derive_epoch_pda(
        channel: &Pubkey,
        epoch_index: u32,
        program_id: &Pubkey,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[b"epoch", channel.as_ref(), &epoch_index.to_le_bytes()],
            program_id,
        )
    }

    /// Derive treasury PDA
    pub fn derive_treasury_pda(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"treasury"], program_id)
    }

    /// Derive creator pool PDA
    pub fn derive_creator_pool_pda(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"creator_pool"], program_id)
    }

    /// Compute merkle leaf hash
    pub fn compute_leaf(claimer: &Pubkey, index: u32, amount: u64, id: &str) -> [u8; 32] {
        let mut hasher = Keccak256::new();
        hasher.update(claimer.as_ref());
        hasher.update(&index.to_le_bytes());
        hasher.update(&amount.to_le_bytes());
        hasher.update(id.as_bytes());

        let result = hasher.finalize();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&result);
        arr
    }

    /// Verify merkle proof
    pub fn verify_proof(leaf: [u8; 32], proof: &[[u8; 32]], root: [u8; 32]) -> bool {
        let mut computed_hash = leaf;

        for proof_element in proof {
            computed_hash = if computed_hash < *proof_element {
                Self::hash_pair(&computed_hash, proof_element)
            } else {
                Self::hash_pair(proof_element, &computed_hash)
            };
        }

        computed_hash == root
    }

    fn hash_pair(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Keccak256::new();
        hasher.update(a);
        hasher.update(b);

        let result = hasher.finalize();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&result);
        arr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pda_derivation() {
        let program_id = ID;
        let user = Pubkey::new_unique();

        let (passport_pda, _bump) = AttentionOracleClient::derive_passport_pda(&user, &program_id);

        assert_ne!(passport_pda, user);
    }

    #[test]
    fn test_merkle_hash() {
        let user = Pubkey::new_unique();
        let leaf = AttentionOracleClient::compute_leaf(&user, 0, 1000, "test");

        assert_eq!(leaf.len(), 32);
    }
}
