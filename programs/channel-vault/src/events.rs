//! Event definitions for ChannelVault.

use anchor_lang::prelude::*;

/// Emitted when a new vault is initialized.
#[event]
pub struct VaultInitialized {
    pub vault: Pubkey,
    pub channel_config: Pubkey,
    pub ccm_mint: Pubkey,
    pub vlofi_mint: Pubkey,
    pub admin: Pubkey,
    pub timestamp: i64,
}

/// Emitted when a user deposits CCM and receives vLOFI.
#[event]
pub struct Deposited {
    pub user: Pubkey,
    pub vault: Pubkey,
    pub ccm_amount: u64,
    pub shares_minted: u64,
    /// CCM per share (scaled by 1e9)
    pub exchange_rate: u64,
    pub timestamp: i64,
}

/// Emitted when pending deposits are compounded into Oracle.
#[event]
pub struct Compounded {
    pub vault: Pubkey,
    /// CCM staked in this compound
    pub pending_staked: u64,
    /// Total CCM now staked in Oracle
    pub total_staked: u64,
    /// Rewards claimed and restaked
    pub rewards_claimed: u64,
    /// Compound count
    pub compound_count: u64,
    /// Caller who triggered compound
    pub caller: Pubkey,
    pub timestamp: i64,
}

/// Emitted when a user requests withdrawal.
#[event]
pub struct WithdrawRequested {
    pub user: Pubkey,
    pub vault: Pubkey,
    pub request_id: u64,
    pub shares_burned: u64,
    pub ccm_amount: u64,
    /// Slot when withdrawal can be completed
    pub completion_slot: u64,
    pub timestamp: i64,
}

/// Emitted when a withdrawal is completed.
#[event]
pub struct WithdrawCompleted {
    pub user: Pubkey,
    pub vault: Pubkey,
    pub request_id: u64,
    pub ccm_returned: u64,
    pub timestamp: i64,
}

/// Emitted when a user does instant redemption (20% penalty, from buffer/reserve).
#[event]
pub struct InstantRedeemed {
    pub user: Pubkey,
    pub vault: Pubkey,
    pub shares_burned: u64,
    pub ccm_gross: u64,
    /// Amount after 20% penalty
    pub ccm_returned: u64,
    /// Amount added to emergency reserve
    pub penalty_to_reserve: u64,
    /// Reserve balance after
    pub reserve_balance: u64,
    pub timestamp: i64,
}

/// Emitted when admin triggers Oracle emergency unstake (extreme scenario).
/// This affects ALL stakers - only for catastrophic situations.
#[event]
pub struct AdminEmergencyUnstaked {
    pub vault: Pubkey,
    pub admin: Pubkey,
    /// Total CCM that was staked in Oracle
    pub oracle_stake_before: u64,
    /// CCM returned after 20% Oracle penalty
    pub ccm_returned: u64,
    /// CCM burned by Oracle penalty
    pub oracle_penalty: u64,
    pub timestamp: i64,
}

/// Emitted when vault is paused.
#[event]
pub struct VaultPaused {
    pub vault: Pubkey,
    pub admin: Pubkey,
    pub timestamp: i64,
}

/// Emitted when vault is resumed.
#[event]
pub struct VaultResumed {
    pub vault: Pubkey,
    pub admin: Pubkey,
    pub timestamp: i64,
}

/// Emitted when admin is updated.
#[event]
pub struct AdminUpdated {
    pub vault: Pubkey,
    pub old_admin: Pubkey,
    pub new_admin: Pubkey,
    pub timestamp: i64,
}

/// Emitted when a vault is closed.
#[event]
pub struct VaultClosed {
    pub vault: Pubkey,
    pub channel_config: Pubkey,
    pub admin: Pubkey,
    pub timestamp: i64,
}

/// Emitted when admin syncs the oracle position state.
#[event]
pub struct OraclePositionSynced {
    pub vault: Pubkey,
    pub admin: Pubkey,
    pub is_active: bool,
    pub stake_amount: u64,
    pub lock_end_slot: u64,
    pub timestamp: i64,
}
