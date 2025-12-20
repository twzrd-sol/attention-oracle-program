use anchor_lang::solana_program::{program::invoke_signed, system_instruction};
use anchor_lang::{accounts::account::Account, prelude::*};
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::constants::{
    CHANNEL_BITMAP_BYTES, CHANNEL_STATE_SEED, CLAIM_SKIM_BPS, MAX_ID_BYTES, PROTOCOL_SEED,
};
use crate::errors::OracleError;
use crate::merkle_proof::{compute_leaf, verify_proof};
use crate::state::{ChannelSlot, ChannelState, ProtocolState};
use anchor_lang::accounts::account_loader::AccountLoader;
use anchor_lang::Discriminator;

const CHANNEL_STATE_VERSION: u8 = 1;
const MAX_PROOF_LEN: usize = 32;

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
// INITIALIZE CHANNEL
// ============================================================================

/// Initialize a new ChannelState account.
/// Seeds: ["channel_state", mint, subject_id]
#[derive(Accounts)]
#[instruction(subject_id: Pubkey)]
pub struct InitializeChannel<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ OracleError::ProtocolPaused,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// The channel state to create - uses `init` for top-level allocation
    #[account(
        init_if_needed,
        payer = payer,
        space = ChannelState::LEN,
        seeds = [CHANNEL_STATE_SEED, protocol_state.mint.as_ref(), subject_id.as_ref()],
        bump,
    )]
    pub channel_state: AccountLoader<'info, ChannelState>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_channel(
    ctx: Context<InitializeChannel>,
    subject_id: Pubkey,
) -> Result<()> {
    require!(subject_id != Pubkey::default(), OracleError::InvalidPubkey);

    let protocol_state = &ctx.accounts.protocol_state;

    // `init_if_needed` + `AccountLoader` requires `load_init()` for fresh accounts, otherwise
    // `load_mut()` will fail with AccountDiscriminatorMismatch (discriminator is still zero).
    let channel_state_info = ctx.accounts.channel_state.to_account_info();
    let disc = {
        let data = channel_state_info.try_borrow_data()?;
        require!(data.len() >= 8, OracleError::InvalidChannelState);
        let mut out = [0u8; 8];
        out.copy_from_slice(&data[..8]);
        out
    };

    let mut channel_state = if disc == [0u8; 8] {
        ctx.accounts.channel_state.load_init()?
    } else {
        require!(disc == ChannelState::DISCRIMINATOR, OracleError::InvalidChannelState);
        ctx.accounts.channel_state.load_mut()?
    };

    if channel_state.version == 0 {
        channel_state.version = CHANNEL_STATE_VERSION;
        channel_state.bump = ctx.bumps.channel_state;
        channel_state.mint = protocol_state.mint;
        channel_state.subject = subject_id;
        channel_state.latest_epoch = 0;
        // Slots are zeroed by account creation.
    }

    require!(
        channel_state.mint == protocol_state.mint,
        OracleError::InvalidMint
    );
    require!(
        channel_state.subject == subject_id,
        OracleError::InvalidChannelState
    );

    Ok(())
}

// ============================================================================
// SET CHANNEL MERKLE ROOT
// ============================================================================

#[derive(Accounts)]
pub struct SetChannelMerkleRoot<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ OracleError::ProtocolPaused,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// PDA derived from (mint, channel) - zero_copy account
    /// CHECK: PDA is validated via derivation in handler
    #[account(mut)]
    pub channel_state: AccountLoader<'info, ChannelState>,

    pub system_program: Program<'info, System>,
}

pub fn set_channel_merkle_root(
    ctx: Context<SetChannelMerkleRoot>,
    channel: String,
    epoch: u64,
    root: [u8; 32],
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;

    // Authorization: admin or allowlisted publisher
    let signer = ctx.accounts.payer.key();
    let is_admin = signer == protocol_state.admin;
    let is_publisher =
        protocol_state.publisher != Pubkey::default() && signer == protocol_state.publisher;
    require!(is_admin || is_publisher, OracleError::Unauthorized);

    let subject_id = derive_subject_id(&channel);
    let seeds = [
        CHANNEL_STATE_SEED,
        protocol_state.mint.as_ref(),
        subject_id.as_ref(),
    ];
    let (expected_pda, _bump) = Pubkey::find_program_address(&seeds, ctx.program_id);
    require_keys_eq!(
        expected_pda,
        ctx.accounts.channel_state.to_account_info().key(),
        OracleError::InvalidChannelState
    );

    // Channel must be explicitly initialized via InitializeChannel first
    let mut channel_state = ctx.accounts.channel_state.load_mut()?;

    require!(
        channel_state.version == CHANNEL_STATE_VERSION,
        OracleError::InvalidChannelState
    );
    require!(
        channel_state.mint == protocol_state.mint,
        OracleError::InvalidMint
    );
    require!(
        channel_state.subject == subject_id,
        OracleError::InvalidChannelState
    );

    // Calculate slot index (ring buffer)
    let slot_idx = ChannelState::slot_index(epoch);
    msg!("writing slot {}", slot_idx);

    // Get the slot and check monotonic invariant
    let slot = channel_state.slot_mut(epoch);
    let existing_epoch = slot.epoch;

    // Monotonic guard: new epoch must be greater than existing
    require!(
        existing_epoch == 0 || epoch > existing_epoch,
        OracleError::EpochNotIncreasing
    );

    // Write the new merkle root
    slot.epoch = epoch;
    slot.root = root;
    slot.claim_count = 0;
    slot.claimed_bitmap.fill(0);

    // Update latest_epoch if needed
    channel_state.latest_epoch = channel_state.latest_epoch.max(epoch);

    Ok(())
}

#[derive(Accounts)]
pub struct ClaimChannel<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ OracleError::ProtocolPaused,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// CHECK: PDA is validated via derivation in handler
    #[account(mut)]
    pub channel_state: AccountLoader<'info, ChannelState>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = protocol_state,
        associated_token::token_program = token_program
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = claimer,
        associated_token::mint = mint,
        associated_token::authority = claimer,
        associated_token::token_program = token_program
    )]
    pub claimer_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_channel_open<'info>(
    ctx: Context<'_, '_, '_, 'info, ClaimChannel<'info>>,
    channel: String,
    epoch: u64,
    index: u32,
    amount: u64,
    id: String,
    proof: Vec<[u8; 32]>,
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;
    // Ensure provided mint matches the protocol instance
    require_keys_eq!(
        ctx.accounts.mint.key(),
        protocol_state.mint,
        OracleError::InvalidMint
    );
    require!(
        id.len() <= MAX_ID_BYTES,
        OracleError::InvalidInputLength
    );
    require!(
        proof.len() <= MAX_PROOF_LEN,
        OracleError::InvalidProofLength
    );
    let subject_id = derive_subject_id(&channel);
    let seeds = [
        CHANNEL_STATE_SEED,
        protocol_state.mint.as_ref(),
        subject_id.as_ref(),
    ];
    let (expected_pda, _) = Pubkey::find_program_address(&seeds, ctx.program_id);
    require_keys_eq!(
        expected_pda,
        ctx.accounts.channel_state.to_account_info().key(),
        OracleError::InvalidChannelState
    );

    // Load via Anchor's zero_copy loader
    let mut channel_state = ctx.accounts.channel_state.load_mut()?;

    require!(
        channel_state.version == CHANNEL_STATE_VERSION,
        OracleError::InvalidChannelState
    );
    require!(
        channel_state.mint == protocol_state.mint,
        OracleError::InvalidMint
    );
    require!(
        channel_state.subject == subject_id,
        OracleError::InvalidChannelState
    );

    ChannelSlot::validate_index(index as usize)?;
    let slot = channel_state.slot_mut(epoch);
    require!(slot.epoch == epoch, OracleError::SlotMismatch);

    let byte_i = (index / 8) as usize;
    let bit_mask = 1u8 << (index % 8);
    require!(byte_i < CHANNEL_BITMAP_BYTES, OracleError::InvalidIndex);
    require!(
        slot.claimed_bitmap[byte_i] & bit_mask == 0,
        OracleError::AlreadyClaimed
    );

    let leaf = compute_leaf(&ctx.accounts.claimer.key(), index, amount, &id);
    require!(
        verify_proof(&proof, leaf, slot.root),
        OracleError::InvalidProof
    );

    // Mark as claimed
    slot.claimed_bitmap[byte_i] |= bit_mask;
    slot.claim_count = slot.claim_count.saturating_add(1);

    // Claim-time skim: keep a fixed % in the protocol treasury (source ATA) by
    // transferring less to the user.
    let fee = (amount as u128)
        .saturating_mul(CLAIM_SKIM_BPS as u128)
        .checked_div(10_000)
        .unwrap_or(0) as u64;
    let tokens = amount.saturating_sub(fee);

    let seeds: &[&[u8]] = &[
        PROTOCOL_SEED,
        protocol_state.mint.as_ref(),
        &[protocol_state.bump],
    ];
    let signer = &[seeds];

    let token_program = ctx.accounts.token_program.to_account_info();
    let from = ctx.accounts.treasury_ata.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let to = ctx.accounts.claimer_ata.to_account_info();
    let authority = ctx.accounts.protocol_state.to_account_info();

    crate::transfer_checked_with_remaining(
        &token_program,
        &from,
        &mint,
        &to,
        &authority,
        tokens,
        ctx.accounts.mint.decimals,
        signer,
        ctx.remaining_accounts,
    )?;

    Ok(())
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
    use anchor_lang::Discriminator;
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
