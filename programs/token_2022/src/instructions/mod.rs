// Instruction modules for CCM Token-2022

pub mod admin;
pub mod channel;
pub mod cleanup;
pub mod cumulative;
pub mod creator;
pub mod governance;
pub mod initialize_mint;
pub mod passport;
pub mod push_distribute;
pub mod staking;
#[cfg(feature = "migration")]
pub mod migrate;

// Re-exports
pub use admin::*;
pub use channel::*;
pub use cleanup::*;
pub use cumulative::*;
pub use creator::*;
pub use governance::*;
pub use initialize_mint::*;
pub use passport::*;
pub use push_distribute::*;
pub use staking::*;
#[cfg(feature = "migration")]
pub use migrate::*;
