use anchor_lang::solana_program::{program::invoke_signed, system_instruction};
use anchor_lang::{accounts::account::Account, prelude::*};

use crate::constants::{
    CHANNEL_STATE_SEED, PROTOCOL_SEED,
};
use crate::errors::OracleError;
use crate::state::{ChannelState, ProtocolState};
use anchor_lang::accounts::account_loader::AccountLoader;

/// Derive a stable subject_id from channel name (lowercase, prefixed)
pub fn derive_subject_id(channel: &str) -> Pubkey {
    let mut lower = channel.as_bytes().to_vec();
    // Convert ASCII bytes to lowercase in-place (avoids allocation for Unicode)
    lower.iter_mut().for_each(|b| *b = b.to_ascii_lowercase());
    // Brand- and platform-neutral prefix to derive a stable subject key
    let hash = keccak_hashv(&[b"channel:", lower.as_slice()]);
    Pubkey::new_from_array(hash)
}

// Minimal Keccak-256 helpers (Solana 2.x split crates; use sha3 directly)
use sha3::{Digest, Keccak256};
fn keccak_hashv(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    for p in parts {
        hasher.update(p);
    }
    let out = hasher.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out[..32]);
    arr
}

// ============================================================================
// CLOSE CHANNEL (Admin Maintenance)
// ============================================================================

/// Close a channel state account and reclaim rent to the admin.
/// Critical for cleaning up disabled streams (e.g., Twitch migrations).
#[derive(Accounts)]
#[instruction(channel: String)]
pub struct CloseChannel<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// Protocol state to verify admin authority
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// The channel state to close - rent refunded to admin
    /// CHECK: PDA validated via seeds; closed manually to handle zero_copy
    #[account(mut)]
    pub channel_state: AccountLoader<'info, ChannelState>,

    pub system_program: Program<'info, System>,
}

pub fn close_channel(ctx: Context<CloseChannel>, channel: String) -> Result<()> {
    // Derive and validate subject_id matches the provided channel
    let subject_id = derive_subject_id(&channel);
    let protocol_state = &ctx.accounts.protocol_state;

    // Verify PDA derivation
    let seeds = [
        CHANNEL_STATE_SEED,
        protocol_state.mint.as_ref(),
        subject_id.as_ref(),
    ];
    let (expected_pda, bump) = Pubkey::find_program_address(&seeds, ctx.program_id);
    require_keys_eq!(
        expected_pda,
        ctx.accounts.channel_state.to_account_info().key(),
        OracleError::InvalidChannelState
    );

    // Load channel state to verify it belongs to this protocol instance
    let channel_state = ctx.accounts.channel_state.load()?;
    require!(
        channel_state.mint == protocol_state.mint,
        OracleError::InvalidMint
    );
    require!(
        channel_state.subject == subject_id,
        OracleError::InvalidChannelState
    );

    // Drop the borrow before closing
    drop(channel_state);

    // Close account: transfer lamports to admin, zero data, reassign to system program
    let channel_state_info = ctx.accounts.channel_state.to_account_info();
    let admin_info = ctx.accounts.admin.to_account_info();

    // Transfer all lamports to admin (scoped so borrow drops)
    let lamports = {
        let mut lamports_ref = channel_state_info.try_borrow_mut_lamports()?;
        let lamports = **lamports_ref;
        **lamports_ref = 0;
        lamports
    };
    **admin_info.try_borrow_mut_lamports()? = admin_info
        .lamports()
        .checked_add(lamports)
        .ok_or(OracleError::InvalidAmount)?;

    // Zero out data (scoped to drop borrow before CPI)
    {
        let mut data = channel_state_info.try_borrow_mut_data()?;
        data.fill(0);
    }

    // Reassign to system program to close it
    invoke_signed(
        &system_instruction::assign(channel_state_info.key, &anchor_lang::system_program::ID),
        &[
            channel_state_info.clone(),
            ctx.accounts.system_program.to_account_info(),
        ],
        &[&[
            CHANNEL_STATE_SEED,
            protocol_state.mint.as_ref(),
            subject_id.as_ref(),
            &[bump],
        ]],
    )?;

    let sol_whole = lamports / 1_000_000_000;
    let sol_frac = lamports % 1_000_000_000;
    msg!(
        "Channel '{}' closed by admin: {}. Reclaimed {} lamports (~{}.{:09} SOL)",
        channel,
        ctx.accounts.admin.key(),
        lamports,
        sol_whole,
        sol_frac
    );

    Ok(())
}

// ============================================================================
// CLOSE LEGACY CHANNEL (Admin Maintenance - for old-size accounts)
// ============================================================================

/// Close a legacy channel state account (with size mismatch) and reclaim rent.
/// Uses UncheckedAccount to avoid Anchor's size validation.
#[derive(Accounts)]
#[instruction(channel: String)]
pub struct CloseLegacyChannel<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// Protocol state to verify admin authority
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// The legacy channel state to close
    /// CHECK: PDA validated manually; owner verified; size may differ from current ChannelState
    #[account(mut)]
    pub channel_state: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn close_legacy_channel(ctx: Context<CloseLegacyChannel>, channel: String) -> Result<()> {
    let subject_id = derive_subject_id(&channel);
    let protocol_state = &ctx.accounts.protocol_state;
    let channel_info = &ctx.accounts.channel_state;

    // Verify PDA derivation
    let seeds = [
        CHANNEL_STATE_SEED,
        protocol_state.mint.as_ref(),
        subject_id.as_ref(),
    ];
    let (expected_pda, _bump) = Pubkey::find_program_address(&seeds, ctx.program_id);
    require_keys_eq!(
        expected_pda,
        channel_info.key(),
        OracleError::InvalidChannelState
    );

    // Verify ownership
    require!(
        channel_info.owner == ctx.program_id,
        OracleError::InvalidChannelState
    );

    // Verify account has enough data to be a ChannelState header (88 bytes min)
    let data_len = channel_info.data_len();
    require!(data_len >= 88, OracleError::InvalidChannelState);

    // Verify discriminator and mint from header (manual read to avoid size check)
    let data = channel_info.try_borrow_data()?;
        require!(
        &data[0..8] == ChannelState::DISCRIMINATOR,
        OracleError::InvalidChannelState
    );
    let mint_bytes: [u8; 32] = data[10..42]
        .try_into()
        .map_err(|_| OracleError::InvalidChannelState)?;
    let account_mint = Pubkey::new_from_array(mint_bytes);
    require!(
        account_mint == protocol_state.mint,
        OracleError::InvalidMint
    );
    drop(data);

    // Close account: transfer lamports to admin
    let admin_info = ctx.accounts.admin.to_account_info();
    let lamports = {
        let mut lamports_ref = channel_info.try_borrow_mut_lamports()?;
        let lamports = **lamports_ref;
        **lamports_ref = 0;
        lamports
    };
    **admin_info.try_borrow_mut_lamports()? = admin_info
        .lamports()
        .checked_add(lamports)
        .ok_or(OracleError::InvalidAmount)?;

    // Zero out data and reassign to system program
    let channel_account_info = channel_info.to_account_info();
    channel_account_info.assign(&anchor_lang::system_program::ID);
    channel_account_info.resize(0)?;

    let sol_whole = lamports / 1_000_000_000;
    let sol_frac = lamports % 1_000_000_000;
    msg!(
        "Legacy channel '{}' closed (size: {}). Reclaimed {} lamports (~{}.{:09} SOL)",
        channel,
        data_len,
        lamports,
        sol_whole,
        sol_frac
    );

    Ok(())
}
