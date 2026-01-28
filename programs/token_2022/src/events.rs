use anchor_lang::prelude::*;

#[event]
pub struct PassportMinted {
    pub user_hash: [u8; 32],
    pub owner: Pubkey,
    pub tier: u8,
    pub score: u64,
    pub updated_at: i64,
}

#[event]
pub struct PassportUpgraded {
    pub user_hash: [u8; 32],
    pub owner: Pubkey,
    pub new_tier: u8,
    pub new_score: u64,
    pub epoch_count: u32,
    pub weighted_presence: u64,
    pub badges: u32,
    pub leaf_hash: Option<[u8; 32]>,
    pub updated_at: i64,
}

#[event]
pub struct PassportReissued {
    pub user_hash: [u8; 32],
    pub old_owner: Pubkey,
    pub new_owner: Pubkey,
    pub updated_at: i64,
}

#[event]
pub struct PassportRevoked {
    pub user_hash: [u8; 32],
    pub owner: Pubkey,
    pub updated_at: i64,
}

#[event]
pub struct CumulativeRewardsClaimed {
    pub channel: Pubkey,
    pub claimer: Pubkey,
    pub user_amount: u64,
    pub creator_amount: u64,
    pub cumulative_total: u64,
    pub root_seq: u64,
}

/// Emitted when rewards are claimed directly into staking (invisible staking).
/// No liquid tokens hit the user's wallet.
#[event]
pub struct InvisibleStaked {
    pub channel: Pubkey,
    pub claimer: Pubkey,
    pub staked_amount: u64,
    pub creator_amount: u64,
    pub cumulative_total: u64,
    pub root_seq: u64,
    pub total_staked: u64,
}

/// Emitted when admin withdraws from treasury.
#[event]
pub struct TreasuryWithdrawn {
    pub admin: Pubkey,
    pub destination: Pubkey,
    pub amount: u64,
    pub withdrawn_today: u64,
    pub total_withdrawn: u64,
    pub timestamp: i64,
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

/// Emitted when receipt requirement policy changes.
#[event]
pub struct PolicyUpdated {
    pub admin: Pubkey,
    pub require_receipt: bool,
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

/// Emitted when a passport account is closed and rent returned.
#[event]
pub struct PassportAccountClosed {
    pub user_hash: [u8; 32],
    pub owner: Pubkey,
    pub rent_returned_to: Pubkey,
    pub lamports_returned: u64,
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