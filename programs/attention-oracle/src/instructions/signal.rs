//! On-chain signal consumer — read WZRD velocity data atomically via CPI.
//!
//! Any Solana program can CPI into `read_velocity` to get the current
//! attention multiplier for a model/market. This is the READ side of
//! the oracle — `update_attention` writes, `read_velocity` reads.
//!
//! Usage from another program:
//!   1. Pass the UserMarketPosition PDA as an account
//!   2. CPI call `read_velocity`
//!   3. Read return data: velocity_bps (u64) + slot (u64) = 16 bytes
//!
//! For BAM plugin integration:
//!   Bundle [update_attention, read_velocity] = guaranteed fresh signal
//!   in the same block. Consumers pay the tip. Signal producers get
//!   ordered first.
//!
//! Instructions:
//!   - read_velocity      — returns current multiplier_bps from position PDA
//!   - read_market_velocity — returns aggregate velocity from market vault

use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

// ── Position PDA layout offsets ────────────────────────────────

// UserMarketPosition layout (from vault.rs):
//   [0..8]    discriminator
//   [8]       bump
//   [9]       version
//   [10..42]  user_pubkey
//   [42..50]  market_id
//   [50..58]  usdc_deposited
//   [58..66]  vlofi_minted
//   [66..74]  multiplier_bps  ← THIS IS THE VELOCITY SIGNAL
//   [74]      is_settled
//   [75..83]  settled_at_slot
//   [83..91]  created_at (i64 unix timestamp)
const POS_MULTIPLIER_BPS: usize = 66;
const POS_MIN_LEN: usize = 91;
// Anchor discriminators: SHA-256("account:TypeName")[..8]
const DISC_USER_POSITION: [u8; 8] = [0xad, 0xad, 0xd2, 0x13, 0x8d, 0x55, 0xd3, 0x15]; // UserMarketPosition
const DISC_MARKET_VAULT: [u8; 8] = [0x31, 0x09, 0x96, 0x84, 0x7c, 0xa2, 0x89, 0xd0]; // MarketVault

// MarketVault layout
const MV_VELOCITY_EMA: usize = 113; // velocity_ema_primary at offset 113 in MarketVault V2
const MV_MIN_LEN: usize = 121;

#[inline(always)]
fn r64(d: &[u8], o: usize) -> u64 {
    u64::from_le_bytes([
        d[o],
        d[o + 1],
        d[o + 2],
        d[o + 3],
        d[o + 4],
        d[o + 5],
        d[o + 6],
        d[o + 7],
    ])
}

// =============================================================================
// read_velocity — read attention multiplier from a UserMarketPosition PDA
// =============================================================================
//
// Accounts:
//   [0] user_position  — UserMarketPosition PDA (read-only)
//
// Returns via sol_set_return_data:
//   [0..8]  multiplier_bps (u64 LE)
//   [8..16] current_slot (u64 LE)
//
// No signer required — anyone can read. This is a public oracle.

pub fn read_velocity(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _ix_data: &[u8],
) -> ProgramResult {
    if accounts.is_empty() {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let position = &accounts[0];

    // Verify owned by our program
    if !position.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }

    let data = unsafe { position.borrow_data_unchecked() };
    if data.len() < POS_MIN_LEN || data[..8] != DISC_USER_POSITION {
        return Err(ProgramError::InvalidAccountData);
    }

    let multiplier_bps = r64(&data, POS_MULTIPLIER_BPS);
    let clock = Clock::get()?;

    // Return velocity + slot as program return data (16 bytes)
    let mut result = [0u8; 16];
    result[0..8].copy_from_slice(&multiplier_bps.to_le_bytes());
    result[8..16].copy_from_slice(&clock.slot.to_le_bytes());

    pinocchio::program::set_return_data(&result);
    pinocchio::msg!("velocity_read");

    Ok(())
}

// =============================================================================
// read_market_velocity — read aggregate velocity from MarketVault
// =============================================================================
//
// Accounts:
//   [0] market_vault  — MarketVault PDA (read-only)
//
// Returns via sol_set_return_data:
//   [0..8]  velocity_ema (u64 LE) — aggregate market velocity
//   [8..16] current_slot (u64 LE)
//
// This reads the market-level velocity, not per-user.
// For routing decisions: higher velocity = more attention = pick this model.

pub fn read_market_velocity(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _ix_data: &[u8],
) -> ProgramResult {
    if accounts.is_empty() {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let market_vault = &accounts[0];

    if !market_vault.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }

    let data = unsafe { market_vault.borrow_data_unchecked() };

    // Check discriminator — try both V1 and V2
    if data.len() < MV_MIN_LEN {
        return Err(ProgramError::InvalidAccountData);
    }

    // Read velocity_ema from MarketVault
    // V2 layout has velocity at offset 113
    let velocity_ema = if data.len() >= MV_VELOCITY_EMA + 8 {
        r64(&data, MV_VELOCITY_EMA)
    } else {
        0 // V1 layout doesn't have velocity
    };

    let clock = Clock::get()?;

    let mut result = [0u8; 16];
    result[0..8].copy_from_slice(&velocity_ema.to_le_bytes());
    result[8..16].copy_from_slice(&clock.slot.to_le_bytes());

    pinocchio::program::set_return_data(&result);
    pinocchio::msg!("market_velocity_read");

    Ok(())
}
