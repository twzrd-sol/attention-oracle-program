//! Transfer vLOFI mint authority to a new owner.
//!
//! One-time migration instruction: transfers the vLOFI mint authority
//! from this vault PDA to a new authority (e.g., the AO protocol_state PDA).
//! This allows the new AO program to mint/burn vLOFI directly.
//!
//! Admin-only. Irreversible — once transferred, this vault can no longer
//! mint or burn vLOFI.

use anchor_lang::prelude::*;
use anchor_spl::token::spl_token::instruction::AuthorityType;
use anchor_spl::token::{self, SetAuthority, Token};

use crate::constants::VAULT_SEED;
use crate::errors::VaultError;
use crate::state::ChannelVault;

#[derive(Accounts)]
pub struct TransferMintAuthority<'info> {
    /// Vault admin (Squads vault PDA on mainnet).
    #[account(
        constraint = admin.key() == vault.admin @ VaultError::Unauthorized
    )]
    pub admin: Signer<'info>,

    /// Channel vault PDA (current vLOFI mint authority).
    #[account(
        seeds = [VAULT_SEED, vault.channel_config.as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, ChannelVault>,

    /// vLOFI mint account (writable — authority will be changed).
    /// CHECK: We verify it matches the vault's stored vlofi_mint.
    #[account(
        mut,
        constraint = vlofi_mint.key() == vault.vlofi_mint @ VaultError::InvalidMint,
    )]
    pub vlofi_mint: AccountInfo<'info>,

    /// Standard SPL Token program (vLOFI is standard SPL).
    pub token_program: Program<'info, Token>,
}

/// Transfer the vLOFI mint authority from this vault PDA to `new_authority`.
///
/// This is a one-way migration. After calling this, the channel-vault
/// program can no longer mint or burn vLOFI for this vault.
pub fn handler(ctx: Context<TransferMintAuthority>, new_authority: Pubkey) -> Result<()> {
    require!(
        new_authority != Pubkey::default(),
        VaultError::InvalidPubkey
    );

    let vault = &ctx.accounts.vault;
    let channel_config = vault.channel_config;
    let vault_bump = vault.bump;

    // Vault PDA signer seeds: ["vault", channel_config, [bump]]
    let vault_seeds = &[VAULT_SEED, channel_config.as_ref(), &[vault_bump]];
    let signer = &[&vault_seeds[..]];

    // CPI: spl_token::SetAuthority(MintTokens) on the vLOFI mint
    let cpi_accounts = SetAuthority {
        current_authority: ctx.accounts.vault.to_account_info(),
        account_or_mint: ctx.accounts.vlofi_mint.to_account_info(),
    };
    token::set_authority(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer,
        ),
        AuthorityType::MintTokens,
        Some(new_authority),
    )?;

    msg!(
        "vLOFI mint authority transferred: {} -> {}",
        vault.key(),
        new_authority
    );

    Ok(())
}
