//! On-chain state definitions for the Attention Oracle Protocol.

use crate::constants::CUMULATIVE_ROOT_HISTORY;
use anchor_lang::prelude::*;

// =============================================================================
// PROTOCOL STATE
// =============================================================================

/// Global protocol state (singleton per mint)
#[account]
pub struct ProtocolState {
    pub is_initialized: bool,
    pub version: u8,
    pub admin: Pubkey,
    pub publisher: Pubkey,
    pub treasury: Pubkey,
    pub mint: Pubkey,
    pub paused: bool,
    /// Legacy field (no longer enforced).
    pub require_receipt: bool,
    pub bump: u8,
}

impl ProtocolState {
    pub const LEN: usize = 8 + 1 + 1 + 32 + 32 + 32 + 32 + 1 + 1 + 1;
}

/// Fee configuration (PDA account)
#[account]
pub struct FeeConfig {
    pub basis_points: u16,
    pub max_fee: u64,
    pub drip_threshold: u64,
    pub treasury_fee_bps: u16,
    pub creator_fee_bps: u16,
    pub tier_multipliers: [u32; 6],
    pub bump: u8,
}

impl FeeConfig {
    pub const LEN: usize = 8 + 2 + 8 + 8 + 2 + 2 + (4 * 6) + 1;
}

// =============================================================================
// CUMULATIVE ROOTS (V2 CLAIMS)
// =============================================================================

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default)]
pub struct RootEntry {
    pub seq: u64,
    pub root: [u8; 32],
    pub dataset_hash: [u8; 32],
    pub published_slot: u64,
}

impl RootEntry {
    pub const LEN: usize = 8 + 32 + 32 + 8;
}

/// Channel configuration for V2 cumulative claims.
/// Stores recent merkle roots and creator fee settings.
#[account]
pub struct ChannelConfigV2 {
    pub version: u8,
    pub bump: u8,
    pub mint: Pubkey,
    pub subject: Pubkey,
    pub authority: Pubkey,
    pub latest_root_seq: u64,
    pub cutover_epoch: u64,
    /// Creator wallet for receiving fee split from claims
    pub creator_wallet: Pubkey,
    /// Creator fee in basis points (0-5000 = 0-50%)
    pub creator_fee_bps: u16,
    /// Padding for alignment
    pub _padding: [u8; 6],
    pub roots: [RootEntry; CUMULATIVE_ROOT_HISTORY],
}

impl ChannelConfigV2 {
    pub const LEN: usize = 8 + 1 + 1 + 32 + 32 + 32 + 8 + 8 + 32 + 2 + 6 + (RootEntry::LEN * CUMULATIVE_ROOT_HISTORY);
}

/// Per-user claim state for V2 cumulative system.
/// Tracks total claimed amount to enable delta-based claims.
#[account]
pub struct ClaimStateV2 {
    pub version: u8,
    pub bump: u8,
    pub channel: Pubkey,
    pub wallet: Pubkey,
    pub claimed_total: u64,
    pub last_claim_seq: u64,
}

impl ClaimStateV2 {
    pub const LEN: usize = 8 + 1 + 1 + 32 + 32 + 8 + 8;
}

// =============================================================================
// CHANNEL STAKING (TOKEN-2022 SOULBOUND NFT)
// =============================================================================

/// Stake pool for a specific channel.
/// Uses Token-2022 NonTransferable extension for soulbound receipts.
/// Seeds: ["channel_pool", channel_config]
#[account]
pub struct ChannelStakePool {
    pub bump: u8,
    /// Reference to ChannelConfigV2
    pub channel: Pubkey,
    /// Token mint (CCM)
    pub mint: Pubkey,
    /// Vault holding staked tokens
    pub vault: Pubkey,
    /// Total tokens staked in this pool
    pub total_staked: u64,
    /// Sum of weighted stakes (for boost tracking)
    pub total_weighted: u64,
    /// Number of active stake positions
    pub staker_count: u64,

    // Reward infrastructure (MasterChef-style)
    /// Accumulated rewards per weighted share (scaled by 1e12)
    pub acc_reward_per_share: u128,
    /// Last slot when rewards were calculated
    pub last_reward_slot: u64,
    /// Rewards distributed per slot (admin-configurable)
    pub reward_per_slot: u64,

    // Emergency shutdown (v1.3.0)
    /// If true, pool is shut down: no new stakes, all locks waived for penalty-free exit
    pub is_shutdown: bool,
}

impl ChannelStakePool {
    // 8 + 1 + 32 + 32 + 32 + 8 + 8 + 8 + 16 + 8 + 8 + 1 = 162
    pub const LEN: usize = 162;
}

/// User's stake position on a channel.
/// Includes reference to soulbound NFT receipt.
/// Seeds: ["channel_user", channel_config, user]
#[account]
pub struct UserChannelStake {
    pub bump: u8,
    /// Staker wallet
    pub user: Pubkey,
    /// Reference to ChannelConfigV2
    pub channel: Pubkey,
    /// Amount of tokens staked
    pub amount: u64,
    /// Slot when stake was created
    pub start_slot: u64,
    /// Slot when lock expires (0 = no lock)
    pub lock_end_slot: u64,
    /// Boost multiplier in basis points (10000 = 1x, 30000 = 3x)
    pub multiplier_bps: u64,
    /// Token-2022 Mint for the soulbound receipt NFT
    pub nft_mint: Pubkey,

    // Reward tracking
    /// User's reward debt (prevents double-claiming)
    pub reward_debt: u128,
    /// Accumulated pending rewards (claimable)
    pub pending_rewards: u64,
}

impl UserChannelStake {
    // 8 + 1 + 32 + 32 + 8 + 8 + 8 + 8 + 32 + 16 + 8 = 161
    pub const LEN: usize = 161;
}

// =============================================================================
// LEGACY TYPES (BACKWARDS COMPATIBILITY)
// =============================================================================

/// Passport registry entry (oracle snapshot for viewer reputation)
#[account]
pub struct PassportRegistry {
    /// NOTE: Passport functionality is not exposed via the current public program
    /// interface. This account type remains for backwards compatibility / history.
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
    pub const LEN: usize = 8 + 32 + 32 + 1 + 8 + 4 + 8 + 4 + 32 + 1 + 32 + 8 + 1;
}

/// Tracks daily withdrawal limits for treasury admin withdrawals.
#[account]
pub struct WithdrawTracker {
    /// NOTE: Treasury admin withdrawals were removed; this account is now legacy.
    pub version: u8,
    pub bump: u8,
    pub mint: Pubkey,
    pub day_start: i64,
    pub withdrawn_today: u64,
    pub total_withdrawn: u64,
    pub last_withdraw_at: i64,
}

impl WithdrawTracker {
    pub const LEN: usize = 8 + 1 + 1 + 32 + 8 + 8 + 8 + 8;
}

/// Global stake pool state (MasterChef-style).
#[account]
pub struct StakePool {
    /// NOTE: Staking functionality is not exposed via the current public program
    /// interface. This account type remains for backwards compatibility / history.
    pub version: u8,
    pub bump: u8,
    pub mint: Pubkey,
    pub total_staked: u64,
    pub acc_reward_per_share: u128,
    pub last_reward_time: i64,
    pub reward_rate: u64,
    pub authority: Pubkey,
    pub total_weighted_stake: u64,
    pub _reserved: [u8; 56],
}

impl StakePool {
    pub const LEN: usize = 8 + 1 + 1 + 32 + 8 + 16 + 8 + 8 + 32 + 8 + 56;

    pub fn get_effective_total(&self) -> u64 {
        if self.total_weighted_stake > 0 {
            self.total_weighted_stake
        } else {
            self.total_staked
        }
    }
}

/// Per-user stake position.
#[account]
pub struct UserStake {
    /// NOTE: Staking functionality is not exposed via the current public program
    /// interface. This account type remains for backwards compatibility / history.
    pub version: u8,
    pub bump: u8,
    pub user: Pubkey,
    pub mint: Pubkey,
    pub staked_amount: u64,
    pub delegated_subject: Option<[u8; 32]>,
    pub lock_end_slot: u64,
    pub reward_debt: u128,
    pub pending_rewards: u64,
    pub last_action_time: i64,
    pub weighted_stake: u64,
    pub _reserved: [u8; 24],
}

impl UserStake {
    pub const LEN: usize = 8 + 1 + 1 + 32 + 32 + 8 + 1 + 32 + 8 + 16 + 8 + 8 + 8 + 24;

    pub fn get_effective_stake(&self) -> u64 {
        if self.weighted_stake > 0 {
            self.weighted_stake
        } else {
            self.staked_amount
        }
    }
}
