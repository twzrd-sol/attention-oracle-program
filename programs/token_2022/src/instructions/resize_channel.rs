use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke_signed, system_instruction};
use anchor_lang::Discriminator;

use crate::constants::{CHANNEL_STATE_SEED, PROTOCOL_SEED};
use crate::errors::OracleError;
use crate::state::{ChannelSlot, ChannelState, ProtocolState};

/// Header bytes: discriminator (8) + version (1) + bump (1) + mint (32) + subject (32) + padding (6) + latest_epoch (8)
const HEADER_BYTES: usize = 8 + 1 + 1 + 32 + 32 + 6 + 8;

/// Resize a `ChannelState` PDA to match the current program's `CHANNEL_RING_SLOTS`.
///
/// This is required when the ring size increases (e.g., 10 → 2016) because the
/// `ChannelState` zero-copy struct size changes and older accounts become too small.
#[derive(Accounts)]
pub struct ResizeChannelState<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// The channel state PDA to resize. We keep this as `UncheckedAccount` so the
    /// instruction can operate on older, smaller account layouts.
    /// CHECK: PDA + owner validated in handler.
    #[account(mut)]
    pub channel_state: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn resize_channel_state(ctx: Context<ResizeChannelState>) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;

    // Authorization: admin or allowlisted publisher
    let signer = ctx.accounts.payer.key();
    let is_admin = signer == protocol_state.admin;
    let is_publisher =
        protocol_state.publisher != Pubkey::default() && signer == protocol_state.publisher;
    require!(is_admin || is_publisher, OracleError::Unauthorized);

    let channel_info = &ctx.accounts.channel_state;

    // Verify ownership
    require!(
        channel_info.owner == ctx.program_id,
        OracleError::InvalidChannelState
    );

    let target_size = ChannelState::LEN;
    let current_size = channel_info.data_len();

    // No-op if already at target size
    if current_size == target_size {
        msg!("ChannelState already at target size: {} bytes", target_size);
        return Ok(());
    }

    // Only allow growth (never shrink via this ix)
    require!(current_size < target_size, OracleError::AccountTooLarge);

    // Read and validate the existing header + slot data
    let old_data = channel_info.try_borrow_data()?;
    require!(old_data.len() >= HEADER_BYTES, OracleError::InvalidChannelState);
    require!(
        &old_data[0..8] == ChannelState::DISCRIMINATOR,
        OracleError::InvalidChannelState
    );

    // Extract mint + subject from header to validate PDA
    let mint = Pubkey::new_from_array(
        old_data[10..42]
            .try_into()
            .map_err(|_| OracleError::InvalidChannelState)?,
    );
    let subject = Pubkey::new_from_array(
        old_data[42..74]
            .try_into()
            .map_err(|_| OracleError::InvalidChannelState)?,
    );
    require!(mint == protocol_state.mint, OracleError::InvalidMint);

    let seeds = [CHANNEL_STATE_SEED, mint.as_ref(), subject.as_ref()];
    let (expected_pda, _) = Pubkey::find_program_address(&seeds, ctx.program_id);
    require_keys_eq!(
        expected_pda,
        channel_info.key(),
        OracleError::InvalidChannelState
    );

    // Snapshot header + existing slots (small, bounded) before realloc.
    let mut header = [0u8; HEADER_BYTES];
    header.copy_from_slice(&old_data[..HEADER_BYTES]);

    let slot_size = core::mem::size_of::<ChannelSlot>();
    let slots_start = HEADER_BYTES;
    let slots_bytes = old_data.len().saturating_sub(slots_start);
    require!(
        slots_bytes % slot_size == 0,
        OracleError::InvalidChannelState
    );
    let old_slot_count = slots_bytes / slot_size;
    require!(old_slot_count > 0, OracleError::InvalidChannelState);

    let mut slots: Vec<(u64, [u8; ChannelSlot::LEN])> = Vec::with_capacity(old_slot_count);
    for i in 0..old_slot_count {
        let start = slots_start + i * slot_size;
        let end = start + slot_size;
        if end > old_data.len() {
            break;
        }
        let epoch = u64::from_le_bytes(
            old_data[start..start + 8]
                .try_into()
                .map_err(|_| OracleError::InvalidChannelState)?,
        );
        if epoch == 0 {
            continue;
        }
        let mut slot_bytes = [0u8; ChannelSlot::LEN];
        slot_bytes.copy_from_slice(&old_data[start..end]);
        slots.push((epoch, slot_bytes));
    }

    drop(old_data);

    msg!(
        "Resizing ChannelState {}: {} bytes → {} bytes (preserving {} slots)",
        channel_info.key(),
        current_size,
        target_size,
        slots.len()
    );

    // Fund rent-exempt minimum for new size if needed
    let rent = Rent::get()?;
    let needed_lamports = rent.minimum_balance(target_size);
    let current_lamports = channel_info.lamports();
    if needed_lamports > current_lamports {
        let diff = needed_lamports - current_lamports;
        invoke_signed(
            &system_instruction::transfer(&ctx.accounts.payer.key(), channel_info.key, diff),
            &[
                ctx.accounts.payer.to_account_info(),
                channel_info.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[],
        )?;
    }

    // Realloc and zero-init the new bytes to avoid random epochs bricking the ring.
    channel_info.to_account_info().realloc(target_size, true)?;

    // Rehydrate header, then map each preserved slot to its new ring index.
    let mut new_data = channel_info.try_borrow_mut_data()?;
    new_data[..HEADER_BYTES].copy_from_slice(&header);

    for (epoch, slot_bytes) in slots {
        let dest_idx = ChannelState::slot_index(epoch);
        let dest_start = slots_start + dest_idx * slot_size;
        let dest_end = dest_start + slot_size;
        if dest_end > new_data.len() {
            continue;
        }

        let existing_epoch = u64::from_le_bytes(
            new_data[dest_start..dest_start + 8]
                .try_into()
                .map_err(|_| OracleError::InvalidChannelState)?,
        );
        if existing_epoch == 0 || epoch > existing_epoch {
            new_data[dest_start..dest_end].copy_from_slice(&slot_bytes);
        }
    }

    Ok(())
}
