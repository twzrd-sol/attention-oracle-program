//! Instruction modules for the Attention Oracle Protocol.

pub mod admin;
pub mod channel;
pub mod channel_staking;
pub mod cumulative;
pub mod governance;
pub mod initialize_mint;

// Re-exports
pub use admin::*;
pub use channel::*;
pub use channel_staking::*;
pub use cumulative::*;
pub use governance::*;
pub use initialize_mint::*;
