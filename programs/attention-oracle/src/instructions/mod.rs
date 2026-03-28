//! Instruction modules for the Liquid Attention Protocol.

pub mod admin;
pub mod global;
pub mod governance;
#[cfg(feature = "prediction_markets")]
pub mod markets;
#[cfg(feature = "price_feed")]
pub mod price_feed;
#[cfg(feature = "channel_staking")]
pub mod staking;
#[cfg(feature = "strategy")]
pub mod strategy;
pub mod vault;

pub use admin::*;
pub use global::*;
pub use governance::*;
#[cfg(feature = "prediction_markets")]
pub use markets::*;
#[cfg(feature = "price_feed")]
pub use price_feed::*;
#[cfg(feature = "channel_staking")]
pub use staking::*;
#[cfg(feature = "strategy")]
pub use strategy::*;
pub use vault::*;
