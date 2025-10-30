use crate::{
    constants::{ADMIN_AUTHORITY, PROTOCOL_SEED},
    errors::ProtocolError,
    state::{FeeConfig, ProtocolState},
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint as SplMint;

#[derive(Accounts)]
pub struct InitializeMint<'info> {
    #[account(
        mut,
        constraint = admin.key() == ADMIN_AUTHORITY @ ProtocolError::Unauthorized
    )]
    pub admin: Signer<'info>,

    /// CCM Token-2022 mint (created externally with spl-token CLI)
    pub milo_mint: InterfaceAccount<'info, SplMint>,

    /// Protocol state PDA
    #[account(
        init,
        payer = admin,
        space = ProtocolState::LEN,
        seeds = [PROTOCOL_SEED],
        bump
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Fee configuration PDA
    #[account(
        init,
        payer = admin,
        space = FeeConfig::LEN,
        seeds = [PROTOCOL_SEED, b"fee_config"],
        bump
    )]
    pub fee_config: Account<'info, FeeConfig>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<InitializeMint>, fee_basis_points: u16, max_fee: u64) -> Result<()> {
    require!(
        fee_basis_points as u16 <= crate::constants::MAX_FEE_BASIS_POINTS,
        ProtocolError::InvalidFeeBps
    );

    let protocol_state = &mut ctx.accounts.protocol_state;
    require!(
        !protocol_state.is_initialized,
        ProtocolError::AlreadyInitialized
    );

    protocol_state.is_initialized = true;
    protocol_state.version = 1;
    protocol_state.admin = ctx.accounts.admin.key();
    protocol_state.publisher = ctx.accounts.admin.key();
    protocol_state.treasury = protocol_state.key();
    protocol_state.mint = ctx.accounts.milo_mint.key();
    protocol_state.paused = false;
    protocol_state.bump = ctx.bumps.protocol_state;

    let fee_cfg = &mut ctx.accounts.fee_config;
    fee_cfg.basis_points = fee_basis_points;
    fee_cfg.max_fee = max_fee;
    fee_cfg.drip_threshold = crate::constants::DRIP_THRESHOLD;
    fee_cfg.bump = ctx.bumps.fee_config;

    Ok(())
}

// ---------------------------------------------
// Permissionless variant: seeds include the mint
// Anyone can initialize a protocol_state for their mint
// ---------------------------------------------

#[derive(Accounts)]
pub struct InitializeMintOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// Token-2022 mint (created externally)
    pub milo_mint: InterfaceAccount<'info, SplMint>,

    /// Protocol state PDA (keyed by mint)
    #[account(
        init,
        payer = admin,
        space = ProtocolState::LEN,
        seeds = [PROTOCOL_SEED, milo_mint.key().as_ref()],
        bump
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Fee configuration PDA (keyed by mint)
    #[account(
        init,
        payer = admin,
        space = FeeConfig::LEN,
        seeds = [PROTOCOL_SEED, milo_mint.key().as_ref(), b"fee_config"],
        bump
    )]
    pub fee_config: Account<'info, FeeConfig>,

    pub system_program: Program<'info, System>,
}

pub fn handler_open(
    ctx: Context<InitializeMintOpen>,
    fee_basis_points: u16,
    max_fee: u64,
) -> Result<()> {
    require!(
        fee_basis_points as u16 <= crate::constants::MAX_FEE_BASIS_POINTS,
        ProtocolError::InvalidFeeBps
    );

    let protocol_state = &mut ctx.accounts.protocol_state;
    require!(
        !protocol_state.is_initialized,
        ProtocolError::AlreadyInitialized
    );

    protocol_state.is_initialized = true;
    protocol_state.version = 1;
    protocol_state.admin = ctx.accounts.admin.key();
    protocol_state.publisher = ctx.accounts.admin.key();
    protocol_state.treasury = protocol_state.key();
    protocol_state.mint = ctx.accounts.milo_mint.key();
    protocol_state.paused = false;
    protocol_state.bump = ctx.bumps.protocol_state;

    let fee_cfg = &mut ctx.accounts.fee_config;
    fee_cfg.basis_points = fee_basis_points;
    fee_cfg.max_fee = max_fee;
    fee_cfg.drip_threshold = crate::constants::DRIP_THRESHOLD;
    fee_cfg.bump = ctx.bumps.fee_config;

    Ok(())
}
