use crate::constants::{CHANNEL_BITMAP_BYTES, CHANNEL_MAX_CLAIMS, CHANNEL_RING_SLOTS};
use crate::errors::ProtocolError;
use anchor_lang::prelude::*;

/// Global protocol state (singleton)
#[account]
pub struct ProtocolState {
    /// Initialization flag
    pub is_initialized: bool,

    /// Current version for migrations
    pub version: u8,

    /// Admin authority (can update config)
    pub admin: Pubkey,

    /// Allowlisted publisher (optional). If set, this key may publish
    /// merkle roots in addition to the admin. If Pubkey::default(),
    /// only the admin is authorized.
    pub publisher: Pubkey,

    /// Treasury PDA
    pub treasury: Pubkey,

    /// CCM mint address
    pub mint: Pubkey,

    /// Emergency pause flag
    pub paused: bool,

    /// Require an external receipt for claims (default: false)
    /// Toggle via set_policy instruction for circuit breaker pattern
    pub require_receipt: bool,

    /// Bump seed for PDA
    pub bump: u8,
}

impl ProtocolState {
    pub const LEN: usize = 8 +  // discriminator
        1 +    // is_initialized
        1 +    // version
        32 +   // admin
        32 +   // publisher
        32 +   // treasury
        32 +   // mint
        1 +    // paused
        1 +    // require_receipt
        1; // bump
}

/// Fee configuration (PDA account)
#[account]
pub struct FeeConfig {
    /// Transfer fee in basis points (10 = 0.1%)
    pub basis_points: u16,

    /// Maximum fee amount
    pub max_fee: u64,

    /// Bump seed
    pub bump: u8,
}

impl FeeConfig {
    pub const LEN: usize = 8 + // discriminator
        2 +    // basis_points
        8 +    // max_fee
        1; // bump
}

/// Fee distribution split
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct FeeSplit {
    /// Percentage to LP (0-100)
    pub lp_allocation: u8,

    /// Percentage to treasury (0-100)
    pub treasury_allocation: u8,

    /// Percentage to burn (0-100)
    pub burn_allocation: u8,
}

impl FeeSplit {
    pub const LEN: usize = 1 + 1 + 1;

    pub fn validate(&self) -> Result<()> {
        require!(
            self.lp_allocation + self.treasury_allocation + self.burn_allocation == 100,
            ProtocolError::InvalidFeeSplit
        );
        Ok(())
    }
}

//

//

/// Epoch state for merkle claims (unchanged from v2)
#[account]
pub struct EpochState {
    pub epoch: u64,
    pub root: [u8; 32],
    pub claim_count: u32,
    pub mint: Pubkey,
    pub streamer: Pubkey,
    pub treasury: Pubkey,
    pub timestamp: i64,
    pub bump: u8,
    pub total_claimed: u64,
    pub closed: bool,
    pub claimed_bitmap: Vec<u8>,
}

impl EpochState {
    pub fn space_for(claims: usize) -> usize {
        8 + // discriminator
        8 + // epoch
        32 + // root
        4 + // claim_count
        32 + // mint
        32 + // streamer
        32 + // treasury
        8 + // timestamp
        1 + // bump
        8 + // total_claimed
        1 + // closed
        4 + // vec length
        ((claims + 7) / 8) // bitmap
    }
}

//

/// Per-channel ring buffer state (stores recent epoch merkle roots)
/// Uses zero_copy to avoid stack overflow (1.7KB struct)
#[account(zero_copy)]
#[repr(C, packed)]
pub struct ChannelState {
    pub version: u8,
    pub bump: u8,
    pub mint: Pubkey,
    pub streamer: Pubkey,
    pub latest_epoch: u64,
    pub slots: [ChannelSlot; CHANNEL_RING_SLOTS],
    /// Switchboard price feed: USDC/SOL price (lamports per USDC)
    pub usdc_sol_price: u64,
    /// Timestamp of last price update
    pub price_updated_at: i64,
}

impl ChannelState {
    pub const LEN: usize = 8 /* disc */
        + 1 /* version */
        + 1 /* bump */
        + 32 /* mint */
        + 32 /* streamer */
        + 8 /* latest_epoch */
        + (ChannelSlot::LEN * CHANNEL_RING_SLOTS);

    pub fn slot_index(epoch: u64) -> usize {
        (epoch as usize) % CHANNEL_RING_SLOTS
    }

    pub fn slot(&self, epoch: u64) -> &ChannelSlot {
        &self.slots[Self::slot_index(epoch)]
    }

    pub fn slot_mut(&mut self, epoch: u64) -> &mut ChannelSlot {
        &mut self.slots[Self::slot_index(epoch)]
    }
}

#[zero_copy]
#[repr(C, packed)]
pub struct ChannelSlot {
    pub epoch: u64,
    pub root: [u8; 32],
    pub claim_count: u16,
    pub claimed_bitmap: [u8; CHANNEL_BITMAP_BYTES],
}

impl ChannelSlot {
    pub const LEN: usize = 8 /* epoch */
        + 32 /* root */
        + 2 /* claim_count */
        + CHANNEL_BITMAP_BYTES;

    pub fn reset(&mut self, epoch: u64, root: [u8; 32]) {
        self.epoch = epoch;
        self.root = root;
        self.claim_count = 0;
        self.claimed_bitmap = [0u8; CHANNEL_BITMAP_BYTES];
    }

    pub fn test_bit(&self, index: usize) -> bool {
        let byte = index / 8;
        let bit = index % 8;
        (self.claimed_bitmap[byte] & (1u8 << bit)) != 0
    }

    pub fn set_bit(&mut self, index: usize) {
        let byte = index / 8;
        let bit = index % 8;
        self.claimed_bitmap[byte] |= 1u8 << bit;
    }

    pub fn validate_index(index: usize) -> Result<()> {
        require!(index < CHANNEL_MAX_CLAIMS, ProtocolError::InvalidIndex);
        Ok(())
    }
}
