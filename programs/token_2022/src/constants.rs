use anchor_lang::prelude::*;

// Seeds
pub const PROTOCOL_SEED: &[u8] = b"protocol";
#[cfg(feature = "legacy")]
pub const EPOCH_STATE_SEED: &[u8] = b"epoch_state";
pub const CHANNEL_STATE_SEED: &[u8] = b"channel_state";

// Ring-buffer retention for per-channel merkle roots.
//
// NOTE: Solana's CPI limit (10KB realloc) caps ChannelState at ~9KB.
// With 16 slots × 560 bytes/slot ≈ 9KB, and 60-min epochs:
// - Retention: 16 hours
// - Users have 16 hours to claim before epoch overwrites.
//
// Future: For longer retention, store roots in separate per-epoch accounts.
pub const CHANNEL_RING_SLOTS: usize = 16;
pub const CHANNEL_MAX_CLAIMS: usize = 4096;
pub const CHANNEL_BITMAP_BYTES: usize = (CHANNEL_MAX_CLAIMS + 7) / 8;
pub const MAX_ID_BYTES: usize = 64;

// Token / economics config
pub const DRIP_THRESHOLD: u64 = 1_000_000 * 1_000_000_000; // 1M CCM volume

// Claim-time fee (legacy revenue rail)
//
// IMPORTANT: If the mint already has Token-2022 `TransferFeeConfig` enabled,
// keep this at 0 to avoid double-charging users on claims.
pub const CLAIM_SKIM_BPS: u16 = 0; // 0.00%

pub const MAX_FEE_BASIS_POINTS: u16 = 1000; // 10% max

// Epoch force-close grace period (e.g., 7 days)
#[cfg(feature = "legacy")]
pub const EPOCH_FORCE_CLOSE_GRACE_SECS: i64 = 7 * 24 * 60 * 60;

// Admin authority (will be DAO eventually)
// Wallet: 2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD
pub const ADMIN_AUTHORITY: Pubkey = Pubkey::new_from_array([
    0x1a, 0xf8, 0xe7, 0xe6, 0xe1, 0x90, 0x4e, 0xd7, 0xf3, 0x9f, 0xcd, 0x62, 0x6a, 0x15, 0xb1, 0x11,
    0x06, 0x7b, 0x7a, 0x88, 0xf2, 0x1c, 0x8c, 0x7c, 0x3b, 0x1f, 0x8a, 0xa7, 0x5e, 0x50, 0x81, 0x16,
]);

// Passport registry seeds / helpers
pub const PASSPORT_SEED: &[u8] = b"passport_owner";

pub fn passport_pda(program_id: &Pubkey, user_hash: &[u8; 32]) -> Pubkey {
    Pubkey::find_program_address(&[PASSPORT_SEED, user_hash], program_id).0
}

// Passport tier defaults (example thresholds)
pub const MAX_TIER: u8 = 6;
pub const MIN_TIER_SILVER: u8 = 2;
pub const MIN_TIER_GOLD: u8 = 4;

pub const BASE_SCORE_PER_EPOCH: u64 = 100;
pub const BONUS_MULTIPLIER_MESSAGES: u64 = 10;
pub const BONUS_MULTIPLIER_SUBS: u64 = 50;
pub const BONUS_MULTIPLIER_BITS: u64 = 1;

// Dynamic Fee Tier Multipliers (0.0-1.0 for creator allocation)
// Tier 0: No verified passport (0.0x)
// Tier 1: Emerging creator (0.2x)
// Tier 2: Active creator (0.4x)
// Tier 3: Established creator (0.6x)
// Tier 4: Featured creator (0.8x)
// Tier 5+: Elite creator (1.0x)
pub const TIER_MULTIPLIERS: [u32; 6] = [0, 2000, 4000, 6000, 8000, 10000];

// Fee split basis points (total = 10 BPS = 0.1%)
pub const TREASURY_FEE_BASIS_POINTS: u16 = 5; // 0.05% to treasury
pub const CREATOR_FEE_BASIS_POINTS: u16 = 5; // 0.05% to creator (multiplied by tier)

// Harvest split (applies to withheld total during harvest)
// Non-breaking default: true 50/50 split between treasury and creator pool
pub const HARVEST_SPLIT_BPS_TREASURY: u16 = 5000; // 50.00%

// =============================================================================
// STAKING SYSTEM (V1)
// =============================================================================
pub const STAKE_POOL_SEED: &[u8] = b"stake_pool";
pub const USER_STAKE_SEED: &[u8] = b"user_stake";
pub const STAKE_VAULT_SEED: &[u8] = b"stake_vault";
pub const MIN_STAKE_AMOUNT: u64 = 1_000_000_000; // 1 CCM (9 decimals)
pub const MAX_LOCK_SLOTS: u64 = 432_000 * 30; // ~30 days at 400ms slots
pub const REWARD_PRECISION: u128 = 1_000_000_000_000; // 1e12 for MasterChef math

// =============================================================================
// CREATOR EXTENSIONS (V1)
// =============================================================================
pub const CHANNEL_META_SEED: &[u8] = b"channel_meta";
pub const MAX_CREATOR_FEE_BPS: u16 = 5000; // 50% max
pub const DEFAULT_CREATOR_FEE_SHARE_BPS: u16 = 1000; // 10% default
