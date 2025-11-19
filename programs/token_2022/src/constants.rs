use anchor_lang::prelude::*;

// Seeds
pub const PROTOCOL_SEED: &[u8] = b"protocol";
pub const TREASURY_SEED: &[u8] = b"treasury";
pub const EPOCH_STATE_SEED: &[u8] = b"epoch_state";
pub const LIQUIDITY_ENGINE_SEED: &[u8] = b"liquidity_engine";
pub const CHANNEL_STATE_SEED: &[u8] = b"channel_state";

pub const CHANNEL_RING_SLOTS: usize = 10;
pub const CHANNEL_MAX_CLAIMS: usize = 4096;
pub const CHANNEL_BITMAP_BYTES: usize = (CHANNEL_MAX_CLAIMS + 7) / 8;
pub const MAX_EPOCH_CLAIMS: u32 = 1_000_000;
pub const MAX_ID_BYTES: usize = 64;

// Token-2022 Config
pub const CCM_DECIMALS: u8 = 9;
pub const INITIAL_SUPPLY: u64 = 1_000_000_000 * 1_000_000_000; // 1B CCM with 9 decimals
pub const DRIP_THRESHOLD: u64 = 1_000_000 * 1_000_000_000; // 1M CCM volume

// Transfer Fee Config (0.1% = 10 basis points)
pub const DEFAULT_TRANSFER_FEE_BASIS_POINTS: u16 = 10;
pub const MAX_FEE_BASIS_POINTS: u16 = 1000; // 10% max

// Fee Distribution (must sum to 100)
pub const DEFAULT_LP_ALLOCATION: u8 = 40; // 40% to LP
pub const DEFAULT_TREASURY_ALLOCATION: u8 = 30; // 30% to treasury
pub const DEFAULT_BURN_ALLOCATION: u8 = 30; // 30% burn

// Liquidity Drip Thresholds (in CCM with decimals)
pub const DRIP_TIER_1_THRESHOLD: u64 = 1_000_000 * 1_000_000_000; // 1M CCM (9 decimals)
pub const DRIP_TIER_2_THRESHOLD: u64 = 5_000_000 * 1_000_000_000; // 5M CCM (9 decimals)
pub const DRIP_TIER_3_THRESHOLD: u64 = 10_000_000 * 1_000_000_000; // 10M CCM (9 decimals)

// Liquidity Drip Amounts
pub const DRIP_TIER_1_CCM: u64 = 5_000_000 * 1_000_000_000; // 5M CCM (9 decimals)
pub const DRIP_TIER_1_SOL: u64 = 2_500_000_000; // 2.5 SOL
pub const DRIP_TIER_2_CCM: u64 = 10_000_000 * 1_000_000_000; // 10M CCM (9 decimals)
pub const DRIP_TIER_2_SOL: u64 = 5_000_000_000; // 5 SOL
pub const DRIP_TIER_3_CCM: u64 = 15_000_000 * 1_000_000_000; // 15M CCM (9 decimals)
pub const DRIP_TIER_3_SOL: u64 = 7_500_000_000; // 7.5 SOL

// Hook Triggers
pub const VOLUME_CHECK_INTERVAL: i64 = 3600; // Check every hour
pub const MIN_VOLUME_FOR_DRIP: u64 = 100_000 * 1_000_000_000; // 100k CCM minimum (9 decimals)

// Epoch force-close grace period (e.g., 7 days)
pub const EPOCH_FORCE_CLOSE_GRACE_SECS: i64 = 7 * 24 * 60 * 60;

// Admin authority (will be DAO eventually)
pub const ADMIN_AUTHORITY: Pubkey = Pubkey::new_from_array([
    0x91, 0x16, 0x1c, 0x33, 0x6c, 0x7e, 0x78, 0x23, 0x60, 0x4f, 0x0a, 0x02, 0x2b, 0x2f, 0x10, 0x60,
    0x40, 0xd8, 0x16, 0xa4, 0x79, 0x0e, 0xc9, 0xf2, 0xba, 0x30, 0x45, 0x3b, 0xdb, 0x1b, 0x59, 0x99,
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
