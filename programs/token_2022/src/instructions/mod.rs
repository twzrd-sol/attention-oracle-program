// Instruction modules for CCM Token-2022

pub mod admin;
pub mod channel;
pub mod cumulative;
pub mod claim_sponsored;
pub mod cleanup;
pub mod creator;
pub mod governance;
pub mod initialize_mint;
pub mod migrate_channel;
pub mod passport;
pub mod push_distribute;
pub mod resize_channel;
pub mod staking;
#[cfg(feature = "migration")]
pub mod migrate;

// Re-exports
pub use admin::*;
pub use channel::*;
pub use cumulative::*;
pub use claim_sponsored::*;
pub use cleanup::*;
pub use creator::*;
pub use governance::*;
pub use initialize_mint::*;
pub use migrate_channel::*;
pub use passport::*;
pub use push_distribute::*;
pub use resize_channel::*;
pub use staking::*;
#[cfg(feature = "migration")]
pub use migrate::*;
