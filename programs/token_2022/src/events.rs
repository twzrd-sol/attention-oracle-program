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