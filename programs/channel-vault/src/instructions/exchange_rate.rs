//! Exchange Rate Oracle — public, verifiable on-chain price feed for vLOFI.
//!
//! This PDA stores the current CCM-per-vLOFI exchange rate, updated on every
//! compound. External protocols (Kamino, marginfi, Switchboard) can read this
//! account via `getAccountInfo` without any CPI — just account deserialization.
//!
//! Seeds: ["exchange_rate", vault_key]

use anchor_lang::prelude::*;

use crate::constants::{EXCHANGE_RATE_SEED, VAULT_SEED};
use crate::errors::VaultError;
use crate::state::{ChannelVault, ExchangeRateOracle};

// =============================================================================
// INITIALIZE EXCHANGE RATE ORACLE (one-time, admin-only)
// =============================================================================

#[derive(Accounts)]
pub struct InitializeExchangeRate<'info> {
    /// Admin authority (must match vault.admin). Pays rent.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The parent vault — oracle is derived from this.
    #[account(
        seeds = [VAULT_SEED, vault.channel_config.as_ref()],
        bump = vault.bump,
        constraint = vault.admin == payer.key() @ VaultError::Unauthorized,
    )]
    pub vault: Account<'info, ChannelVault>,

    /// The exchange rate oracle PDA — initialized here.
    #[account(
        init,
        payer = payer,
        space = ExchangeRateOracle::LEN,
        seeds = [EXCHANGE_RATE_SEED, vault.key().as_ref()],
        bump,
    )]
    pub exchange_rate_oracle: Account<'info, ExchangeRateOracle>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_exchange_rate(ctx: Context<InitializeExchangeRate>) -> Result<()> {
    let oracle = &mut ctx.accounts.exchange_rate_oracle;
    let vault = &ctx.accounts.vault;
    let clock = Clock::get()?;

    oracle.bump = ctx.bumps.exchange_rate_oracle;
    oracle.vault = vault.key();
    oracle.version = ExchangeRateOracle::VERSION;
    oracle._reserved = [0u8; 80];

    // Populate with current vault state
    oracle.update_from_vault(vault, clock.slot, clock.unix_timestamp)?;

    msg!(
        "Exchange rate oracle initialized: rate={}, assets={}, shares={}",
        oracle.current_rate,
        oracle.total_ccm_assets,
        oracle.total_vlofi_shares,
    );

    Ok(())
}
