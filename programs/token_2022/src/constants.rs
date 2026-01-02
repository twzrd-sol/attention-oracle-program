//! Protocol constants for the Attention Oracle.

use anchor_lang::prelude::*;

// =============================================================================
// PDA SEEDS
// =============================================================================

pub const PROTOCOL_SEED: &[u8] = b"protocol";
pub const CHANNEL_CONFIG_V2_SEED: &[u8] = b"channel_cfg_v2";
pub const CLAIM_STATE_V2_SEED: &[u8] = b"claim_state_v2";
pub const PASSPORT_SEED: &[u8] = b"passport_owner";
pub const STAKE_POOL_SEED: &[u8] = b"stake_pool";
pub const USER_STAKE_SEED: &[u8] = b"user_stake";
pub const STAKE_VAULT_SEED: &[u8] = b"stake_vault";

// =============================================================================
// CUMULATIVE V2 CLAIMS
// =============================================================================

/// Number of recent merkle roots to retain per channel
pub const CUMULATIVE_ROOT_HISTORY: usize = 4;

/// Domain separation for cumulative V2 leaf hashing
pub const CUMULATIVE_V2_DOMAIN: &[u8] = b"TWZRD:CUMULATIVE_V2";

// =============================================================================
// ECONOMICS & FEES
// =============================================================================

/// Volume threshold for drip triggers
pub const DRIP_THRESHOLD: u64 = 1_000_000 * 1_000_000_000; // 1M CCM

/// Maximum transfer fee basis points
pub const MAX_FEE_BASIS_POINTS: u16 = 1000; // 10% max

/// Maximum creator fee (50% of claim delta)
pub const MAX_CREATOR_FEE_BPS: u16 = 5000;

/// Treasury fee (applied to transfers)
pub const TREASURY_FEE_BASIS_POINTS: u16 = 5; // 0.05%

/// Creator fee (applied to transfers)
pub const CREATOR_FEE_BASIS_POINTS: u16 = 5; // 0.05%

/// Harvest split to treasury
pub const HARVEST_SPLIT_BPS_TREASURY: u16 = 5000; // 50%

// =============================================================================
// STAKING
// =============================================================================

/// Minimum stake amount (1 CCM with 9 decimals)
pub const MIN_STAKE_AMOUNT: u64 = 1_000_000_000;

/// Maximum lock duration (~30 days at 400ms slots)
pub const MAX_LOCK_SLOTS: u64 = 432_000 * 30;

/// Precision multiplier for MasterChef reward math
pub const REWARD_PRECISION: u128 = 1_000_000_000_000; // 1e12

// =============================================================================
// PASSPORT / IDENTITY
// =============================================================================

pub const MAX_TIER: u8 = 6;
pub const MIN_TIER_SILVER: u8 = 2;
pub const MIN_TIER_GOLD: u8 = 4;

pub const BASE_SCORE_PER_EPOCH: u64 = 100;
pub const BONUS_MULTIPLIER_MESSAGES: u64 = 10;
pub const BONUS_MULTIPLIER_SUBS: u64 = 50;
pub const BONUS_MULTIPLIER_BITS: u64 = 1;

/// Default tier multipliers for fee calculation
pub const TIER_MULTIPLIERS: [u32; 6] = [0, 2000, 4000, 6000, 8000, 10000];

/// Derive passport PDA from user hash
pub fn passport_pda(program_id: &Pubkey, user_hash: &[u8; 32]) -> Pubkey {
    Pubkey::find_program_address(&[PASSPORT_SEED, user_hash], program_id).0
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
