use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke_signed, system_instruction};

use crate::constants::{CHANNEL_BITMAP_BYTES, CHANNEL_STATE_SEED, PROTOCOL_SEED};
use crate::errors::OracleError;
use crate::state::{ChannelState, ProtocolState};

/// Old constants for migration (pre-upgrade values)
const OLD_CHANNEL_MAX_CLAIMS: usize = 1024;
const OLD_CHANNEL_BITMAP_BYTES: usize = (OLD_CHANNEL_MAX_CLAIMS + 7) / 8; // 128 bytes
const CHANNEL_RING_SLOTS: usize = 10;

/// Old slot size: epoch(8) + root(32) + claim_count(2) + padding(6) + bitmap(128) = 176
const OLD_SLOT_SIZE: usize = 8 + 32 + 2 + 6 + OLD_CHANNEL_BITMAP_BYTES;

/// New slot size: epoch(8) + root(32) + claim_count(2) + padding(6) + bitmap(512) = 560
const NEW_SLOT_SIZE: usize = 8 + 32 + 2 + 6 + CHANNEL_BITMAP_BYTES;

/// Old header: version(1) + bump(1) + mint(32) + subject(32) + padding(6) + latest_epoch(8) = 80
const HEADER_SIZE: usize = 1 + 1 + 32 + 32 + 6 + 8;

/// Old total: disc(8) + header(80) + slots(176*10) = 8 + 80 + 1760 = 1848 (but actual is 728?)
/// Actual old account: 728 bytes - need to handle smaller account
const OLD_ACCOUNT_SIZE: usize = 728;

/// New total: disc(8) + header(80) + slots(560*10) = 5688
const NEW_ACCOUNT_SIZE: usize = 8 + std::mem::size_of::<ChannelState>();

#[derive(Accounts)]
pub struct MigrateChannelState<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// The channel state account to migrate.
    /// Using UncheckedAccount because AccountLoader would panic on size mismatch.
    /// CHECK: PDA is validated in handler, owner verified as this program.
    #[account(mut)]
    pub channel_state: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn migrate_channel_state(ctx: Context<MigrateChannelState>, channel: String) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;
    let payer = &ctx.accounts.payer;

    // Authorization: admin or publisher
    let signer = payer.key();
    let is_admin = signer == protocol_state.admin;
    let is_publisher =
        protocol_state.publisher != Pubkey::default() && signer == protocol_state.publisher;
    require!(is_admin || is_publisher, OracleError::Unauthorized);

    let channel_info = &ctx.accounts.channel_state;
    let old_size = channel_info.data_len();

    // Validate it needs migration
    require!(old_size < NEW_ACCOUNT_SIZE, OracleError::AccountTooLarge);
    msg!(
        "Migrating channel state: {} bytes → {} bytes",
        old_size,
        NEW_ACCOUNT_SIZE
    );

    // Verify ownership
    require!(
        channel_info.owner == ctx.program_id,
        OracleError::InvalidChannelState
    );

    // Read header data before realloc (first 88 bytes: 8 disc + 80 header)
    let old_data = channel_info.try_borrow_data()?;
    require!(old_data.len() >= 88, OracleError::InvalidChannelState);

    // Extract header fields
    let discriminator = &old_data[0..8];
    let version = old_data[8];
    let bump = old_data[9];
    let mint = Pubkey::try_from(&old_data[10..42]).unwrap();
    let subject = Pubkey::try_from(&old_data[42..74]).unwrap();
    let _padding = &old_data[74..80];
    let latest_epoch = u64::from_le_bytes(old_data[80..88].try_into().unwrap());

    // Validate PDA
    let seeds = [CHANNEL_STATE_SEED, mint.as_ref(), subject.as_ref()];
    let (expected_pda, _) = Pubkey::find_program_address(&seeds, ctx.program_id);
    require_keys_eq!(expected_pda, channel_info.key(), OracleError::InvalidChannelState);

    // Validate discriminator
    use anchor_lang::Discriminator;
    require!(
        discriminator == ChannelState::DISCRIMINATOR,
        OracleError::InvalidChannelState
    );

    // Extract old slot data (what we can read)
    // Old layout: each slot is 176 bytes starting at offset 88
    let mut old_slots_data = [[0u8; OLD_SLOT_SIZE]; CHANNEL_RING_SLOTS];
    let slots_start = 88;
    let available_slot_bytes = old_size.saturating_sub(slots_start);
    let readable_slots = available_slot_bytes / OLD_SLOT_SIZE;

    for i in 0..readable_slots.min(CHANNEL_RING_SLOTS) {
        let start = slots_start + i * OLD_SLOT_SIZE;
        let end = start + OLD_SLOT_SIZE;
        if end <= old_data.len() {
            old_slots_data[i].copy_from_slice(&old_data[start..end]);
        }
    }

    // Release borrow before realloc
    drop(old_data);

    // Calculate rent for new size
    let rent = Rent::get()?;
    let new_lamports = rent.minimum_balance(NEW_ACCOUNT_SIZE);
    let current_lamports = channel_info.lamports();

    // Fund the account if needed
    if new_lamports > current_lamports {
        let diff = new_lamports - current_lamports;
        invoke_signed(
            &system_instruction::transfer(payer.key, channel_info.key, diff),
            &[
                payer.to_account_info(),
                channel_info.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[], // No seeds needed for transfer
        )?;
    }

    // Realloc the account
    channel_info.realloc(NEW_ACCOUNT_SIZE, false)?;

    // Write new data
    let mut new_data = channel_info.try_borrow_mut_data()?;

    // Write header (same as before)
    new_data[0..8].copy_from_slice(&ChannelState::DISCRIMINATOR);
    new_data[8] = version;
    new_data[9] = bump;
    new_data[10..42].copy_from_slice(mint.as_ref());
    new_data[42..74].copy_from_slice(subject.as_ref());
    new_data[74..80].fill(0); // padding
    new_data[80..88].copy_from_slice(&latest_epoch.to_le_bytes());

    // Write new slots with expanded bitmap
    for i in 0..CHANNEL_RING_SLOTS {
        let new_start = 88 + i * NEW_SLOT_SIZE;

        // Copy epoch (8 bytes)
        new_data[new_start..new_start + 8].copy_from_slice(&old_slots_data[i][0..8]);

        // Copy root (32 bytes)
        new_data[new_start + 8..new_start + 40].copy_from_slice(&old_slots_data[i][8..40]);

        // Copy claim_count (2 bytes)
        new_data[new_start + 40..new_start + 42].copy_from_slice(&old_slots_data[i][40..42]);

        // Copy padding (6 bytes)
        new_data[new_start + 42..new_start + 48].copy_from_slice(&old_slots_data[i][42..48]);

        // Copy old bitmap (128 bytes) then zero-pad rest (384 bytes)
        new_data[new_start + 48..new_start + 48 + OLD_CHANNEL_BITMAP_BYTES]
            .copy_from_slice(&old_slots_data[i][48..48 + OLD_CHANNEL_BITMAP_BYTES]);
        new_data[new_start + 48 + OLD_CHANNEL_BITMAP_BYTES..new_start + NEW_SLOT_SIZE].fill(0);
    }

    msg!(
        "Migration complete: channel={}, old_size={}, new_size={}",
        channel,
        old_size,
        NEW_ACCOUNT_SIZE
    );

    Ok(())
}
