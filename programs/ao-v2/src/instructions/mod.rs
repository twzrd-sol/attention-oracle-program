//! Instruction handlers for the AO v2 program.
//!
//! Each sub-module corresponds to a protocol domain implemented in the
//! Pinocchio rewrite.

pub mod admin;
pub mod global;
pub mod governance;
pub mod stream;
pub mod vault;

#[cfg(feature = "strategy")]
pub mod strategy;

pub mod channel_staking;

pub mod compound;

#[cfg(feature = "prediction_markets")]
pub mod markets;

#[cfg(feature = "price_feed")]
pub mod price_feed;
