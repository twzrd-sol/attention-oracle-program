use crate::constants::{CHANNEL_BITMAP_BYTES, CHANNEL_MAX_CLAIMS, CHANNEL_RING_SLOTS};
use crate::errors::OracleError;
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

    /// Treasury fee basis points (0.05% base)
    pub treasury_fee_bps: u16,

    /// Creator fee basis points (0.05% base, multiplied by tier)
    pub creator_fee_bps: u16,

    /// Tier multipliers for creator allocation (array of 6 f64 values)
    /// Stores as fixed-point u32 (multiplied by 10000 for precision)
    pub tier_multipliers: [u32; 6],

    /// Bump seed
    pub bump: u8,
}

impl FeeConfig {
    pub const LEN: usize = 8 + // discriminator
        2 +    // basis_points
        8 +    // max_fee
        8 +    // drip_threshold
        2 +    // treasury_fee_bps
        2 +    // creator_fee_bps
        (4 * 6) + // tier_multipliers (6 u32s)
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
            OracleError::InvalidFeeSplit
        );
        Ok(())
    }
}

/// Epoch state for legacy merkle claims (deprecated).
#[cfg(feature = "legacy")]
#[account]
pub struct EpochState {
    pub epoch: u64,
    pub root: [u8; 32],
    pub claim_count: u32,
    pub mint: Pubkey,
    pub subject: Pubkey,
    pub treasury: Pubkey,
    pub timestamp: i64,
    pub bump: u8,
    pub total_claimed: u64,
    pub closed: bool,
    pub claimed_bitmap: Vec<u8>,
}

#[cfg(feature = "legacy")]
impl EpochState {
    pub fn space_for(claims: usize) -> usize {
        8 + // discriminator
        8 + // epoch
        32 + // root
        4 + // claim_count
        32 + // mint
        32 + // subject
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

/// Per-channel ring buffer state (stores recent epoch merkle roots).
/// Uses `zero_copy` to avoid full deserialization of large accounts.
#[account(zero_copy)]
#[repr(C)]
pub struct ChannelState {
    pub version: u8,
    pub bump: u8,
    pub mint: Pubkey,
    pub subject: Pubkey,
    pub _padding: [u8; 6], // Explicit padding for u64 alignment
    pub latest_epoch: u64,
    // Large arrays have limited bytemuck Pod/Zeroable impls; we store 2048 slots as 2×1024.
    pub slots_0: [ChannelSlot; 1024],
    pub slots_1: [ChannelSlot; 1024],
}

impl ChannelState {
    pub const LEN: usize = 8 /* disc */ + core::mem::size_of::<ChannelState>();

    pub fn slot_index(epoch: u64) -> usize {
        (epoch as usize) % CHANNEL_RING_SLOTS
    }

    pub fn slot(&self, epoch: u64) -> &ChannelSlot {
        let idx = Self::slot_index(epoch);
        if idx < 1024 {
            &self.slots_0[idx]
        } else {
            &self.slots_1[idx - 1024]
        }
    }

    pub fn slot_mut(&mut self, epoch: u64) -> &mut ChannelSlot {
        let idx = Self::slot_index(epoch);
        if idx < 1024 {
            &mut self.slots_0[idx]
        } else {
            &mut self.slots_1[idx - 1024]
        }
    }
}

#[zero_copy]
#[repr(C)]
pub struct ChannelSlot {
    pub epoch: u64,
    pub root: [u8; 32],
    pub claim_count: u16,
    pub _padding: [u8; 6], // Explicit padding to maintain 8-byte alignment
    pub claimed_bitmap: [u8; CHANNEL_BITMAP_BYTES],
}

impl ChannelSlot {
    pub const LEN: usize = core::mem::size_of::<ChannelSlot>();

    pub fn reset(&mut self, epoch: u64, root: [u8; 32]) {
        self.epoch = epoch;
        self.root = root;
        self.claim_count = 0;
        self._padding = [0u8; 6];
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
        require!(index < CHANNEL_MAX_CLAIMS, OracleError::InvalidIndex);
        Ok(())
    }
}

// =============================================================================
// STAKING SYSTEM (V1)
// =============================================================================

/// Global stake pool state (mint-keyed)
/// Seeds: ["stake_pool", mint]
#[account]
pub struct StakePool {
    /// Version for future migrations
    pub version: u8,
    /// PDA bump seed
    pub bump: u8,
    /// CCM mint this pool is for
    pub mint: Pubkey,
    /// Total CCM staked in pool
    pub total_staked: u64,
    /// MasterChef-style accumulated reward per share (scaled by REWARD_PRECISION)
    pub acc_reward_per_share: u128,
    /// Last time rewards were updated
    pub last_reward_time: i64,
    /// Reward rate (CCM per second)
    pub reward_rate: u64,
    /// Authority that can modify pool params
    pub authority: Pubkey,
    /// Reserved for future use
    pub _reserved: [u8; 64],
}

impl StakePool {
    pub const LEN: usize = 8 +  // discriminator
        1 +     // version
        1 +     // bump
        32 +    // mint
        8 +     // total_staked
        16 +    // acc_reward_per_share
        8 +     // last_reward_time
        8 +     // reward_rate
        32 +    // authority
        64;     // _reserved
    // Total: 178 bytes
}

/// Per-user stake state (user + mint keyed)
/// Seeds: ["user_stake", user, mint]
#[account]
pub struct UserStake {
    /// Version for future migrations
    pub version: u8,
    /// PDA bump seed
    pub bump: u8,
    /// User pubkey
    pub user: Pubkey,
    /// CCM mint this stake is for
    pub mint: Pubkey,
    /// Amount of CCM staked
    pub staked_amount: u64,
    /// Optional channel subject_id for delegation (keccak hash)
    pub delegated_subject: Option<[u8; 32]>,
    /// Slot when lock expires (0 = unlocked)
    pub lock_end_slot: u64,
    /// MasterChef reward debt (scaled by REWARD_PRECISION)
    pub reward_debt: u128,
    /// Pending rewards to claim
    pub pending_rewards: u64,
    /// Last action timestamp
    pub last_action_time: i64,
    /// Reserved for future use
    pub _reserved: [u8; 32],
}

impl UserStake {
    pub const LEN: usize = 8 +  // discriminator
        1 +     // version
        1 +     // bump
        32 +    // user
        32 +    // mint
        8 +     // staked_amount
        1 + 32 + // delegated_subject Option<[u8;32]>
        8 +     // lock_end_slot
        16 +    // reward_debt
        8 +     // pending_rewards
        8 +     // last_action_time
        32;     // _reserved
    // Total: 187 bytes
}

// =============================================================================
// CREATOR EXTENSIONS (V1)
// =============================================================================

/// Channel metadata for creator revenue sharing
/// Seeds: ["channel_meta", channel_state]
#[account]
pub struct ChannelMeta {
    /// Version for future migrations
    pub version: u8,
    /// PDA bump seed
    pub bump: u8,
    /// Associated channel state PDA
    pub channel_state: Pubkey,
    /// Creator wallet that receives fee share
    pub creator_wallet: Pubkey,
    /// Fee share in basis points (e.g., 1000 = 10%)
    pub fee_share_bps: u16,
    /// Sum of delegated stakes to this channel
    pub total_delegated: u64,
    /// Reserved for future use
    pub _reserved: [u8; 64],
}

impl ChannelMeta {
    pub const LEN: usize = 8 +  // discriminator
        1 +     // version
        1 +     // bump
        32 +    // channel_state
        32 +    // creator_wallet
        2 +     // fee_share_bps
        8 +     // total_delegated
        64;     // _reserved
    // Total: 148 bytes
}
