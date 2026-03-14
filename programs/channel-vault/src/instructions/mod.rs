//! Instruction handlers for ChannelVault.

pub mod admin;
pub mod close;
pub mod compound;
pub mod deposit;
pub mod exchange_rate;
pub mod initialize;
pub mod metadata;
pub mod migrate_oracle_position;
pub mod redeem;
pub mod transfer_authority;

pub use admin::*;
pub use close::*;
pub use compound::*;
pub use deposit::*;
pub use exchange_rate::*;
pub use initialize::*;
pub use metadata::*;
pub use migrate_oracle_position::*;
pub use redeem::*;
pub use transfer_authority::*;
