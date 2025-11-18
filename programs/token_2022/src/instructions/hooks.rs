use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::{
    constants::{PASSPORT_SEED, PROTOCOL_SEED},
    errors::MiloError,
    events::TransferFeeEvent,
    state::{FeeConfig, PassportRegistry, ProtocolState},
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

/// Dynamic transfer hook: calculates fees based on passport tier and emits event
/// with fee breakdown. Treasury receives fixed 0.05%, creator receives 0.05% * tier multiplier.
/// Fees are withheld by Token-2022's transfer fee extension and harvested separately.
pub fn transfer_hook(ctx: Context<TransferHook>, amount: u64) -> Result<()> {
    require!(amount > 0, MiloError::InvalidAmount);

    let ts = Clock::get()?.unix_timestamp;
    let fee_config = &ctx.accounts.fee_config;

    // Calculate base fees in basis points (from fee_config)
    let treasury_fee_bps = fee_config.treasury_fee_bps;
    let creator_fee_bps = fee_config.creator_fee_bps;

    // Calculate fees in tokens (amount * bps / 10000)
    let treasury_fee = (amount as u128 * treasury_fee_bps as u128 / 10000) as u64;
    let creator_fee_unscaled = (amount as u128 * creator_fee_bps as u128 / 10000) as u64;

    // Lookup passport tier for transfer initiator (via remaining_accounts)
    let mut creator_tier: u8 = 0;
    let mut tier_multiplier: u32 = 0; // Default 0.0x for unverified

    // Determine transfer owner for matching (prefer token account owner)
    let transfer_owner = ctx
        .accounts
        ._source
        .as_ref()
        .map(|s| s.owner)
        .or_else(|| ctx.accounts._destination.as_ref().map(|d| d.owner))
        .unwrap_or_else(|| ctx.accounts.payer.key());

    // Search remaining_accounts for matching PassportRegistry
    for account_info in ctx.remaining_accounts.iter() {
        if let Ok(data) = account_info.try_borrow_data() {
            if let Ok(registry) = PassportRegistry::try_deserialize(&mut &data[..]) {
                if registry.owner == transfer_owner {
                    creator_tier = registry.tier;
                    // Tier mapping: 0 => 0; 1..=6 => table indices 0..=5
                    if creator_tier > 0 {
                        let idx = core::cmp::min(
                            (creator_tier - 1) as usize,
                            fee_config.tier_multipliers.len() - 1,
                        );
                        tier_multiplier = fee_config.tier_multipliers[idx];
                    }
                    break;
                }
            }
        }
    }

    // Scale creator fee by tier multiplier (fixed-point: multiplier / 10000)
    let creator_fee = (creator_fee_unscaled as u128 * tier_multiplier as u128 / 10000) as u64;
    let total_fee = treasury_fee.saturating_add(creator_fee);

    // Emit event for indexers/keepers (fees accumulated in mint, harvested separately)
    emit!(TransferFeeEvent {
        transfer_amount: amount,
        total_fee,
        treasury_fee,
        creator_fee,
        creator_tier,
        tier_multiplier,
        timestamp: ts,
    });

    // Note: No CPI transfers here; fees are withheld by Token-2022 and distributed via harvest
    let _ = &ctx.accounts.protocol_state;
    Ok(())
}
