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
pub struct TransferFeeEvent {
    pub transfer_amount: u64,
    pub total_fee: u64,
    pub treasury_fee: u64,
    pub creator_fee: u64,
    pub creator_tier: u8,
    pub tier_multiplier: u32, // stored as fixed-point (e.g., 2000 = 0.2)
    pub timestamp: i64,
}
