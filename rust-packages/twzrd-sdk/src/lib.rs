//! # twzrd-sdk
//!
//! Rust SDK for **TWZRD Attention Oracle** on Solana.
//!
//! Open-core Solana primitive for tokenized attention.
//! Presence → Proof → Tokens.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use twzrd_sdk::{TwzrdClient, PROGRAM_ID};
//! use solana_sdk::pubkey::Pubkey;
//!
//! let client = TwzrdClient::new(rpc_url);
//! let channel_state = client.get_channel_state(&streamer_pubkey).await?;
//! ```

use solana_sdk::pubkey::Pubkey;
use thiserror::Error;

/// TWZRD Program ID (placeholder - update with actual program ID)
pub const PROGRAM_ID: Pubkey = solana_sdk::pubkey!("11111111111111111111111111111111");

#[derive(Error, Debug)]
pub enum TwzrdError {
    #[error("RPC client error: {0}")]
    RpcError(String),
    #[error("Account not found")]
    AccountNotFound,
    #[error("Invalid channel state")]
    InvalidChannelState,
}

/// Main client for interacting with TWZRD Attention Oracle
pub struct TwzrdClient {
    // TODO: Add RPC client
}

impl TwzrdClient {
    /// Create a new TWZRD client
    pub fn new(rpc_url: &str) -> Self {
        Self {}
    }

    /// Get channel state for a given streamer
    pub async fn get_channel_state(&self, _streamer: &Pubkey) -> Result<ChannelState, TwzrdError> {
        todo!("Implement channel state fetching")
    }
}

/// Channel state account data
#[derive(Debug, Clone)]
pub struct ChannelState {
    pub streamer: Pubkey,
    pub mint: Pubkey,
    pub current_epoch: u64,
    pub total_minted: u64,
    pub active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_program_id() {
        assert_eq!(PROGRAM_ID.to_string().len(), 44);
    }
}
