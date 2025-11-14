use crate::{
    constants::PROTOCOL_SEED,
    errors::ProtocolError,
    state::{FeeConfig, FeeSplit, ProtocolState},
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UpdateFeeConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Global protocol state (authority + mint/treasury refs)
    #[account(
        mut,
        seeds = [PROTOCOL_SEED],
        bump = protocol_state.bump,
        constraint = authority.key() == protocol_state.admin @ ProtocolError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Fee configuration PDA
    #[account(
        mut,
        seeds = [PROTOCOL_SEED, b"fee_config"],
        bump = fee_config.bump
    )]
    pub fee_config: Account<'info, FeeConfig>,

    pub system_program: Program<'info, System>,
}

pub fn update_fee_config(
    ctx: Context<UpdateFeeConfig>,
    new_basis_points: u16,
    _fee_split: FeeSplit,
) -> Result<()> {
    require!(
        new_basis_points <= crate::constants::MAX_FEE_BASIS_POINTS,
        ProtocolError::InvalidFeeBps
    );

    // For v1, we store only basis_points and max_fee at init; allow updating basis points.
    let fee_cfg = &mut ctx.accounts.fee_config;
    fee_cfg.basis_points = new_basis_points;

    Ok(())
}

// Open variant for mint-keyed protocol instances
#[derive(Accounts)]
pub struct UpdateFeeConfigOpen<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Global protocol state (mint-keyed)
    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = authority.key() == protocol_state.admin @ ProtocolError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Fee configuration PDA (mint-keyed)
    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref(), b"fee_config"],
        bump = fee_config.bump
    )]
    pub fee_config: Account<'info, FeeConfig>,

    pub system_program: Program<'info, System>,
}

pub fn update_fee_config_open(
    ctx: Context<UpdateFeeConfigOpen>,
    new_basis_points: u16,
    _fee_split: FeeSplit,
) -> Result<()> {
    require!(
        new_basis_points <= crate::constants::MAX_FEE_BASIS_POINTS,
        ProtocolError::InvalidFeeBps
    );

    let fee_cfg = &mut ctx.accounts.fee_config;
    fee_cfg.basis_points = new_basis_points;
    Ok(())
}
