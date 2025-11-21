use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::{
    constants::{passport_pda, PROTOCOL_SEED},
    errors::OracleError,
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
    /// Owner or delegate initiating the transfer
    /// For AMM swaps, this will be the AMM program's delegate authority
    pub authority: Signer<'info>,

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

    /// Source token account
    #[account(mut)]
    pub source: InterfaceAccount<'info, TokenAccount>,

    /// Destination token account
    #[account(mut)]
    pub destination: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

/// Dynamic transfer hook: calculates fees based on passport tier and emits event
/// with fee breakdown. Allows AMM delegate transfers for Jupiter routing.
/// ENFORCER MODE (Week 2+): Checks sender attention score and applies tax policy.
pub fn transfer_hook(ctx: Context<TransferHook>, amount: u64) -> Result<()> {
    require!(amount > 0, OracleError::InvalidAmount);

    let ts = Clock::get()?.unix_timestamp;
    let fee_config = &ctx.accounts.fee_config;

    // Detect delegate transfer (AMM swap): authority != source.owner
    let is_delegate_transfer = ctx.accounts.authority.key() != ctx.accounts.source.owner;

    // For passport tier lookup, use source token account owner (not delegate)
    let transfer_owner = ctx.accounts.source.owner;

    // -------------------------------------------------------------------------
    // ENFORCER LOGIC: Check sender attention score for VIP/Tourist status
    // -------------------------------------------------------------------------

    // Check if enforcer is active (min_score_threshold > 0)
    let enforcer_active = fee_config.min_score_threshold > 0;

    // Lookup passport for transfer initiator (via remaining_accounts)
    let mut sender_score: u64 = 0; // Zero Trust Policy: default to 0 if no passport
    let mut creator_tier: u8 = 0;
    let mut tier_multiplier: u32 = 0; // Default 0.0x for unverified

    // Search remaining_accounts for matching PassportRegistry
    for account_info in ctx.remaining_accounts.iter() {
        // Only consider accounts owned by this program
        if account_info.owner != ctx.program_id {
            continue;
        }

        if let Ok(data) = account_info.try_borrow_data() {
            if let Ok(registry) = PassportRegistry::try_deserialize(&mut &data[..]) {
                // Ensure this is the correct PDA for the registry entry
                let expected_pda = passport_pda(ctx.program_id, &registry.user_hash);
                if expected_pda != account_info.key() {
                    continue;
                }

                if registry.owner == transfer_owner {
                    sender_score = registry.score;
                    creator_tier = registry.tier;

                    // Tier mapping: index = tier (0 = 0%, 1 = 20%, ..., 5 = 100%)
                    let idx = core::cmp::min(
                        creator_tier as usize,
                        fee_config.tier_multipliers.len() - 1,
                    );
                    tier_multiplier = fee_config.tier_multipliers[idx];
                    break;
                }
            }
        }
    }

    // Apply enforcer policy if active
    let mut enforcer_tax: u64 = 0;
    let mut is_vip: bool = true; // Assume VIP if enforcer dormant

    if enforcer_active {
        // VIP status: score >= threshold
        is_vip = sender_score >= fee_config.min_score_threshold;

        if !is_vip {
            // Tourist detected: score < threshold
            msg!(
                "Enforcer: Tourist detected - score={}, threshold={}",
                sender_score,
                fee_config.min_score_threshold
            );

            // Hard mode: revert transfer
            if fee_config.revert_if_below {
                msg!("Enforcer: Transfer blocked (hard mode)");
                return Err(OracleError::ScoreBelowThreshold.into());
            }

            // Soft mode: calculate tax liability
            enforcer_tax = (amount as u128 * fee_config.tax_bps as u128 / 10000) as u64;
            msg!(
                "Enforcer: Tax applied (soft mode) - tax_bps={}, tax_amount={}",
                fee_config.tax_bps,
                enforcer_tax
            );
        } else {
            msg!(
                "Enforcer: VIP status confirmed - score={}, threshold={}",
                sender_score,
                fee_config.min_score_threshold
            );
        }
    }

    // -------------------------------------------------------------------------
    // EXISTING FEE CALCULATION (for audit/metrics)
    // -------------------------------------------------------------------------

    // Calculate base fees in basis points (from fee_config)
    let treasury_fee_bps = fee_config.treasury_fee_bps;
    let creator_fee_bps = fee_config.creator_fee_bps;

    // Calculate fees in tokens (amount * bps / 10000)
    let treasury_fee = (amount as u128 * treasury_fee_bps as u128 / 10000) as u64;
    let creator_fee_unscaled = (amount as u128 * creator_fee_bps as u128 / 10000) as u64;

    // Scale creator fee by tier multiplier (fixed-point: multiplier / 10000)
    let creator_fee = (creator_fee_unscaled as u128 * tier_multiplier as u128 / 10000) as u64;
    let total_fee = treasury_fee.saturating_add(creator_fee).saturating_add(enforcer_tax);

    // Emit event for indexers/keepers
    // NOTE: Fees are calculated for audit purposes but NOT deducted
    // (mint does not have Transfer Fee extension, only Transfer Hook extension)
    emit!(TransferFeeEvent {
        transfer_amount: amount,
        total_fee,
        treasury_fee,
        creator_fee,
        creator_tier,
        tier_multiplier,
        timestamp: ts,
    });

    // Allow all transfers (including AMM delegate transfers)
    // No CPI transfers or fee deductions in this audit-mode hook
    let _ = &ctx.accounts.protocol_state;
    let _ = is_delegate_transfer; // Acknowledged for future use
    Ok(())
}
