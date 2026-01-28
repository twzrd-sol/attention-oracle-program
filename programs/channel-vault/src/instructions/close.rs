//! Close an empty vault and reclaim rent.
//!
//! Only callable by admin when:
//! - total_shares == 0
//! - pending_deposits == 0
//! - pending_withdrawals == 0

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    close_account, CloseAccount, Mint as MintInterface, TokenAccount, TokenInterface,
};
use anchor_spl::token::{self, CloseAccount as TokenCloseAccount, Token};

use crate::constants::{TOKEN_2022_PROGRAM_ID, VAULT_CCM_BUFFER_SEED, VAULT_ORACLE_POSITION_SEED, VAULT_SEED, VLOFI_MINT_SEED};
use crate::errors::VaultError;
use crate::events::VaultClosed;
use crate::state::{ChannelVault, VaultOraclePosition};

#[derive(Accounts)]
pub struct CloseVault<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [VAULT_SEED, vault.channel_config.as_ref()],
        bump = vault.bump,
        constraint = vault.admin == admin.key() @ VaultError::Unauthorized,
        constraint = vault.total_shares == 0 @ VaultError::VaultNotEmpty,
        constraint = vault.pending_deposits == 0 @ VaultError::VaultNotEmpty,
        constraint = vault.pending_withdrawals == 0 @ VaultError::VaultNotEmpty,
        close = admin,
    )]
    pub vault: Box<Account<'info, ChannelVault>>,

    /// Vault's CCM buffer (Token-2022) - must be empty
    #[account(
        mut,
        seeds = [VAULT_CCM_BUFFER_SEED, vault.key().as_ref()],
        bump,
        constraint = vault_ccm_buffer.amount == 0 @ VaultError::VaultNotEmpty,
    )]
    pub vault_ccm_buffer: Box<InterfaceAccount<'info, TokenAccount>>,

    /// vLOFI mint - must have zero supply
    #[account(
        mut,
        seeds = [VLOFI_MINT_SEED, vault.key().as_ref()],
        bump,
        constraint = vlofi_mint.supply == 0 @ VaultError::VaultNotEmpty,
    )]
    pub vlofi_mint: Box<Account<'info, anchor_spl::token::Mint>>,

    /// Vault Oracle position tracker
    #[account(
        mut,
        seeds = [VAULT_ORACLE_POSITION_SEED, vault.key().as_ref()],
        bump = vault_oracle_position.bump,
        constraint = !vault_oracle_position.is_active @ VaultError::VaultStakeNotActive,
        close = admin,
    )]
    pub vault_oracle_position: Box<Account<'info, VaultOraclePosition>>,

    /// CCM mint for closing buffer account
    #[account(address = vault.ccm_mint)]
    pub ccm_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// Token-2022 program (for closing CCM buffer)
    #[account(
        constraint = token_2022_program.key() == TOKEN_2022_PROGRAM_ID @ VaultError::InvalidTokenProgram,
    )]
    pub token_2022_program: Interface<'info, TokenInterface>,

    /// Standard SPL Token program (for closing vLOFI mint)
    pub token_program: Program<'info, Token>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CloseVault>) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let clock = Clock::get()?;

    let channel_config_key = vault.channel_config;
    let vault_bump = vault.bump;
    let vault_key = vault.key();

    // Vault signer seeds
    let signer_seeds: &[&[&[u8]]] = &[&[
        VAULT_SEED,
        channel_config_key.as_ref(),
        &[vault_bump],
    ]];

    // Close CCM buffer (Token-2022)
    let close_buffer_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_2022_program.to_account_info(),
        CloseAccount {
            account: ctx.accounts.vault_ccm_buffer.to_account_info(),
            destination: ctx.accounts.admin.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        },
        signer_seeds,
    );
    close_account(close_buffer_ctx)?;

    // Close vLOFI mint (standard SPL - requires SetAuthority to None first, then close)
    // Note: Mints can only be closed if supply is 0 and close authority is set
    // For simplicity, we leave the mint open but with 0 supply (it's ~0.002 SOL)
    // A full close would require: set_authority to None, then close_account
    // This is acceptable since mint authority is the vault PDA which is being closed

    emit!(VaultClosed {
        vault: vault_key,
        channel_config: channel_config_key,
        admin: ctx.accounts.admin.key(),
        timestamp: clock.unix_timestamp,
    });

    msg!("Vault closed: {}", vault_key);

    Ok(())
}
