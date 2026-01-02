//! On-chain state definitions for the Attention Oracle Protocol.

use crate::constants::CUMULATIVE_ROOT_HISTORY;
use crate::errors::OracleError;
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

/// Fee distribution split
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct FeeSplit {
    pub lp_allocation: u8,
    pub treasury_allocation: u8,
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

// =============================================================================
// IDENTITY (PASSPORT)
// =============================================================================

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
    pub const LEN: usize = 8 + 32 + 32 + 1 + 8 + 4 + 8 + 4 + 32 + 1 + 32 + 8 + 1;
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
// STAKING
// =============================================================================

/// Global stake pool state (MasterChef-style).
#[account]
pub struct StakePool {
    pub version: u8,
    pub bump: u8,
    pub mint: Pubkey,
    pub total_staked: u64,
    pub acc_reward_per_share: u128,
    pub last_reward_time: i64,
    pub reward_rate: u64,
    pub authority: Pubkey,
    pub _reserved: [u8; 64],
}

impl StakePool {
    pub const LEN: usize = 8 + 1 + 1 + 32 + 8 + 16 + 8 + 8 + 32 + 64;
}

/// Per-user stake position.
#[account]
pub struct UserStake {
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
    pub _reserved: [u8; 32],
}

impl UserStake {
    pub const LEN: usize = 8 + 1 + 1 + 32 + 32 + 8 + 1 + 32 + 8 + 16 + 8 + 8 + 32;
}
