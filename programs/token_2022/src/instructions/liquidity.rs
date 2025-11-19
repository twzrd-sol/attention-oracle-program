use crate::{
    constants::{LIQUIDITY_ENGINE_SEED, PROTOCOL_SEED},
    errors::OracleError,
    state::{LiquidityEngine, ProtocolState},
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct TriggerLiquidityDrip<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Global protocol state (future: gated by governance)
    #[account(
        seeds = [PROTOCOL_SEED],
        bump = protocol_state.bump,
        constraint = authority.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Liquidity engine PDA
    #[account(
        init_if_needed,
        payer = authority,
        space = LiquidityEngine::LEN,
        seeds = [LIQUIDITY_ENGINE_SEED],
        bump,
    )]
    pub liquidity_engine: Account<'info, LiquidityEngine>,

    pub system_program: Program<'info, System>,
}

pub fn trigger_drip(ctx: Context<TriggerLiquidityDrip>, tier: u8) -> Result<()> {
    // Minimal skeleton: mark tier complete and set timestamp.
    let engine = &mut ctx.accounts.liquidity_engine;
    let now = Clock::get()?.unix_timestamp;

    // Initialize if fresh
    if engine.state.last_drip == 0 {
        engine.state.current_tier = 0;
        engine.state.total_claimed = 0;
        engine.state.total_dripped = 0;
        engine.state.pool_address = Pubkey::default();
        engine.state.tier_1_complete = false;
        engine.state.tier_2_complete = false;
        engine.state.tier_3_complete = false;
    }

    match tier {
        1 => {
            require!(
                !engine.state.tier_1_complete,
                OracleError::DripAlreadyExecuted
            );
            engine.state.tier_1_complete = true;
            engine.state.current_tier = engine.state.current_tier.max(1);
        }
        2 => {
            require!(engine.state.tier_1_complete, OracleError::InvalidDripTier);
            require!(
                !engine.state.tier_2_complete,
                OracleError::DripAlreadyExecuted
            );
            engine.state.tier_2_complete = true;
            engine.state.current_tier = engine.state.current_tier.max(2);
        }
        3 => {
            require!(engine.state.tier_2_complete, OracleError::InvalidDripTier);
            require!(
                !engine.state.tier_3_complete,
                OracleError::DripAlreadyExecuted
            );
            engine.state.tier_3_complete = true;
            engine.state.current_tier = engine.state.current_tier.max(3);
        }
        _ => return err!(OracleError::InvalidDripTier),
    }

    engine.state.last_drip = now;
    Ok(())
}
