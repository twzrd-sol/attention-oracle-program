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

/// V3 claim event with stake snapshot binding (anti-gaming)
#[event]
pub struct CumulativeRewardsClaimedV3 {
    pub channel: Pubkey,
    pub claimer: Pubkey,
    pub user_amount: u64,
    pub creator_amount: u64,
    pub cumulative_total: u64,
    pub root_seq: u64,
    /// Stake amount at snapshot time (bound to merkle leaf)
    pub stake_snapshot: u64,
    /// Current stake amount at claim time
    pub current_stake: u64,
}

/// Global V4 root publish event (single root shared across channels/subjects).
#[event]
pub struct GlobalRootPublished {
    pub mint: Pubkey,
    pub root_seq: u64,
    pub root: [u8; 32],
    pub dataset_hash: [u8; 32],
    pub publisher: Pubkey,
    pub slot: u64,
}

/// V4 global claim event with subject + stake snapshot binding.
#[event]
pub struct GlobalRewardsClaimedV4 {
    pub subject: Pubkey,
    pub claimer: Pubkey,
    pub user_amount: u64,
    pub creator_amount: u64,
    pub cumulative_total: u64,
    pub root_seq: u64,
    pub stake_snapshot: u64,
    pub current_stake: u64,
}

// =============================================================================
// GLOBAL ROOT (V4) EVENTS — Simple per-user totals
// =============================================================================

/// V4 global claim event (single root, per-user totals, no channel scope)
#[event]
pub struct GlobalRewardsClaimed {
    pub claimer: Pubkey,
    pub amount: u64,
    pub cumulative_total: u64,
    pub root_seq: u64,
}

// =============================================================================
// CREATOR MARKETS EVENTS
// =============================================================================

/// Emitted when a new creator market is initialized.
#[event]
pub struct MarketCreated {
    pub market: Pubkey,
    pub market_id: u64,
    pub authority: Pubkey,
    pub creator_wallet: Pubkey,
    pub mint: Pubkey,
    pub metric: u8,
    pub target: u64,
    pub resolution_root_seq: u64,
    pub created_slot: u64,
}

/// Emitted when a creator market is resolved against a published global root.
#[event]
pub struct MarketResolved {
    pub market: Pubkey,
    pub market_id: u64,
    pub resolver: Pubkey,
    pub creator_wallet: Pubkey,
    pub metric: u8,
    pub target: u64,
    pub resolution_root_seq: u64,
    pub verified_cumulative_total: u64,
    pub outcome: bool,
    pub resolved_slot: u64,
}

/// Emitted when market tokens (vault, YES/NO mints) are initialized.
#[event]
pub struct MarketTokensInitialized {
    pub market: Pubkey,
    pub market_id: u64,
    pub vault: Pubkey,
    pub yes_mint: Pubkey,
    pub no_mint: Pubkey,
    pub mint_authority: Pubkey,
}

/// Emitted when a user deposits CCM and receives YES + NO shares.
#[event]
pub struct SharesMinted {
    pub market: Pubkey,
    pub market_id: u64,
    pub depositor: Pubkey,
    /// Gross CCM amount user intended to deposit
    pub deposit_amount: u64,
    /// Net CCM received in vault after Token-2022 transfer fee
    pub net_amount: u64,
    /// YES + NO shares minted (equals net_amount)
    pub shares_minted: u64,
}

/// Emitted when a user burns equal YES + NO shares to reclaim CCM (pre-resolution).
#[event]
pub struct SharesRedeemed {
    pub market: Pubkey,
    pub market_id: u64,
    pub redeemer: Pubkey,
    pub shares_burned: u64,
    /// CCM transferred out (net after outbound transfer fee)
    pub ccm_returned: u64,
}

/// Emitted when a user burns winning shares to claim CCM (post-resolution).
#[event]
pub struct MarketSettled {
    pub market: Pubkey,
    pub market_id: u64,
    pub settler: Pubkey,
    pub winning_side: bool,
    pub shares_burned: u64,
    /// CCM transferred out (net after outbound transfer fee)
    pub ccm_returned: u64,
}

/// Withheld fees withdrawn from mint to treasury
#[event]
pub struct MintFeesWithdrawn {
    pub mint: Pubkey,
    pub destination: Pubkey,
    pub amount: u64,
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

/// Emitted when admin updates the V2 cutover epoch on a channel.
#[event]
pub struct CutoverEpochUpdated {
    pub admin: Pubkey,
    pub channel_config: Pubkey,
    pub old_epoch: u64,
    pub new_epoch: u64,
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

/// Emitted when admin closes a fully-emptied shutdown pool, recovering surplus CCM.
#[event]
pub struct PoolClosed {
    pub channel: Pubkey,
    pub admin: Pubkey,
    pub tokens_recovered: u64,
    pub timestamp: i64,
}

/// Emitted when admin recovers a shutdown pool without state loss.
#[event]
pub struct PoolRecovered {
    pub pool: Pubkey,
    pub channel: Pubkey,
    pub total_staked: u64,
    pub staker_count: u64,
    pub was_shutdown: bool,
}
