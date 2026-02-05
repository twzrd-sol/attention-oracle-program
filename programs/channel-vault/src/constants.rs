//! Constants for ChannelVault.

use anchor_lang::prelude::*;

// =============================================================================
// PDA SEEDS
// =============================================================================

/// Seed for ChannelVault PDA: ["vault", channel_config]
pub const VAULT_SEED: &[u8] = b"vault";

/// Seed for vault's CCM buffer: ["vault_ccm", vault]
pub const VAULT_CCM_BUFFER_SEED: &[u8] = b"vault_ccm";

/// Seed for vLOFI mint: ["vlofi", vault]
pub const VLOFI_MINT_SEED: &[u8] = b"vlofi";

/// Seed for vault's Oracle position tracker: ["vault_oracle", vault]
pub const VAULT_ORACLE_POSITION_SEED: &[u8] = b"vault_oracle";

/// Seed for withdraw requests: ["withdraw", vault, user, request_id]
pub const WITHDRAW_REQUEST_SEED: &[u8] = b"withdraw";

// =============================================================================
// STAKING PARAMETERS
// =============================================================================

/// Minimum initial deposit to prevent first depositor attacks
pub const MIN_INITIAL_DEPOSIT: u64 = 1_000_000_000_000; // 1000 CCM (9 decimals)

/// Minimum deposit amount for subsequent deposits
pub const MIN_DEPOSIT: u64 = 1_000_000_000; // 1 CCM (9 decimals)

// NOTE: Lock duration and withdrawal queue duration are now per-vault config.
// Suggested values for initialize_vault():
//   - Trial:      lock_duration_slots = 3 * SLOTS_PER_HOUR (3 hours)
//   - Production: lock_duration_slots = 7 * SLOTS_PER_DAY  (7 days, 1.25x boost)

// =============================================================================
// TOKEN STANDARDS
// =============================================================================

/// Token-2022 program ID (TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb)
pub const TOKEN_2022_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    0x06, 0xdd, 0xf6, 0xe1, 0xee, 0x75, 0x8f, 0xde, 0x18, 0x42, 0x5d, 0xbc, 0xe4, 0x6c, 0xcd, 0xda,
    0xb6, 0x1a, 0xfc, 0x4d, 0x83, 0xb9, 0x0d, 0x27, 0xfe, 0xbd, 0xf9, 0x28, 0xd8, 0xa1, 0x8b, 0xfc,
]);

// =============================================================================
// PRECISION
// =============================================================================

/// Virtual offset for share calculations (ERC4626-style protection)
pub const VIRTUAL_SHARES: u64 = 1_000_000_000; // 1e9
pub const VIRTUAL_ASSETS: u64 = 1_000_000_000; // 1e9

// =============================================================================
// EMERGENCY RESERVE
// =============================================================================

/// Emergency reserve cap (5% of NAV in basis points)
/// Reserve is funded by instant redeem penalties and capped at this % of NAV
pub const RESERVE_CAP_BPS: u64 = 500;

/// Emergency/instant redeem penalty (20% in basis points)
pub const EMERGENCY_PENALTY_BPS: u64 = 2000;

/// Basis points denominator
pub const BPS_DENOMINATOR: u64 = 10_000;

// =============================================================================
// KEEPER INCENTIVES
// =============================================================================

/// Compound keeper bounty (basis points of rewards claimed).
/// Paid from claimed rewards only, never from principal.
pub const COMPOUND_BOUNTY_BPS: u64 = 10; // 0.10%

// =============================================================================
// EXTERNAL PROGRAMS
// =============================================================================

/// Metaplex Token Metadata program ID (metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s)
pub const METADATA_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    11, 112, 101, 177, 227, 209, 124, 69, 56, 157, 82, 127, 107, 4, 195, 205,
    88, 184, 108, 115, 26, 160, 253, 181, 73, 182, 209, 188, 3, 248, 41, 70,
]);
