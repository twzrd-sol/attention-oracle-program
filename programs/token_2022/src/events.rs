use anchor_lang::prelude::*;

#[event]
pub struct CumulativeRewardsClaimed {
    pub channel: Pubkey,
    pub claimer: Pubkey,
    pub user_amount: u64,
    pub creator_amount: u64,
    pub cumulative_total: u64,
    pub root_seq: u64,
}

// =============================================================================
// ADMIN OPERATION EVENTS
// Emitting events for state changes enables off-chain indexing and observability.
// =============================================================================

/// Emitted when the allowlisted publisher is updated.
#[event]
pub struct PublisherUpdated {
    pub admin: Pubkey,
    pub old_publisher: Pubkey,
    pub new_publisher: Pubkey,
    pub mint: Pubkey,
    pub timestamp: i64,
}

/// Emitted when protocol is paused or unpaused.
#[event]
pub struct ProtocolPaused {
    pub admin: Pubkey,
    pub paused: bool,
    pub mint: Pubkey,
    pub timestamp: i64,
}

/// Emitted when admin authority is transferred.
#[event]
pub struct AdminTransferred {
    pub old_admin: Pubkey,
    pub new_admin: Pubkey,
    pub mint: Pubkey,
    pub timestamp: i64,
}

/// Emitted when channel creator fee is updated.
#[event]
pub struct CreatorFeeUpdated {
    pub admin: Pubkey,
    pub channel_config: Pubkey,
    pub old_fee_bps: u16,
    pub new_fee_bps: u16,
    pub timestamp: i64,
}

/// Emitted when admin manually sets root sequence.
#[event]
pub struct RootSeqRecovered {
    pub admin: Pubkey,
    pub channel_config: Pubkey,
    pub old_seq: u64,
    pub new_seq: u64,
    pub timestamp: i64,
}

/// Emitted when a channel config is closed and rent reclaimed.
#[event]
pub struct ChannelClosed {
    pub channel_config: Pubkey,
    pub admin: Pubkey,
    pub lamports_returned: u64,
    pub timestamp: i64,
}

// =============================================================================
// CHANNEL STAKING EVENTS (V1.2.0)
// =============================================================================

/// Emitted when a user stakes tokens on a channel and receives a soulbound NFT receipt.
#[event]
pub struct ChannelStaked {
    pub user: Pubkey,
    pub channel: Pubkey,
    pub amount: u64,
    pub nft_mint: Pubkey,
    pub lock_duration: u64,
    pub boost_bps: u64,
    pub timestamp: i64,
}

/// Emitted when a user unstakes tokens by burning their soulbound NFT receipt.
#[event]
pub struct ChannelUnstaked {
    pub user: Pubkey,
    pub channel: Pubkey,
    pub amount: u64,
    pub nft_mint: Pubkey,
    pub timestamp: i64,
}

/// Emitted when a user emergency unstakes before lock expiry with penalty.
#[event]
pub struct ChannelEmergencyUnstaked {
    pub user: Pubkey,
    pub channel: Pubkey,
    pub staked_amount: u64,
    pub penalty_amount: u64,
    pub returned_amount: u64,
    pub nft_mint: Pubkey,
    pub remaining_lock_slots: u64,
    pub timestamp: i64,
}

/// Emitted when a user claims accumulated staking rewards.
#[event]
pub struct ChannelRewardsClaimed {
    pub user: Pubkey,
    pub channel: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

/// Emitted when admin updates the reward rate for a channel stake pool.
#[event]
pub struct RewardRateUpdated {
    pub channel: Pubkey,
    pub old_rate: u64,
    pub new_rate: u64,
    pub admin: Pubkey,
    pub timestamp: i64,
}

/// Emitted when admin shuts down a pool for emergency penalty-free exits.
#[event]
pub struct PoolShutdown {
    pub channel: Pubkey,
    pub admin: Pubkey,
    pub reason: String,
    pub staker_count: u64,
    pub total_staked: u64,
    pub timestamp: i64,
}
