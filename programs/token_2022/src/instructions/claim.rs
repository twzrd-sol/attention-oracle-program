use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::{
    constants::{CLAIM_SKIM_BPS, EPOCH_STATE_SEED, MAX_ID_BYTES, PROTOCOL_SEED},
    errors::OracleError,
    merkle_proof::{compute_leaf, verify_proof},
    state::{EpochState, ProtocolState},
};

const MAX_PROOF_LEN: usize = 32;

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    /// Global protocol state (PDA authority over treasury ATA)
    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ OracleError::ProtocolPaused,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Epoch state (root + bitmap). Mutable for marking claims.
    #[account(mut)]
    pub epoch_state: Account<'info, EpochState>,

    /// CCM mint (Token‑2022)
    pub mint: InterfaceAccount<'info, Mint>,

    /// Treasury ATA (owned by protocol_state PDA)
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = protocol_state,
        associated_token::token_program = token_program
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,

    /// Claimer ATA (create if needed)
    #[account(
        init_if_needed,
        payer = claimer,
        associated_token::mint = mint,
        associated_token::authority = claimer,
        associated_token::token_program = token_program
    )]
    pub claimer_ata: InterfaceAccount<'info, TokenAccount>,

    /// Optional: TWZRD cNFT receipt (if provided, verifies L1 participation)
    /// CHECK: Optional account, validated in instruction if present
    pub twzrd_receipt: Option<UncheckedAccount<'info>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim<'info>(
    ctx: Context<'_, '_, '_, 'info, Claim<'info>>,
    _subject_index: u8,
    index: u32,
    amount: u64,
    id: String,
    proof: Vec<[u8; 32]>,
) -> Result<()> {
    // Copy key before taking a mutable borrow to satisfy the borrow checker
    let epoch_state_key = ctx.accounts.epoch_state.key();
    let epoch = &mut ctx.accounts.epoch_state;

    // Basic guards
    require!(!epoch.closed, OracleError::EpochClosed);
    require!(index < epoch.claim_count, OracleError::InvalidIndex);
    require!(
        epoch.mint == ctx.accounts.mint.key(),
        OracleError::InvalidMint
    );
    require!(
        id.as_bytes().len() <= MAX_ID_BYTES,
        OracleError::InvalidInputLength
    );
    require!(
        proof.len() <= MAX_PROOF_LEN,
        OracleError::InvalidProofLength
    );

    // Prevent spoofed epoch_state accounts
    let expected_epoch_state = Pubkey::find_program_address(
        &[
            EPOCH_STATE_SEED,
            &epoch.epoch.to_le_bytes(),
            epoch.subject.as_ref(),
        ],
        ctx.program_id,
    )
    .0;
    require_keys_eq!(
        expected_epoch_state,
        epoch_state_key,
        OracleError::InvalidEpochState
    );

    // Check bitmap not already claimed
    let byte_i = (index / 8) as usize;
    let bit = 1u8 << (index % 8);
    require!(
        byte_i < epoch.claimed_bitmap.len(),
        OracleError::InvalidIndex
    );
    require!(
        epoch.claimed_bitmap[byte_i] & bit == 0,
        OracleError::AlreadyClaimed
    );

    // Verify proof against on-chain root
    let leaf = compute_leaf(&ctx.accounts.claimer.key(), index, amount, &id);
    require!(
        verify_proof(&proof, leaf, epoch.root),
        OracleError::InvalidProof
    );

    // Ensure the provided mint matches the protocol instance
    require_keys_eq!(
        ctx.accounts.mint.key(),
        ctx.accounts.protocol_state.mint,
        OracleError::InvalidMint
    );

    // Transfer CCM from treasury PDA to claimer (use transfer_checked for Token-2022)
    // NOTE: ProtocolState is mint-keyed (seeds include mint).
    let seeds: &[&[u8]] = &[
        PROTOCOL_SEED,
        ctx.accounts.protocol_state.mint.as_ref(),
        &[ctx.accounts.protocol_state.bump],
    ];
    let signer = &[seeds];

    let token_program = ctx.accounts.token_program.to_account_info();
    let from = ctx.accounts.treasury_ata.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let to = ctx.accounts.claimer_ata.to_account_info();
    let authority = ctx.accounts.protocol_state.to_account_info();

    let fee = (amount as u128)
        .saturating_mul(CLAIM_SKIM_BPS as u128)
        .checked_div(10_000)
        .unwrap_or(0) as u64;
    let net_amount = amount.saturating_sub(fee);

    crate::transfer_checked_with_remaining(
        &token_program,
        &from,
        &mint,
        &to,
        &authority,
        net_amount,
        ctx.accounts.mint.decimals,
        signer,
        ctx.remaining_accounts,
    )?;

    // Mark claimed and bump totals
    epoch.claimed_bitmap[byte_i] |= bit;
    epoch.total_claimed = epoch.total_claimed.saturating_add(net_amount);

    Ok(())
}

// Open variant: protocol_state keyed by mint
#[derive(Accounts)]
pub struct ClaimOpen<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    #[account(
        mut,
        seeds = [crate::constants::PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ OracleError::ProtocolPaused,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(mut)]
    pub epoch_state: Account<'info, EpochState>,

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

    /// Optional: TWZRD epoch state (for cNFT verification)
    /// CHECK: Optional account, validated in instruction if receipt required
    pub twzrd_epoch_state: Option<UncheckedAccount<'info>>,

    /// Optional: Bubblegum tree (for cNFT verification)
    /// CHECK: Optional account, validated in instruction if receipt required
    pub merkle_tree: Option<UncheckedAccount<'info>>,

    /// Optional: SPL Account Compression program (for cNFT verification)
    /// CHECK: Optional account, validated in instruction if receipt required
    pub compression_program: Option<UncheckedAccount<'info>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_open<'info>(
    ctx: Context<'_, '_, '_, 'info, ClaimOpen<'info>>,
    _subject_index: u8,
    index: u32,
    amount: u64,
    id: String,
    proof: Vec<[u8; 32]>,
    // Legacy parameters (unused, kept for ABI compatibility)
    _channel: Option<String>,
    _twzrd_epoch: Option<u64>,
) -> Result<()> {
    let epoch_state_key = ctx.accounts.epoch_state.key();
    let epoch = &mut ctx.accounts.epoch_state;
    require!(!epoch.closed, OracleError::EpochClosed);
    require!(index < epoch.claim_count, OracleError::InvalidIndex);
    require!(
        epoch.mint == ctx.accounts.mint.key(),
        OracleError::InvalidMint
    );
    require!(
        id.as_bytes().len() <= MAX_ID_BYTES,
        OracleError::InvalidInputLength
    );
    require!(
        proof.len() <= MAX_PROOF_LEN,
        OracleError::InvalidProofLength
    );

    let expected_epoch_state = Pubkey::find_program_address(
        &[
            EPOCH_STATE_SEED,
            &epoch.epoch.to_le_bytes(),
            epoch.subject.as_ref(),
            ctx.accounts.protocol_state.mint.as_ref(),
        ],
        ctx.program_id,
    )
    .0;
    require_keys_eq!(
        expected_epoch_state,
        epoch_state_key,
        OracleError::InvalidEpochState
    );

    // Verify merkle proof for CCM claim
    let byte_i = (index / 8) as usize;
    let bit = 1u8 << (index % 8);
    require!(
        byte_i < epoch.claimed_bitmap.len(),
        OracleError::InvalidIndex
    );
    require!(
        epoch.claimed_bitmap[byte_i] & bit == 0,
        OracleError::AlreadyClaimed
    );

    let leaf = compute_leaf(&ctx.accounts.claimer.key(), index, amount, &id);
    require!(
        verify_proof(&proof, leaf, epoch.root),
        OracleError::InvalidProof
    );

    // Step 3: Transfer CCM tokens
    let seeds: &[&[u8]] = &[
        crate::constants::PROTOCOL_SEED,
        ctx.accounts.protocol_state.mint.as_ref(),
        &[ctx.accounts.protocol_state.bump],
    ];
    let signer = &[seeds];

    let token_program = ctx.accounts.token_program.to_account_info();
    let from = ctx.accounts.treasury_ata.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let to = ctx.accounts.claimer_ata.to_account_info();
    let authority = ctx.accounts.protocol_state.to_account_info();

    let fee = (amount as u128)
        .saturating_mul(CLAIM_SKIM_BPS as u128)
        .checked_div(10_000)
        .unwrap_or(0) as u64;
    let net_amount = amount.saturating_sub(fee);

    crate::transfer_checked_with_remaining(
        &token_program,
        &from,
        &mint,
        &to,
        &authority,
        net_amount,
        ctx.accounts.mint.decimals,
        signer,
        ctx.remaining_accounts,
    )?;

    // Step 4: Mark claimed
    epoch.claimed_bitmap[byte_i] |= bit;
    epoch.total_claimed = epoch.total_claimed.saturating_add(net_amount);
    Ok(())
}
