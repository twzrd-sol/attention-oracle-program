use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::{
    constants::PROTOCOL_SEED,
    errors::ProtocolError,
    state::{FeeConfig, ProtocolState},
};

#[event]
pub struct TransferObserved {
    pub amount: u64,
    pub ts: i64,
}

#[derive(Accounts)]
pub struct TransferHook<'info> {
    /// Signer paying fees if any CPI allocations are needed (unused for now)
    pub payer: Signer<'info>,

    /// Global protocol state (mint-keyed for open variant)
    #[account(
        seeds = [PROTOCOL_SEED, mint.key().as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Fee configuration PDA (mint-keyed)
    #[account(
        seeds = [PROTOCOL_SEED, mint.key().as_ref(), b"fee_config"],
        bump = fee_config.bump
    )]
    pub fee_config: Account<'info, FeeConfig>,

    /// CCM mint (Token‑2022)
    pub mint: InterfaceAccount<'info, Mint>,

    /// Optional source/destination accounts for future withheld-fee harvests
    #[account(mut)]
    pub _source: Option<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub _destination: Option<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

/// Minimal transfer hook: emits an on-chain event so off-chain indexers/overlays
/// can react to CCM transfers. Future versions may harvest Token-2022 withheld
/// fees and route them per FeeSplit.
pub fn transfer_hook(ctx: Context<TransferHook>, amount: u64) -> Result<()> {
    require!(amount > 0, ProtocolError::InvalidAmount);

    // Emit event for indexers / overlays
    let ts = Clock::get()?.unix_timestamp;
    emit!(TransferObserved { amount, ts });

    // Placeholder: no state mutation yet (volume tracking to be added)
    let _ = (&ctx.accounts.protocol_state, &ctx.accounts.fee_config);
    Ok(())
}
