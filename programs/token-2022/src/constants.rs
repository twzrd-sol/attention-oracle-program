use anchor_lang::prelude::*;

// Seeds
pub const PROTOCOL_SEED: &[u8] = b"protocol";
pub const TREASURY_SEED: &[u8] = b"treasury";
pub const EPOCH_STATE_SEED: &[u8] = b"epoch_state";
pub const CHANNEL_STATE_SEED: &[u8] = b"channel_state";

pub const CHANNEL_RING_SLOTS: usize = 10;
pub const CHANNEL_MAX_CLAIMS: usize = 1024;
pub const CHANNEL_BITMAP_BYTES: usize = (CHANNEL_MAX_CLAIMS + 7) / 8;

pub const MAX_EPOCH_CLAIMS: u32 = 1_000_000;
pub const MAX_ID_BYTES: usize = 64;

// Token-2022 Config
pub const CCM_DECIMALS: u8 = 9;
pub const INITIAL_SUPPLY: u64 = 1_000_000_000 * 1_000_000_000; // 1B CCM with 9 decimals
                                                               // Transfer Fee Config (0.1% = 10 basis points)
pub const DEFAULT_TRANSFER_FEE_BASIS_POINTS: u16 = 10;
pub const MAX_FEE_BASIS_POINTS: u16 = 1000; // 10% max
pub const MAX_FEE_AMOUNT: u64 = 1_000_000_000_000;

// Epoch force-close grace period (e.g., 7 days)
pub const EPOCH_FORCE_CLOSE_GRACE_SECS: i64 = 7 * 24 * 60 * 60;

// Initial admin authority for singleton protocol initialization
pub const ADMIN_AUTHORITY: Pubkey = Pubkey::new_from_array([
    0x91, 0x16, 0x1c, 0x33, 0x6c, 0x7e, 0x78, 0x23, 0x60, 0x4f, 0x0a, 0x02, 0x2b, 0x2f, 0x10, 0x60,
    0x40, 0xd8, 0x16, 0xa4, 0x79, 0x0e, 0xc9, 0xf2, 0xba, 0x30, 0x45, 0x3b, 0xdb, 0x1b, 0x59, 0x99,
]);
