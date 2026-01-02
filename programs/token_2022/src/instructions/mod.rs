//! Instruction modules for the Attention Oracle Protocol.

pub mod admin;
pub mod channel;
pub mod cumulative;
pub mod governance;
pub mod initialize_mint;
pub mod passport;
pub mod staking;

#[cfg(feature = "migration")]
pub mod migrate;

// Re-exports
pub use admin::*;
pub use channel::*;
pub use cumulative::*;
pub use governance::*;
pub use initialize_mint::*;
pub use passport::*;
pub use staking::*;

#[cfg(feature = "migration")]
pub use migrate::*;
