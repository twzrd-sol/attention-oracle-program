//! Instruction handlers for the AO v2 program.
//!
//! Each sub-module corresponds to a protocol domain implemented in the
//! Pinocchio rewrite.

pub mod vault;
pub mod global;
pub mod governance;
pub mod admin;

#[cfg(feature = "strategy")]
pub mod strategy;

#[cfg(feature = "channel_staking")]
pub mod channel_staking;

#[cfg(feature = "prediction_markets")]
pub mod markets;

#[cfg(feature = "price_feed")]
pub mod price_feed;
