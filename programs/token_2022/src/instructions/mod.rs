//! Instruction modules for the Attention Oracle Protocol.

pub mod admin;
pub mod channel;
pub mod cumulative;
pub mod global;
pub mod governance;
pub mod initialize_mint;
pub mod markets;
pub mod staking;

// Re-exports
pub use admin::*;
pub use channel::*;
pub use cumulative::*;
pub use global::*;
pub use governance::*;
pub use initialize_mint::*;
pub use markets::*;
pub use staking::*;
