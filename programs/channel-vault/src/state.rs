//! On-chain state definitions for ChannelVault.

use anchor_lang::prelude::*;

use crate::constants::{VIRTUAL_ASSETS, VIRTUAL_SHARES};
use crate::errors::VaultError;

// =============================================================================
// CHANNEL VAULT
// =============================================================================

/// Main vault state for a specific channel.
/// Seeds: ["vault", channel_config]
#[account]
pub struct ChannelVault {
    /// PDA bump
    pub bump: u8,
    /// Version for future migrations
    pub version: u8,
    /// Oracle's ChannelConfigV2 this vault stakes to
    pub channel_config: Pubkey,
    /// CCM token mint (Token-2022)
    pub ccm_mint: Pubkey,
    /// vLOFI share token mint (standard SPL)
    pub vlofi_mint: Pubkey,
    /// Vault's CCM buffer token account
    pub ccm_buffer: Pubkey,
    /// Total CCM staked in Oracle (excludes pending)
    pub total_staked: u64,
    /// Total vLOFI shares outstanding
    pub total_shares: u64,
    /// Pending CCM awaiting next compound
    pub pending_deposits: u64,
    /// Pending CCM reserved for withdrawal queue
    pub pending_withdrawals: u64,
    /// Last compound slot
    pub last_compound_slot: u64,
    /// Compound count (for analytics)
    pub compound_count: u64,
    /// Admin authority
    pub admin: Pubkey,
    /// Minimum deposit amount
    pub min_deposit: u64,
    /// Is vault paused?
    pub paused: bool,
    /// Emergency reserve balance (funded by instant redeem penalties)
    pub emergency_reserve: u64,
    /// Lock duration for Oracle stakes (slots)
    pub lock_duration_slots: u64,
    /// Withdrawal queue duration (slots)
    pub withdraw_queue_slots: u64,
    /// Reserved for future use
    pub _reserved: [u8; 40],
}

impl ChannelVault {
    pub const LEN: usize = 8  // discriminator
        + 1   // bump
        + 1   // version
        + 32  // channel_config
        + 32  // ccm_mint
        + 32  // vlofi_mint
        + 32  // ccm_buffer
        + 8   // total_staked
        + 8   // total_shares
        + 8   // pending_deposits
        + 8   // pending_withdrawals
        + 8   // last_compound_slot
        + 8   // compound_count
        + 32  // admin
        + 8   // min_deposit
        + 1   // paused
        + 8   // emergency_reserve
        + 8   // lock_duration_slots
        + 8   // withdraw_queue_slots
        + 40; // _reserved

    /// Calculate net assets available for share pricing.
    /// NAV = total_staked + pending_deposits + emergency_reserve - pending_withdrawals
    ///
    /// emergency_reserve IS included in NAV (Option B) - it's shareholder assets
    /// that can be used for instant redeems. This keeps accounting simple and
    /// ensures depositors share in the reserve built from penalties.
    ///
    /// pending_withdrawals is subtracted because it's already committed to queued users.
    fn net_assets(&self) -> Result<u64> {
        let gross = self.total_staked
            .checked_add(self.pending_deposits)
            .ok_or(VaultError::MathOverflow)?
            .checked_add(self.emergency_reserve)
            .ok_or(VaultError::MathOverflow)?;

        // Subtract pending_withdrawals (already committed)
        // Fail if insolvent rather than silently returning 0
        if self.pending_withdrawals > gross {
            return Err(VaultError::VaultInsolvent.into());
        }
        let net = gross - self.pending_withdrawals;

        Ok(net)
    }

    /// Calculate available liquidity for instant redeems.
    /// This is the buffer balance minus what's reserved for queued withdrawals.
    pub fn available_for_instant_redeem(&self, buffer_balance: u64) -> Result<u64> {
        // Available = buffer_balance - pending_withdrawals
        if self.pending_withdrawals > buffer_balance {
            return Err(VaultError::InsufficientReserve.into());
        }
        Ok(buffer_balance - self.pending_withdrawals)
    }

    /// Calculate shares for a deposit amount using virtual offset pattern (ERC4626).
    /// This prevents first-depositor inflation attacks.
    pub fn calculate_shares(&self, deposit_amount: u64) -> Result<u64> {
        let total_assets = self.net_assets()?
            .checked_add(VIRTUAL_ASSETS)
            .ok_or(VaultError::MathOverflow)?;

        let total_supply = self.total_shares
            .checked_add(VIRTUAL_SHARES)
            .ok_or(VaultError::MathOverflow)?;

        // shares = deposit_amount * total_supply / total_assets
        let shares = (deposit_amount as u128)
            .checked_mul(total_supply as u128)
            .ok_or(VaultError::MathOverflow)?
            .checked_div(total_assets as u128)
            .ok_or(VaultError::MathOverflow)? as u64;

        Ok(shares)
    }

    /// Calculate CCM amount for share redemption using virtual offset pattern.
    pub fn calculate_redeem_amount(&self, shares: u64) -> Result<u64> {
        let total_assets = self.net_assets()?
            .checked_add(VIRTUAL_ASSETS)
            .ok_or(VaultError::MathOverflow)?;

        let total_supply = self.total_shares
            .checked_add(VIRTUAL_SHARES)
            .ok_or(VaultError::MathOverflow)?;

        // amount = shares * total_assets / total_supply
        let amount = (shares as u128)
            .checked_mul(total_assets as u128)
            .ok_or(VaultError::MathOverflow)?
            .checked_div(total_supply as u128)
            .ok_or(VaultError::MathOverflow)? as u64;

        Ok(amount)
    }

    /// Get current exchange rate (CCM per vLOFI share, scaled by 1e9).
    pub fn exchange_rate(&self) -> Result<u64> {
        let total_assets = self.net_assets()?;

        if self.total_shares == 0 {
            return Ok(1_000_000_000); // 1:1 ratio
        }

        // rate = total_assets * 1e9 / total_shares
        let rate = (total_assets as u128)
            .checked_mul(1_000_000_000)
            .ok_or(VaultError::MathOverflow)?
            .checked_div(self.total_shares as u128)
            .ok_or(VaultError::MathOverflow)? as u64;

        Ok(rate)
    }

    /// Calculate the reserve cap based on current NAV.
    pub fn reserve_cap(&self) -> Result<u64> {
        use crate::constants::{BPS_DENOMINATOR, RESERVE_CAP_BPS};

        let nav = self.net_assets()?;
        let cap = (nav as u128)
            .checked_mul(RESERVE_CAP_BPS as u128)
            .ok_or(VaultError::MathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(VaultError::MathOverflow)? as u64;

        Ok(cap)
    }

    /// Add to emergency reserve (capped at 5% of NAV).
    pub fn add_to_reserve(&mut self, amount: u64) -> Result<u64> {
        let cap = self.reserve_cap()?;
        let space = cap.saturating_sub(self.emergency_reserve);
        let to_add = amount.min(space);

        self.emergency_reserve = self.emergency_reserve
            .checked_add(to_add)
            .ok_or(VaultError::MathOverflow)?;

        Ok(to_add)
    }
}

// =============================================================================
// VAULT ORACLE POSITION
// =============================================================================

/// Tracks the vault's stake position in the Oracle.
/// Seeds: ["vault_oracle", vault]
#[account]
pub struct VaultOraclePosition {
    /// PDA bump
    pub bump: u8,
    /// Parent vault
    pub vault: Pubkey,
    /// Oracle's UserChannelStake PDA for this vault
    pub oracle_user_stake: Pubkey,
    /// Oracle's NFT mint (soulbound receipt held by vault)
    pub oracle_nft_mint: Pubkey,
    /// Vault's NFT ATA
    pub oracle_nft_ata: Pubkey,
    /// Is currently staked in oracle?
    pub is_active: bool,
    /// Last known stake amount in Oracle
    pub stake_amount: u64,
    /// Lock end slot from Oracle position
    pub lock_end_slot: u64,
}

impl VaultOraclePosition {
    pub const LEN: usize = 8  // discriminator
        + 1   // bump
        + 32  // vault
        + 32  // oracle_user_stake
        + 32  // oracle_nft_mint
        + 32  // oracle_nft_ata
        + 1   // is_active
        + 8   // stake_amount
        + 8;  // lock_end_slot
}

// =============================================================================
// WITHDRAW REQUEST
// =============================================================================

/// User's pending withdrawal request in the queue.
/// Seeds: ["withdraw", vault, user, request_id]
#[account]
pub struct WithdrawRequest {
    /// PDA bump
    pub bump: u8,
    /// User who requested withdrawal
    pub user: Pubkey,
    /// Vault this request is for
    pub vault: Pubkey,
    /// Request ID (incrementing per user)
    pub request_id: u64,
    /// vLOFI shares burned
    pub shares_burned: u64,
    /// CCM amount locked at request time
    pub ccm_amount: u64,
    /// Slot when request was created
    pub request_slot: u64,
    /// Slot when withdrawal can be completed
    pub completion_slot: u64,
    /// Has this request been completed?
    pub completed: bool,
}

impl WithdrawRequest {
    pub const LEN: usize = 8  // discriminator
        + 1   // bump
        + 32  // user
        + 32  // vault
        + 8   // request_id
        + 8   // shares_burned
        + 8   // ccm_amount
        + 8   // request_slot
        + 8   // completion_slot
        + 1;  // completed
}

// =============================================================================
// USER VAULT STATE (Optional - for tracking request IDs)
// =============================================================================

/// Per-user state for tracking withdrawal request IDs.
/// Seeds: ["user_state", vault, user]
#[account]
pub struct UserVaultState {
    /// PDA bump
    pub bump: u8,
    /// User
    pub user: Pubkey,
    /// Vault
    pub vault: Pubkey,
    /// Next request ID
    pub next_request_id: u64,
    /// Total shares deposited (for analytics)
    pub total_deposited: u64,
    /// Total shares redeemed (for analytics)
    pub total_redeemed: u64,
}

impl UserVaultState {
    pub const LEN: usize = 8  // discriminator
        + 1   // bump
        + 32  // user
        + 32  // vault
        + 8   // next_request_id
        + 8   // total_deposited
        + 8;  // total_redeemed
}
