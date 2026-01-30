//! Instruction handlers for ChannelVault.

pub mod admin;
pub mod close;
pub mod compound;
pub mod deposit;
pub mod initialize;
pub mod metadata;
pub mod redeem;

pub use admin::*;
pub use close::*;
pub use compound::*;
pub use deposit::*;
pub use initialize::*;
pub use metadata::*;
pub use redeem::*;
