// Instruction modules for CCM Token-2022

pub mod admin;
pub mod channel;
pub mod claim;
pub mod cleanup;
pub mod cnft_verify;
pub mod extra_account_metas;
pub mod governance;
pub mod hooks;
pub mod initialize_mint;
pub mod liquidity;
pub mod merkle;
pub mod merkle_ring;
pub mod passport;
pub mod points;
pub mod push_distribute;

// Re-exports
pub use admin::*;
pub use channel::*;
pub use claim::*;
pub use cleanup::*;
pub use cnft_verify::*;
pub use extra_account_metas::*;
pub use governance::*;
pub use hooks::*;
pub use initialize_mint::*;
pub use liquidity::*;
pub use merkle::*;
pub use merkle_ring::*;
pub use passport::*;
pub use points::*;
pub use push_distribute::*;
