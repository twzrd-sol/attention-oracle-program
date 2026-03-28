use anchor_lang::prelude::*;

// =============================================================================
// GLOBAL ROOT (V4) EVENTS
// =============================================================================

#[event]
pub struct GlobalRootPublished {
    pub mint: Pubkey,
    pub root_seq: u64,
    pub root: [u8; 32],
    pub dataset_hash: [u8; 32],
    pub publisher: Pubkey,
    pub slot: u64,
}

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

#[event]
pub struct MarketTokensInitialized {
    pub market: Pubkey,
    pub market_id: u64,
    pub vault: Pubkey,
    pub yes_mint: Pubkey,
    pub no_mint: Pubkey,
    pub mint_authority: Pubkey,
}

#[event]
pub struct SharesMinted {
    pub market: Pubkey,
    pub market_id: u64,
    pub depositor: Pubkey,
    pub deposit_amount: u64,
    pub net_amount: u64,
    pub shares_minted: u64,
}

#[event]
pub struct SharesRedeemed {
    pub market: Pubkey,
    pub market_id: u64,
    pub redeemer: Pubkey,
    pub shares_burned: u64,
    pub ccm_returned: u64,
}

#[event]
pub struct MarketSettled {
    pub market: Pubkey,
    pub market_id: u64,
    pub settler: Pubkey,
    pub winning_side: bool,
    pub shares_burned: u64,
    pub ccm_returned: u64,
}

#[event]
pub struct MarketSwept {
    pub market: Pubkey,
    pub market_id: u64,
    pub admin: Pubkey,
    pub amount_swept: u64,
    pub treasury: Pubkey,
}

#[event]
pub struct MarketClosed {
    pub market: Pubkey,
    pub market_id: u64,
    pub admin: Pubkey,
}

#[event]
pub struct MarketMintsClosed {
    pub market_id: u64,
    pub yes_mint: Pubkey,
    pub no_mint: Pubkey,
    pub admin: Pubkey,
}

#[event]
pub struct MintFeesWithdrawn {
    pub mint: Pubkey,
    pub destination: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

// =============================================================================
// ADMIN EVENTS
// =============================================================================

#[event]
pub struct PublisherUpdated {
    pub admin: Pubkey,
    pub old_publisher: Pubkey,
    pub new_publisher: Pubkey,
    pub mint: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct ProtocolPaused {
    pub admin: Pubkey,
    pub paused: bool,
    pub mint: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct AdminTransferred {
    pub old_admin: Pubkey,
    pub new_admin: Pubkey,
    pub mint: Pubkey,
    pub timestamp: i64,
}

// =============================================================================
// CHANNEL STAKING EVENTS
// =============================================================================

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

#[event]
pub struct ChannelUnstaked {
    pub user: Pubkey,
    pub channel: Pubkey,
    pub amount: u64,
    pub nft_mint: Pubkey,
    pub timestamp: i64,
}

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

#[event]
pub struct ChannelRewardsClaimed {
    pub user: Pubkey,
    pub channel: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct RewardRateUpdated {
    pub channel: Pubkey,
    pub old_rate: u64,
    pub new_rate: u64,
    pub admin: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct PoolShutdown {
    pub channel: Pubkey,
    pub admin: Pubkey,
    pub reason: String,
    pub staker_count: u64,
    pub total_staked: u64,
    pub timestamp: i64,
}

#[event]
pub struct PoolClosed {
    pub channel: Pubkey,
    pub admin: Pubkey,
    pub tokens_recovered: u64,
    pub timestamp: i64,
}

#[event]
pub struct PoolRecovered {
    pub pool: Pubkey,
    pub channel: Pubkey,
    pub total_staked: u64,
    pub staker_count: u64,
    pub was_shutdown: bool,
}
