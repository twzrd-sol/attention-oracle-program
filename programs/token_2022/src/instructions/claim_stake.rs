//! Claim + Auto-Stake instruction
//! Extends claim_open with optional CPI to lofi-bank for auto-staking

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked},
};

use crate::{
    constants::{EPOCH_STATE_SEED, PROTOCOL_SEED},
    errors::OracleError,
    instructions::claim::{compute_leaf, verify_proof},
    state::{EpochState, ProtocolState},
};

use lofi_bank::cpi::accounts::Stake as BankStakeAccounts;
use lofi_bank::cpi::stake as bank_stake_cpi;
use lofi_bank::program::LofiBank;

/// Claim with optional auto-stake to lofi-bank
#[derive(Accounts)]
pub struct ClaimAndStake<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ OracleError::ProtocolPaused,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(mut)]
    pub epoch_state: Account<'info, EpochState>,

    /// CCM/TWZRD mint (Token-2022)
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

    // --- LOFI BANK ACCOUNTS (for auto-stake) ---
    /// Lofi Bank program
    pub lofi_bank_program: Program<'info, LofiBank>,

    /// User vault PDA in lofi-bank
    /// CHECK: Validated by lofi-bank CPI (seeds = [b"user_vault", claimer.key()])
    #[account(mut)]
    pub bank_user_vault: UncheckedAccount<'info>,

    /// Treasury state PDA in lofi-bank
    /// CHECK: Validated by lofi-bank CPI (seeds = [b"treasury_state"])
    #[account(mut)]
    pub bank_treasury_state: UncheckedAccount<'info>,

    /// Bank treasury token account (ATA owned by treasury_state PDA)
    #[account(mut)]
    pub bank_treasury_token: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_and_stake(
    ctx: Context<ClaimAndStake>,
    _subject_index: u8,
    index: u32,
    amount: u64,
    id: String,
    proof: Vec<[u8; 32]>,
    auto_stake: bool,
    stake_percent: u8,   // 0-100, default 50
    lock_epochs: u32,    // lock period in epochs, default 12
) -> Result<()> {
    let epoch_state_key = ctx.accounts.epoch_state.key();
    let epoch = &mut ctx.accounts.epoch_state;

    // Basic guards
    require!(!epoch.closed, OracleError::EpochClosed);
    require!(index < epoch.claim_count, OracleError::InvalidIndex);
    require!(
        epoch.mint == ctx.accounts.mint.key(),
        OracleError::InvalidMint
    );

    // Validate epoch_state PDA
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

    // Verify merkle proof
    let leaf = compute_leaf(&ctx.accounts.claimer.key(), index, amount, &id);
    require!(
        verify_proof(&proof, leaf, epoch.root),
        OracleError::InvalidProof
    );

    // Transfer full amount from treasury to claimer
    let seeds: &[&[u8]] = &[
        PROTOCOL_SEED,
        ctx.accounts.protocol_state.mint.as_ref(),
        &[ctx.accounts.protocol_state.bump],
    ];
    let signer = &[seeds];

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.treasury_ata.to_account_info(),
                to: ctx.accounts.claimer_ata.to_account_info(),
                authority: ctx.accounts.protocol_state.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
            signer,
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    // Mark claimed
    epoch.claimed_bitmap[byte_i] |= bit;
    epoch.total_claimed = epoch.total_claimed.saturating_add(amount);

    // Auto-stake CPI if enabled
    if auto_stake {
        let pct = stake_percent.min(100) as u64;
        let stake_amount = amount.saturating_mul(pct) / 100;
        let lock_period = lock_epochs.max(1);

        if stake_amount > 0 {
            let cpi_program = ctx.accounts.lofi_bank_program.to_account_info();

            let cpi_accounts = BankStakeAccounts {
                user_vault: ctx.accounts.bank_user_vault.to_account_info(),
                treasury_state: ctx.accounts.bank_treasury_state.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                user_token: ctx.accounts.claimer_ata.to_account_info(),
                treasury_token: ctx.accounts.bank_treasury_token.to_account_info(),
                payer: ctx.accounts.claimer.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
            };

            // Claimer is signing the top-level tx, so they authorize the stake
            let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
            bank_stake_cpi(cpi_ctx, stake_amount, lock_period)?;

            msg!(
                "Auto-staked {} tokens for {} epochs ({}%)",
                stake_amount,
                lock_period,
                pct
            );
        }
    }

    msg!("Claimed {} tokens, index={}", amount, index);
    Ok(())
}
