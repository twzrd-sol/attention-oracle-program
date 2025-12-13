use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke_signed, system_instruction};
use anchor_lang::Discriminator;

use crate::constants::{CHANNEL_STATE_SEED, PROTOCOL_SEED};
use crate::errors::OracleError;
use crate::state::{ChannelState, ProtocolState};

/// Header bytes: discriminator (8) + version (1) + bump (1) + mint (32) + subject (32) + padding (6) + latest_epoch (8)
const HEADER_BYTES: usize = 8 + 1 + 1 + 32 + 32 + 6 + 8;

/// Max bytes we can grow per instruction (Solana limit)
const MAX_REALLOC_DELTA: usize = 10240;

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
    if current_size >= target_size {
        msg!("ChannelState already at target size: {} bytes", target_size);
        return Ok(());
    }

    // Calculate this iteration's target (chunked resize due to Solana 10KB limit)
    let delta = target_size.saturating_sub(current_size);
    let this_delta = delta.min(MAX_REALLOC_DELTA);
    let this_target = current_size + this_delta;
    let is_final = this_target == target_size;

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

    // Snapshot header before realloc.
    let mut header = [0u8; HEADER_BYTES];
    header.copy_from_slice(&old_data[..HEADER_BYTES]);

    drop(old_data);

    msg!(
        "Resizing ChannelState {}: {} → {} bytes",
        channel_info.key(),
        current_size,
        this_target
    );

    // Fund rent-exempt minimum for THIS iteration's target size
    let rent = Rent::get()?;
    let needed_lamports = rent.minimum_balance(this_target);
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

    // Realloc to THIS iteration's target (chunked due to Solana 10KB limit)
    channel_info.to_account_info().realloc(this_target, true)?;

    // Rehydrate header (slot data stays in place, new bytes zero-initialized)
    let mut new_data = channel_info.try_borrow_mut_data()?;
    new_data[..HEADER_BYTES].copy_from_slice(&header);

    if is_final {
        msg!("Resize complete - final size: {} bytes", this_target);
    } else {
        msg!(
            "Intermediate resize: {} → {} bytes ({} more iterations needed)",
            current_size,
            this_target,
            (target_size - this_target + MAX_REALLOC_DELTA - 1) / MAX_REALLOC_DELTA
        );
    }

    Ok(())
}
