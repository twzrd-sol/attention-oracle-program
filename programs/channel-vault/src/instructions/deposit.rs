//! Deposit CCM and receive vLOFI shares.

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, MintTo, Token},
    token_interface::{Mint as MintInterface, TokenAccount, TokenInterface, TransferChecked},
};

use crate::constants::{MIN_DEPOSIT, MIN_INITIAL_DEPOSIT, TOKEN_2022_PROGRAM_ID, VAULT_SEED};
use crate::errors::VaultError;
use crate::events::Deposited;
use crate::state::ChannelVault;

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [VAULT_SEED, vault.channel_config.as_ref()],
        bump = vault.bump,
        constraint = !vault.paused @ VaultError::VaultPaused,
    )]
    pub vault: Box<Account<'info, ChannelVault>>,

    /// CCM mint (Token-2022)
    #[account(address = vault.ccm_mint)]
    pub ccm_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// vLOFI mint
    #[account(
        mut,
        address = vault.vlofi_mint,
    )]
    pub vlofi_mint: Box<Account<'info, anchor_spl::token::Mint>>,

    /// User's CCM token account
    #[account(
        mut,
        constraint = user_ccm.owner == user.key() @ VaultError::Unauthorized,
        constraint = user_ccm.mint == ccm_mint.key() @ VaultError::InvalidMint,
    )]
    pub user_ccm: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Vault's CCM buffer
    #[account(
        mut,
        address = vault.ccm_buffer,
    )]
    pub vault_ccm_buffer: Box<InterfaceAccount<'info, TokenAccount>>,

    /// User's vLOFI token account (created if needed)
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = vlofi_mint,
        associated_token::authority = user,
    )]
    pub user_vlofi: Box<Account<'info, anchor_spl::token::TokenAccount>>,

    /// Token-2022 program (for CCM transfer)
    #[account(
        constraint = token_2022_program.key() == TOKEN_2022_PROGRAM_ID @ VaultError::InvalidTokenProgram,
    )]
    pub token_2022_program: Interface<'info, TokenInterface>,

    /// Standard SPL Token program (for vLOFI mint)
    pub token_program: Program<'info, Token>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Deposit>, amount: u64, min_shares: u64) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let clock = Clock::get()?;

    // Validate deposit amount (pre-fee)
    let min_required = if vault.total_shares == 0 {
        MIN_INITIAL_DEPOSIT
    } else {
        vault.min_deposit.max(MIN_DEPOSIT)
    };
    require!(amount >= min_required, VaultError::DepositTooSmall);

    // Capture buffer balance BEFORE transfer (for transfer-fee accounting)
    let balance_before = ctx.accounts.vault_ccm_buffer.amount;

    // Transfer CCM from user to vault buffer (Token-2022)
    // Note: CCM has 0.5% transfer fee - vault receives less than `amount`
    let transfer_ctx = CpiContext::new(
        ctx.accounts.token_2022_program.to_account_info(),
        TransferChecked {
            from: ctx.accounts.user_ccm.to_account_info(),
            mint: ctx.accounts.ccm_mint.to_account_info(),
            to: ctx.accounts.vault_ccm_buffer.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        },
    );
    anchor_spl::token_interface::transfer_checked(
        transfer_ctx,
        amount,
        ctx.accounts.ccm_mint.decimals,
    )?;

    // Reload buffer to get actual received amount (post transfer-fee)
    ctx.accounts.vault_ccm_buffer.reload()?;
    let balance_after = ctx.accounts.vault_ccm_buffer.amount;
    let actual_received = balance_after
        .checked_sub(balance_before)
        .ok_or(VaultError::MathOverflow)?;

    // Calculate shares based on ACTUAL received (not requested amount)
    let shares = vault.calculate_shares(actual_received)?;
    require!(shares > 0, VaultError::DepositTooSmall);
    require!(shares >= min_shares, VaultError::SlippageExceeded);

    // Mint vLOFI shares to user (standard SPL)
    let channel_config_key = vault.channel_config;
    let bump = vault.bump;
    let signer_seeds: &[&[&[u8]]] = &[&[
        VAULT_SEED,
        channel_config_key.as_ref(),
        &[bump],
    ]];

    let mint_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        MintTo {
            mint: ctx.accounts.vlofi_mint.to_account_info(),
            to: ctx.accounts.user_vlofi.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        },
        signer_seeds,
    );
    token::mint_to(mint_ctx, shares)?;

    // Update vault state with ACTUAL received (not requested amount)
    let vault = &mut ctx.accounts.vault;
    vault.pending_deposits = vault
        .pending_deposits
        .checked_add(actual_received)
        .ok_or(VaultError::MathOverflow)?;
    vault.total_shares = vault
        .total_shares
        .checked_add(shares)
        .ok_or(VaultError::MathOverflow)?;

    // Get exchange rate for event
    let exchange_rate = vault.exchange_rate()?;

    emit!(Deposited {
        user: ctx.accounts.user.key(),
        vault: vault.key(),
        ccm_amount: actual_received, // Post-fee amount
        shares_minted: shares,
        exchange_rate,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Deposited {} CCM (requested {}), minted {} vLOFI, rate={}",
        actual_received,
        amount,
        shares,
        exchange_rate
    );

    Ok(())
}
