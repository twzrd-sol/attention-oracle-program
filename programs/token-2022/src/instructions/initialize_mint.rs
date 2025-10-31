use crate::{
    constants::{ADMIN_AUTHORITY, PROTOCOL_SEED},
    errors::ProtocolError,
    state::{FeeConfig, ProtocolState},
};
use anchor_lang::prelude::*;
use anchor_spl::token_2022::spl_token_2022::{
    extension::{transfer_fee::TransferFeeConfig, BaseStateWithExtensions, StateWithExtensions},
    state::Mint as RawMint,
};
use anchor_spl::token_interface::Mint as SplMint;

#[derive(Accounts)]
pub struct InitializeMint<'info> {
    #[account(
        mut,
        constraint = admin.key() == ADMIN_AUTHORITY @ ProtocolError::Unauthorized
    )]
    pub admin: Signer<'info>,

    /// CCM Token-2022 mint (Token-2022 with transfer-fee extension enabled)
    pub mint: InterfaceAccount<'info, SplMint>,

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
        fee_basis_points <= crate::constants::MAX_FEE_BASIS_POINTS,
        ProtocolError::InvalidFeeBps
    );
    require!(
        max_fee <= crate::constants::MAX_FEE_AMOUNT,
        ProtocolError::InvalidAmount
    );

    assert_transfer_fee_extension(&ctx.accounts.mint)?;

    let protocol_state = &mut ctx.accounts.protocol_state;
    require!(
        !protocol_state.is_initialized,
        ProtocolError::AlreadyInitialized
    );

    protocol_state.is_initialized = true;
    protocol_state.version = 1;
    protocol_state.admin = ctx.accounts.admin.key();
    protocol_state.publisher = Pubkey::default();
    protocol_state.treasury = protocol_state.key();
    protocol_state.mint = ctx.accounts.mint.key();
    protocol_state.paused = false;
    protocol_state.require_receipt = false;
    protocol_state.bump = ctx.bumps.protocol_state;

    let fee_cfg = &mut ctx.accounts.fee_config;
    fee_cfg.basis_points = fee_basis_points;
    fee_cfg.max_fee = max_fee;
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
    pub mint: InterfaceAccount<'info, SplMint>,

    /// Protocol state PDA (keyed by mint)
    #[account(
        init,
        payer = admin,
        space = ProtocolState::LEN,
        seeds = [PROTOCOL_SEED, mint.key().as_ref()],
        bump
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Fee configuration PDA (keyed by mint)
    #[account(
        init,
        payer = admin,
        space = FeeConfig::LEN,
        seeds = [PROTOCOL_SEED, mint.key().as_ref(), b"fee_config"],
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
        fee_basis_points <= crate::constants::MAX_FEE_BASIS_POINTS,
        ProtocolError::InvalidFeeBps
    );
    require!(
        max_fee <= crate::constants::MAX_FEE_AMOUNT,
        ProtocolError::InvalidAmount
    );

    assert_transfer_fee_extension(&ctx.accounts.mint)?;

    let protocol_state = &mut ctx.accounts.protocol_state;
    require!(
        !protocol_state.is_initialized,
        ProtocolError::AlreadyInitialized
    );

    protocol_state.is_initialized = true;
    protocol_state.version = 1;
    protocol_state.admin = ctx.accounts.admin.key();
    protocol_state.publisher = Pubkey::default();
    protocol_state.treasury = protocol_state.key();
    protocol_state.mint = ctx.accounts.mint.key();
    protocol_state.paused = false;
    protocol_state.require_receipt = false;
    protocol_state.bump = ctx.bumps.protocol_state;

    let fee_cfg = &mut ctx.accounts.fee_config;
    fee_cfg.basis_points = fee_basis_points;
    fee_cfg.max_fee = max_fee;
    fee_cfg.bump = ctx.bumps.fee_config;

    Ok(())
}

fn assert_transfer_fee_extension(mint: &InterfaceAccount<SplMint>) -> Result<()> {
    let account_info = mint.to_account_info();
    let data = account_info.try_borrow_data()?;
    let state = StateWithExtensions::<RawMint>::unpack(&data)
        .map_err(|_| error!(ProtocolError::InvalidMint))?;
    require!(
        state.get_extension::<TransferFeeConfig>().is_ok(),
        ProtocolError::InvalidMint
    );
    Ok(())
}
