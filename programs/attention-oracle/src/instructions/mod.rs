//! Instruction handlers for the AO v2 program.
//!
//! Each sub-module corresponds to a protocol domain implemented in the
//! Pinocchio rewrite.

pub mod admin;
pub mod global;
pub mod governance;
pub mod signal;
pub mod vault;
pub mod velocity_feed;

#[cfg(feature = "strategy")]
pub mod strategy;

#[cfg(feature = "channel_staking")]
pub mod channel_staking;

#[cfg(feature = "prediction_markets")]
pub mod markets;

#[cfg(feature = "price_feed")]
pub mod price_feed;
