use crate::constants::{CHANNEL_BITMAP_BYTES, CHANNEL_MAX_CLAIMS, CHANNEL_RING_SLOTS};
use crate::errors::MiloError;
use anchor_lang::prelude::*;

/// Global protocol state (singleton)
#[account]
pub struct ProtocolState {
    /// Initialization flag
    pub is_initialized: bool,

    /// Current version for migrations
    pub version: u8,

    /// Admin authority (can update config)
    pub admin: Pubkey,

    /// Allowlisted publisher (optional). If set, this key may publish
    /// merkle roots in addition to the admin. If Pubkey::default(),
    /// only the admin is authorized.
    pub publisher: Pubkey,

    /// Treasury PDA
    pub treasury: Pubkey,

    /// CCM mint address
    pub mint: Pubkey,

    /// Emergency pause flag
    pub paused: bool,

    /// Require TWZRD Layer-1 cNFT receipt for claims (default: false)
    /// Toggle via set_policy instruction for circuit breaker pattern
    pub require_receipt: bool,

    /// Bump seed for PDA
    pub bump: u8,
}

impl ProtocolState {
    pub const LEN: usize = 8 +  // discriminator
        1 +    // is_initialized
        1 +    // version
        32 +   // admin
        32 +   // publisher
        32 +   // treasury
        32 +   // mint
        1 +    // paused
        1 +    // require_receipt
        1; // bump
}

/// Fee configuration (PDA account)
#[account]
pub struct FeeConfig {
    /// Transfer fee in basis points (10 = 0.1%)
    pub basis_points: u16,

    /// Maximum fee amount
    pub max_fee: u64,

    /// Drip threshold (volume triggers)
    pub drip_threshold: u64,

    /// Bump seed
    pub bump: u8,
}

impl FeeConfig {
    pub const LEN: usize = 8 + // discriminator
        2 +    // basis_points
        8 +    // max_fee
        8 +    // drip_threshold
        1; // bump
}

/// Fee distribution split
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct FeeSplit {
    /// Percentage to LP (0-100)
    pub lp_allocation: u8,

    /// Percentage to treasury (0-100)
    pub treasury_allocation: u8,

    /// Percentage to burn (0-100)
    pub burn_allocation: u8,
}

impl FeeSplit {
    pub const LEN: usize = 1 + 1 + 1;

    pub fn validate(&self) -> Result<()> {
        require!(
            self.lp_allocation + self.treasury_allocation + self.burn_allocation == 100,
            MiloError::InvalidFeeSplit
        );
        Ok(())
    }
}

/// Volume tracking for hook triggers
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct VolumeStats {
    /// Total volume all-time
    pub total_volume: u128,

    /// Volume in current hour
    pub hourly_volume: u64,

    /// Volume in current day
    pub daily_volume: u64,

    /// Last hourly reset timestamp
    pub last_hour_reset: i64,

    /// Last daily reset timestamp
    pub last_day_reset: i64,

    /// Total transfer count
    pub transfer_count: u64,
}

impl VolumeStats {
    pub const LEN: usize = 16 + 8 + 8 + 8 + 8 + 8;

    pub fn update(&mut self, amount: u64, current_time: i64) {
        // Reset hourly if needed
        if current_time - self.last_hour_reset > 3600 {
            self.hourly_volume = 0;
            self.last_hour_reset = current_time;
        }

        // Reset daily if needed
        if current_time - self.last_day_reset > 86400 {
            self.daily_volume = 0;
            self.last_day_reset = current_time;
        }

        // Update volumes
        self.total_volume = self.total_volume.saturating_add(amount as u128);
        self.hourly_volume = self.hourly_volume.saturating_add(amount);
        self.daily_volume = self.daily_volume.saturating_add(amount);
        self.transfer_count = self.transfer_count.saturating_add(1);
    }
}

/// Liquidity engine state
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct LiquidityState {
    /// Current drip tier (0-3)
    pub current_tier: u8,

    /// Total CCM claimed across all epochs
    pub total_claimed: u64,

    /// Total CCM dripped to LP
    pub total_dripped: u64,

    /// Raydium pool address
    pub pool_address: Pubkey,

    /// Last drip timestamp
    pub last_drip: i64,

    /// Tier 1 completed
    pub tier_1_complete: bool,

    /// Tier 2 completed
    pub tier_2_complete: bool,

    /// Tier 3 completed
    pub tier_3_complete: bool,
}

impl LiquidityState {
    pub const LEN: usize = 1 + 8 + 8 + 32 + 8 + 1 + 1 + 1;

    pub fn should_drip(&self, total_claimed: u64) -> Option<u8> {
        use crate::constants::*;

        if !self.tier_1_complete && total_claimed >= DRIP_TIER_1_THRESHOLD {
            return Some(1);
        }
        if !self.tier_2_complete && total_claimed >= DRIP_TIER_2_THRESHOLD {
            return Some(2);
        }
        if !self.tier_3_complete && total_claimed >= DRIP_TIER_3_THRESHOLD {
            return Some(3);
        }
        None
    }
}

/// Liquidity engine PDA account (wraps LiquidityState + bump)
#[account]
pub struct LiquidityEngine {
    pub state: LiquidityState,
    pub bump: u8,
}

impl LiquidityEngine {
    pub const LEN: usize = 8 /* disc */ + LiquidityState::LEN + 1;
}

/// Epoch state for merkle claims (unchanged from v2)
#[account]
pub struct EpochState {
    pub epoch: u64,
    pub root: [u8; 32],
    pub claim_count: u32,
    pub mint: Pubkey,
    pub streamer: Pubkey,
    pub treasury: Pubkey,
    pub timestamp: i64,
    pub bump: u8,
    pub total_claimed: u64,
    pub closed: bool,
    pub claimed_bitmap: Vec<u8>,
}

impl EpochState {
    pub fn space_for(claims: usize) -> usize {
        8 + // discriminator
        8 + // epoch
        32 + // root
        4 + // claim_count
        32 + // mint
        32 + // streamer
        32 + // treasury
        8 + // timestamp
        1 + // bump
        8 + // total_claimed
        1 + // closed
        4 + // vec length
        ((claims + 7) / 8) // bitmap
    }
}

/// Passport registry entry (oracle snapshot for viewer reputation)
#[account]
pub struct PassportRegistry {
    pub owner: Pubkey,
    pub user_hash: [u8; 32],
    pub tier: u8,
    pub score: u64,
    pub epoch_count: u32,
    pub weighted_presence: u64,
    pub badges: u32,
    pub tree: Pubkey,
    pub leaf_hash: Option<[u8; 32]>,
    pub updated_at: i64,
    pub bump: u8,
}

impl PassportRegistry {
    pub const LEN: usize = 8 + // discriminator
        32 + // owner
        32 + // user_hash
        1 +  // tier
        8 +  // score
        4 +  // epoch_count
        8 +  // weighted_presence
        4 +  // badges
        32 + // tree
        1 + 32 + // Option<[u8;32]> tag + value
        8 +  // updated_at
        1; // bump
}

/// Per-channel ring buffer state (stores recent epoch merkle roots)
/// Uses zero_copy to avoid stack overflow (1.7KB struct)
#[account(zero_copy)]
#[repr(C, packed)]
pub struct ChannelState {
    pub version: u8,
    pub bump: u8,
    pub mint: Pubkey,
    pub streamer: Pubkey,
    pub latest_epoch: u64,
    pub slots: [ChannelSlot; CHANNEL_RING_SLOTS],
}

impl ChannelState {
    pub const LEN: usize = 8 /* disc */
        + 1 /* version */
        + 1 /* bump */
        + 32 /* mint */
        + 32 /* streamer */
        + 8 /* latest_epoch */
        + (ChannelSlot::LEN * CHANNEL_RING_SLOTS);

    pub fn slot_index(epoch: u64) -> usize {
        (epoch as usize) % CHANNEL_RING_SLOTS
    }

    pub fn slot(&self, epoch: u64) -> &ChannelSlot {
        &self.slots[Self::slot_index(epoch)]
    }

    pub fn slot_mut(&mut self, epoch: u64) -> &mut ChannelSlot {
        &mut self.slots[Self::slot_index(epoch)]
    }
}

#[zero_copy]
#[repr(C, packed)]
pub struct ChannelSlot {
    pub epoch: u64,
    pub root: [u8; 32],
    pub claim_count: u16,
    pub claimed_bitmap: [u8; CHANNEL_BITMAP_BYTES],
}

impl ChannelSlot {
    pub const LEN: usize = 8 /* epoch */
        + 32 /* root */
        + 2 /* claim_count */
        + CHANNEL_BITMAP_BYTES;

    pub fn reset(&mut self, epoch: u64, root: [u8; 32]) {
        self.epoch = epoch;
        self.root = root;
        self.claim_count = 0;
        self.claimed_bitmap = [0u8; CHANNEL_BITMAP_BYTES];
    }

    pub fn test_bit(&self, index: usize) -> bool {
        let byte = index / 8;
        let bit = index % 8;
        (self.claimed_bitmap[byte] & (1u8 << bit)) != 0
    }

    pub fn set_bit(&mut self, index: usize) {
        let byte = index / 8;
        let bit = index % 8;
        self.claimed_bitmap[byte] |= 1u8 << bit;
    }

    pub fn validate_index(index: usize) -> Result<()> {
        require!(index < CHANNEL_MAX_CLAIMS, MiloError::InvalidIndex);
        Ok(())
    }
}
