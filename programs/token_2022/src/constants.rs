//! Protocol constants for the Attention Oracle.

use anchor_lang::prelude::*;

// =============================================================================
// PDA SEEDS
// =============================================================================

pub const PROTOCOL_SEED: &[u8] = b"protocol";
pub const CHANNEL_CONFIG_V2_SEED: &[u8] = b"channel_cfg_v2";
pub const CLAIM_STATE_V2_SEED: &[u8] = b"claim_state_v2";

// Channel staking PDAs (Token-2022 with NonTransferable extension)
pub const CHANNEL_STAKE_POOL_SEED: &[u8] = b"channel_pool";
pub const CHANNEL_USER_STAKE_SEED: &[u8] = b"channel_user";
pub const STAKE_NFT_MINT_SEED: &[u8] = b"stake_nft";
pub const STAKE_VAULT_SEED: &[u8] = b"stake_vault";

// =============================================================================
// CUMULATIVE V2 CLAIMS
// =============================================================================

/// Number of recent merkle roots to retain per channel
pub const CUMULATIVE_ROOT_HISTORY: usize = 4;

/// Domain separation for cumulative V2 leaf hashing
pub const CUMULATIVE_V2_DOMAIN: &[u8] = b"TWZRD:CUMULATIVE_V2";

/// Domain separation for cumulative V3 leaf hashing (includes stake_snapshot)
pub const CUMULATIVE_V3_DOMAIN: &[u8] = b"TWZRD:CUMULATIVE_V3";

// =============================================================================
// ECONOMICS & FEES
// =============================================================================

/// Volume threshold for drip triggers
pub const DRIP_THRESHOLD: u64 = 1_000_000 * 1_000_000_000; // 1M CCM

/// Maximum transfer fee basis points
pub const MAX_FEE_BASIS_POINTS: u16 = 1000; // 10% max

/// Treasury fee (applied to transfers)
pub const TREASURY_FEE_BASIS_POINTS: u16 = 5; // 0.05%

/// Creator fee (applied to transfers)
pub const CREATOR_FEE_BASIS_POINTS: u16 = 5; // 0.05%

// =============================================================================
// STAKING
// =============================================================================

/// Minimum stake amount (1 CCM with 9 decimals)
pub const MIN_STAKE_AMOUNT: u64 = 1_000_000_000;

/// Maximum lock duration (~365 days at 400ms slots)
pub const MAX_LOCK_SLOTS: u64 = 432_000 * 365;

// =============================================================================
// STAKING BOOST
// =============================================================================

/// Precision for boost calculations (100% = 10000)
pub const BOOST_PRECISION: u64 = 10_000;

/// Slots per day (approximate at 400ms slot time)
pub const SLOTS_PER_DAY: u64 = 216_000;

/// Minimum runway slots for reward rate validation (~1 day)
/// When setting a reward rate, the treasury must have enough funds
/// to sustain that rate for at least this many slots.
pub const MIN_RUNWAY_SLOTS: u64 = 216_000;

/// Precision for reward accumulator (1e12)
pub const REWARD_PRECISION: u128 = 1_000_000_000_000;

/// Slots per year (approximate at 400ms slot time)
pub const SLOTS_PER_YEAR: u64 = 78_840_000;

/// Maximum APR in basis points (15% = 1500 bps)
pub const MAX_APR_BPS: u64 = 1500;

/// Basis points denominator
pub const BPS_DENOMINATOR: u64 = 10_000;

/// Calculate boost basis points based on lock duration.
/// Returns multiplier in basis points (10000 = 1.0x, 30000 = 3.0x)
pub fn calculate_boost_bps(lock_duration: u64) -> u64 {
    let days = lock_duration / SLOTS_PER_DAY;

    match days {
        0..=6 => 10_000,      // 1.0x   - less than 7 days
        7..=29 => 12_500,     // 1.25x  - 7-29 days
        30..=89 => 15_000,    // 1.5x   - 30-89 days
        90..=179 => 20_000,   // 2.0x   - 90-179 days
        180..=364 => 25_000,  // 2.5x   - 180-364 days
        _ => 30_000,          // 3.0x   - 365+ days
    }
}

// =============================================================================
// ADMIN
// =============================================================================

/// Admin authority (will transition to DAO)
/// Wallet: 2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD
pub const ADMIN_AUTHORITY: Pubkey = Pubkey::new_from_array([
    0x1a, 0xf8, 0xe7, 0xe6, 0xe1, 0x90, 0x4e, 0xd7, 0xf3, 0x9f, 0xcd, 0x62, 0x6a, 0x15, 0xb1, 0x11,
    0x06, 0x7b, 0x7a, 0x88, 0xf2, 0x1c, 0x8c, 0x7c, 0x3b, 0x1f, 0x8a, 0xa7, 0x5e, 0x50, 0x81, 0x16,
]);
