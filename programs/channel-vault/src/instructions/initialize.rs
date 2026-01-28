//! Initialize a new ChannelVault for a specific channel.

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::Token,
    token_interface::{Mint as MintInterface, TokenAccount, TokenInterface},
};

use crate::constants::{TOKEN_2022_PROGRAM_ID, VAULT_CCM_BUFFER_SEED, VAULT_ORACLE_POSITION_SEED, VAULT_SEED, VLOFI_MINT_SEED};
use crate::errors::VaultError;
use crate::events::VaultInitialized;
use crate::state::{ChannelVault, VaultOraclePosition};

// Import Oracle types for validation
use token_2022::{ChannelConfigV2, ProtocolState, CHANNEL_CONFIG_V2_SEED, PROTOCOL_SEED};

#[derive(Accounts)]
pub struct InitializeVault<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// Oracle's protocol state (for mint validation)
    #[account(
        seeds = [PROTOCOL_SEED, oracle_protocol.mint.as_ref()],
        bump = oracle_protocol.bump,
        seeds::program = token_2022::ID,
    )]
    pub oracle_protocol: Box<Account<'info, ProtocolState>>,

    /// Oracle's channel config (validates channel exists)
    #[account(
        constraint = oracle_channel_config.mint == oracle_protocol.mint @ VaultError::InvalidMint,
    )]
    pub oracle_channel_config: Box<Account<'info, ChannelConfigV2>>,

    /// CCM mint (Token-2022)
    #[account(
        constraint = ccm_mint.key() == oracle_protocol.mint @ VaultError::InvalidMint,
    )]
    pub ccm_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// New vault PDA
    #[account(
        init,
        payer = admin,
        space = ChannelVault::LEN,
        seeds = [VAULT_SEED, oracle_channel_config.key().as_ref()],
        bump
    )]
    pub vault: Box<Account<'info, ChannelVault>>,

    /// Vault's CCM buffer token account (Token-2022)
    #[account(
        init,
        payer = admin,
        seeds = [VAULT_CCM_BUFFER_SEED, vault.key().as_ref()],
        bump,
        token::mint = ccm_mint,
        token::authority = vault,
        token::token_program = token_2022_program,
    )]
    pub ccm_buffer: Box<InterfaceAccount<'info, TokenAccount>>,

    /// vLOFI mint (standard SPL Token, authority = vault)
    #[account(
        init,
        payer = admin,
        seeds = [VLOFI_MINT_SEED, vault.key().as_ref()],
        bump,
        mint::decimals = 9,
        mint::authority = vault,
    )]
    pub vlofi_mint: Box<Account<'info, anchor_spl::token::Mint>>,

    /// Vault Oracle position tracker
    #[account(
        init,
        payer = admin,
        space = VaultOraclePosition::LEN,
        seeds = [VAULT_ORACLE_POSITION_SEED, vault.key().as_ref()],
        bump
    )]
    pub vault_oracle_position: Box<Account<'info, VaultOraclePosition>>,

    /// Token-2022 program (for CCM)
    #[account(
        constraint = token_2022_program.key() == TOKEN_2022_PROGRAM_ID @ VaultError::InvalidTokenProgram,
    )]
    pub token_2022_program: Interface<'info, TokenInterface>,

    /// Standard SPL Token program (for vLOFI)
    pub token_program: Program<'info, Token>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(
    ctx: Context<InitializeVault>,
    min_deposit: u64,
    lock_duration_slots: u64,
    withdraw_queue_slots: u64,
) -> Result<()> {
    let clock = Clock::get()?;

    // Initialize vault
    let vault = &mut ctx.accounts.vault;
    vault.bump = ctx.bumps.vault;
    vault.version = 1;
    vault.channel_config = ctx.accounts.oracle_channel_config.key();
    vault.ccm_mint = ctx.accounts.ccm_mint.key();
    vault.vlofi_mint = ctx.accounts.vlofi_mint.key();
    vault.ccm_buffer = ctx.accounts.ccm_buffer.key();
    vault.total_staked = 0;
    vault.total_shares = 0;
    vault.pending_deposits = 0;
    vault.pending_withdrawals = 0;
    vault.last_compound_slot = clock.slot;
    vault.compound_count = 0;
    vault.admin = ctx.accounts.admin.key();
    vault.min_deposit = min_deposit;
    vault.paused = false;
    vault.emergency_reserve = 0;
    vault.lock_duration_slots = lock_duration_slots;
    vault.withdraw_queue_slots = withdraw_queue_slots;
    vault._reserved = [0u8; 40];

    // Initialize vault oracle position tracker
    let position = &mut ctx.accounts.vault_oracle_position;
    position.bump = ctx.bumps.vault_oracle_position;
    position.vault = vault.key();
    position.oracle_user_stake = Pubkey::default();
    position.oracle_nft_mint = Pubkey::default();
    position.oracle_nft_ata = Pubkey::default();
    position.is_active = false;
    position.stake_amount = 0;
    position.lock_end_slot = 0;

    emit!(VaultInitialized {
        vault: vault.key(),
        channel_config: vault.channel_config,
        ccm_mint: vault.ccm_mint,
        vlofi_mint: vault.vlofi_mint,
        admin: vault.admin,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Initialized vault for channel: {}, vlofi_mint: {}",
        ctx.accounts.oracle_channel_config.key(),
        ctx.accounts.vlofi_mint.key()
    );

    Ok(())
}
