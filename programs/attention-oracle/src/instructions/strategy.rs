//! Kamino K-Lend strategy vault — Phase 2.
//!
//! Single-strategy: deploys idle USDC from MarketVault into Kamino K-Lend,
//! receives cUSDC (cTokens) as receipt. No multi-protocol routing.
//!
//! Accounting: `deployed_amount` = raw USDC principal sent to Kamino.
//! NAV is derived off-chain from the cToken balance and reserve state.
//!
//! `settle_market` NEVER calls Kamino CPI. It draws only from `vault_usdc_ata`.
//! If reserve liquidity is short, settlement reverts with `InsufficientReserve`
//! before any burn occurs, and the server-side queue handles deferred exits.

use anchor_lang::{
    prelude::*,
    solana_program::{
        program::{invoke, invoke_signed},
        sysvar::instructions::ID as SYSVAR_INSTRUCTIONS_ID,
    },
};
use anchor_spl::token::{Mint as SplMint, Token, TokenAccount as SplTokenAccount};

use crate::constants::{BPS_DENOMINATOR, MARKET_VAULT_SEED, STRATEGY_VAULT_SEED};
use crate::errors::OracleError;
use crate::klend::{
    self, DepositReserveLiquidityKeys, RedeemReserveCollateralKeys, RefreshReserveKeys,
};
use crate::state::{MarketVault, ProtocolState, StrategyVault};

const STRATEGY_VAULT_VERSION: u8 = 1;
const STRATEGY_STATUS_ACTIVE: u8 = 0;
const STRATEGY_STATUS_EMERGENCY: u8 = 1;
const MIN_DEPLOY_AMOUNT: u64 = 1_000_000; // 1 USDC (6 decimals)
const KAMINO_STATUS_ACTIVE: u8 = 0;

// =============================================================================
// INITIALIZE STRATEGY VAULT
// =============================================================================

#[derive(Accounts)]
pub struct InitializeStrategyVault<'info> {
    #[account(mut)]
    pub admin_authority: Signer<'info>,

    #[account(
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
        constraint = protocol_state.admin == admin_authority.key() @ OracleError::Unauthorized,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    #[account(
        seeds = [MARKET_VAULT_SEED, protocol_state.key().as_ref(), &market_vault.market_id.to_le_bytes()],
        bump = market_vault.bump,
    )]
    pub market_vault: Box<Account<'info, MarketVault>>,

    #[account(address = market_vault.deposit_mint)]
    pub deposit_mint: Box<Account<'info, SplMint>>,

    #[account(
        init,
        payer = admin_authority,
        space = StrategyVault::LEN,
        seeds = [STRATEGY_VAULT_SEED, market_vault.key().as_ref()],
        bump,
    )]
    pub strategy_vault: Box<Account<'info, StrategyVault>>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_strategy_vault(
    ctx: Context<InitializeStrategyVault>,
    reserve_ratio_bps: u16,
    utilization_cap_bps: u16,
    operator_authority: Pubkey,
    klend_program: Pubkey,
    klend_reserve: Pubkey,
    klend_lending_market: Pubkey,
    ctoken_ata: Pubkey,
) -> Result<()> {
    require!(reserve_ratio_bps <= 10_000, OracleError::InvalidInputLength);
    require!(
        utilization_cap_bps <= 10_000,
        OracleError::InvalidInputLength
    );
    require!(
        reserve_ratio_bps
            .checked_add(utilization_cap_bps)
            .ok_or(OracleError::MathOverflow)?
            <= 10_000,
        OracleError::InvalidInputLength
    );
    require!(
        operator_authority != Pubkey::default(),
        OracleError::InvalidPubkey
    );
    require!(
        klend_program != Pubkey::default(),
        OracleError::InvalidPubkey
    );
    require!(
        klend_reserve != Pubkey::default(),
        OracleError::InvalidPubkey
    );
    require!(
        klend_lending_market != Pubkey::default(),
        OracleError::InvalidPubkey
    );
    require!(ctoken_ata != Pubkey::default(), OracleError::InvalidPubkey);

    let sv = &mut ctx.accounts.strategy_vault;
    sv.version = STRATEGY_VAULT_VERSION;
    sv.bump = ctx.bumps.strategy_vault;
    sv.status = STRATEGY_STATUS_ACTIVE;
    sv.reserve_ratio_bps = reserve_ratio_bps;
    sv.utilization_cap_bps = utilization_cap_bps;
    sv.protocol_state = ctx.accounts.protocol_state.key();
    sv.market_vault = ctx.accounts.market_vault.key();
    sv.deposit_mint = ctx.accounts.deposit_mint.key();
    sv.admin_authority = ctx.accounts.admin_authority.key();
    sv.operator_authority = operator_authority;
    sv.klend_program = klend_program;
    sv.klend_reserve = klend_reserve;
    sv.klend_lending_market = klend_lending_market;
    sv.ctoken_ata = ctoken_ata;
    sv.deployed_amount = 0;
    sv.pending_withdraw_amount = 0;
    sv.harvested_yield_amount = 0;
    sv.last_deploy_slot = 0;
    sv.last_withdraw_slot = 0;
    sv.last_harvest_slot = 0;

    Ok(())
}

// =============================================================================
// DEPLOY TO STRATEGY (USDC → Kamino cUSDC)
// =============================================================================

#[derive(Accounts)]
pub struct DeployToStrategy<'info> {
    #[account(mut)]
    pub operator_authority: Signer<'info>,

    #[account(
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    #[account(
        seeds = [MARKET_VAULT_SEED, protocol_state.key().as_ref(), &market_vault.market_id.to_le_bytes()],
        bump = market_vault.bump,
    )]
    pub market_vault: Box<Account<'info, MarketVault>>,

    #[account(
        mut,
        seeds = [STRATEGY_VAULT_SEED, market_vault.key().as_ref()],
        bump = strategy_vault.bump,
        has_one = protocol_state,
        has_one = market_vault,
        has_one = operator_authority,
        constraint = strategy_vault.status == STRATEGY_STATUS_ACTIVE @ OracleError::StrategyInactive,
    )]
    pub strategy_vault: Box<Account<'info, StrategyVault>>,

    #[account(address = strategy_vault.deposit_mint)]
    pub deposit_mint: Box<Account<'info, SplMint>>,

    #[account(
        mut,
        constraint = vault_usdc_ata.owner == market_vault.key(),
        constraint = vault_usdc_ata.mint == deposit_mint.key(),
    )]
    pub vault_usdc_ata: Box<Account<'info, SplTokenAccount>>,

    #[account(
        mut,
        address = strategy_vault.ctoken_ata,
        constraint = ctoken_ata.owner == market_vault.key(),
    )]
    pub ctoken_ata: Box<Account<'info, SplTokenAccount>>,

    /// CHECK: Kamino K-Lend program. Address pinned at init.
    #[account(address = strategy_vault.klend_program)]
    pub klend_program: UncheckedAccount<'info>,

    /// CHECK: Kamino USDC Reserve. Address pinned at init.
    #[account(mut, address = strategy_vault.klend_reserve)]
    pub klend_reserve: UncheckedAccount<'info>,

    /// CHECK: Kamino lending market. Address pinned at init.
    #[account(address = strategy_vault.klend_lending_market)]
    pub klend_lending_market: UncheckedAccount<'info>,

    /// CHECK: Kamino lending market authority PDA, derived from lending market.
    pub klend_lending_market_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = reserve_liquidity_supply.mint == deposit_mint.key(),
    )]
    pub reserve_liquidity_supply: Box<Account<'info, SplTokenAccount>>,

    #[account(mut)]
    pub reserve_collateral_mint: Box<Account<'info, SplMint>>,

    /// CHECK: Oracle account required by Kamino refresh_reserve.
    pub pyth_oracle: UncheckedAccount<'info>,

    /// CHECK: Oracle account required by Kamino refresh_reserve.
    pub switchboard_price_oracle: UncheckedAccount<'info>,

    /// CHECK: Oracle account required by Kamino refresh_reserve.
    pub switchboard_twap_oracle: UncheckedAccount<'info>,

    /// CHECK: Oracle account required by Kamino refresh_reserve.
    pub scope_prices: UncheckedAccount<'info>,

    /// CHECK: Sysvar instructions account required by Kamino deposit/redeem.
    #[account(address = SYSVAR_INSTRUCTIONS_ID)]
    pub instruction_sysvar_account: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

#[inline(never)]
pub fn deploy_to_strategy(ctx: Context<DeployToStrategy>, amount: u64) -> Result<()> {
    require!(
        amount >= MIN_DEPLOY_AMOUNT,
        OracleError::StrategyAmountTooSmall
    );

    let reserve_balance = ctx.accounts.vault_usdc_ata.amount;
    let deployed_amount = ctx.accounts.strategy_vault.deployed_amount;
    let total_managed = reserve_balance
        .checked_add(deployed_amount)
        .ok_or(OracleError::MathOverflow)?;
    let reserve_floor = total_managed
        .checked_mul(u64::from(ctx.accounts.strategy_vault.reserve_ratio_bps))
        .ok_or(OracleError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR)
        .ok_or(OracleError::MathOverflow)?;
    let new_deployed = deployed_amount
        .checked_add(amount)
        .ok_or(OracleError::MathOverflow)?;
    let max_deployed = total_managed
        .checked_mul(u64::from(ctx.accounts.strategy_vault.utilization_cap_bps))
        .ok_or(OracleError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR)
        .ok_or(OracleError::MathOverflow)?;

    require!(
        reserve_balance.saturating_sub(amount) >= reserve_floor,
        OracleError::InsufficientReserve
    );
    require!(
        new_deployed <= max_deployed,
        OracleError::UtilizationCapExceeded
    );

    let reserve = load_and_validate_kamino_reserve(
        &ctx.accounts.strategy_vault,
        ctx.accounts.market_vault.key(),
        &ctx.accounts.deposit_mint.to_account_info(),
        &ctx.accounts.ctoken_ata,
        &ctx.accounts.ctoken_ata.to_account_info(),
        &ctx.accounts.klend_program.to_account_info(),
        &ctx.accounts.klend_reserve.to_account_info(),
        &ctx.accounts.klend_lending_market.to_account_info(),
        &ctx.accounts
            .klend_lending_market_authority
            .to_account_info(),
        &ctx.accounts.reserve_liquidity_supply.to_account_info(),
        &ctx.accounts.reserve_collateral_mint.to_account_info(),
        &ctx.accounts.pyth_oracle.to_account_info(),
        &ctx.accounts.switchboard_price_oracle.to_account_info(),
        &ctx.accounts.switchboard_twap_oracle.to_account_info(),
        &ctx.accounts.scope_prices.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
    )?;
    require!(
        reserve.config.status == KAMINO_STATUS_ACTIVE,
        OracleError::InvalidExternalState
    );

    let preview_collateral = klend::preview_deposit_collateral(&reserve, amount)
        .ok_or(OracleError::InvalidExternalState)?;
    require!(preview_collateral > 0, OracleError::StrategyAmountTooSmall);

    invoke_refresh_reserve(
        ctx.accounts.klend_program.key(),
        &ctx.accounts.klend_reserve.to_account_info(),
        &ctx.accounts.klend_lending_market.to_account_info(),
        &ctx.accounts.pyth_oracle.to_account_info(),
        &ctx.accounts.switchboard_price_oracle.to_account_info(),
        &ctx.accounts.switchboard_twap_oracle.to_account_info(),
        &ctx.accounts.scope_prices.to_account_info(),
    )?;

    let protocol_state_key = ctx.accounts.protocol_state.key();
    let market_id_bytes = ctx.accounts.market_vault.market_id.to_le_bytes();
    let market_vault_bump = [ctx.accounts.market_vault.bump];
    let market_vault_signer: &[&[u8]] = &[
        MARKET_VAULT_SEED,
        protocol_state_key.as_ref(),
        market_id_bytes.as_ref(),
        market_vault_bump.as_ref(),
    ];

    let pre_ctoken_balance = ctx.accounts.ctoken_ata.amount;
    invoke_deposit_reserve_liquidity(
        ctx.accounts.klend_program.key(),
        DepositReserveLiquidityKeys {
            owner: ctx.accounts.market_vault.key(),
            reserve: ctx.accounts.klend_reserve.key(),
            lending_market: ctx.accounts.klend_lending_market.key(),
            lending_market_authority: ctx.accounts.klend_lending_market_authority.key(),
            reserve_liquidity_mint: ctx.accounts.deposit_mint.key(),
            reserve_liquidity_supply: ctx.accounts.reserve_liquidity_supply.key(),
            reserve_collateral_mint: ctx.accounts.reserve_collateral_mint.key(),
            user_source_liquidity: ctx.accounts.vault_usdc_ata.key(),
            user_destination_collateral: ctx.accounts.ctoken_ata.key(),
            collateral_token_program: ctx.accounts.token_program.key(),
            liquidity_token_program: ctx.accounts.token_program.key(),
            instruction_sysvar_account: ctx.accounts.instruction_sysvar_account.key(),
        },
        amount,
        &[
            ctx.accounts.market_vault.to_account_info(),
            ctx.accounts.klend_reserve.to_account_info(),
            ctx.accounts.klend_lending_market.to_account_info(),
            ctx.accounts
                .klend_lending_market_authority
                .to_account_info(),
            ctx.accounts.deposit_mint.to_account_info(),
            ctx.accounts.reserve_liquidity_supply.to_account_info(),
            ctx.accounts.reserve_collateral_mint.to_account_info(),
            ctx.accounts.vault_usdc_ata.to_account_info(),
            ctx.accounts.ctoken_ata.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.instruction_sysvar_account.to_account_info(),
        ],
        market_vault_signer,
    )?;

    ctx.accounts.ctoken_ata.reload()?;
    require!(
        ctx.accounts.ctoken_ata.amount > pre_ctoken_balance,
        OracleError::StrategyAmountTooSmall
    );

    let clock = Clock::get()?;
    let strategy_vault = &mut ctx.accounts.strategy_vault;
    strategy_vault.deployed_amount = new_deployed;
    strategy_vault.last_deploy_slot = clock.slot;

    msg!(
        "DeployToStrategy complete: amount={} reserve_floor={} deployed_amount={}",
        amount,
        reserve_floor,
        strategy_vault.deployed_amount
    );
    Ok(())
}

// =============================================================================
// WITHDRAW FROM STRATEGY (Kamino cUSDC → USDC)
// =============================================================================

#[derive(Accounts)]
pub struct WithdrawFromStrategy<'info> {
    #[account(mut)]
    pub operator_authority: Signer<'info>,

    #[account(
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    #[account(
        seeds = [MARKET_VAULT_SEED, protocol_state.key().as_ref(), &market_vault.market_id.to_le_bytes()],
        bump = market_vault.bump,
    )]
    pub market_vault: Box<Account<'info, MarketVault>>,

    #[account(
        mut,
        seeds = [STRATEGY_VAULT_SEED, market_vault.key().as_ref()],
        bump = strategy_vault.bump,
        has_one = protocol_state,
        has_one = market_vault,
        has_one = operator_authority,
    )]
    pub strategy_vault: Box<Account<'info, StrategyVault>>,

    #[account(address = strategy_vault.deposit_mint)]
    pub deposit_mint: Box<Account<'info, SplMint>>,

    #[account(
        mut,
        constraint = vault_usdc_ata.owner == market_vault.key(),
        constraint = vault_usdc_ata.mint == deposit_mint.key(),
    )]
    pub vault_usdc_ata: Box<Account<'info, SplTokenAccount>>,

    #[account(
        mut,
        address = strategy_vault.ctoken_ata,
        constraint = ctoken_ata.owner == market_vault.key(),
    )]
    pub ctoken_ata: Box<Account<'info, SplTokenAccount>>,

    /// CHECK: Kamino K-Lend program.
    #[account(address = strategy_vault.klend_program)]
    pub klend_program: UncheckedAccount<'info>,

    /// CHECK: Kamino USDC Reserve.
    #[account(mut, address = strategy_vault.klend_reserve)]
    pub klend_reserve: UncheckedAccount<'info>,

    /// CHECK: Kamino lending market.
    #[account(address = strategy_vault.klend_lending_market)]
    pub klend_lending_market: UncheckedAccount<'info>,

    /// CHECK: Kamino lending market authority PDA.
    pub klend_lending_market_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = reserve_liquidity_supply.mint == deposit_mint.key(),
    )]
    pub reserve_liquidity_supply: Box<Account<'info, SplTokenAccount>>,

    #[account(mut)]
    pub reserve_collateral_mint: Box<Account<'info, SplMint>>,

    /// CHECK: Oracle account required by Kamino refresh_reserve.
    pub pyth_oracle: UncheckedAccount<'info>,

    /// CHECK: Oracle account required by Kamino refresh_reserve.
    pub switchboard_price_oracle: UncheckedAccount<'info>,

    /// CHECK: Oracle account required by Kamino refresh_reserve.
    pub switchboard_twap_oracle: UncheckedAccount<'info>,

    /// CHECK: Oracle account required by Kamino refresh_reserve.
    pub scope_prices: UncheckedAccount<'info>,

    /// CHECK: Sysvar instructions account required by Kamino deposit/redeem.
    #[account(address = SYSVAR_INSTRUCTIONS_ID)]
    pub instruction_sysvar_account: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

#[inline(never)]
pub fn withdraw_from_strategy(ctx: Context<WithdrawFromStrategy>, amount: u64) -> Result<()> {
    require!(amount > 0, OracleError::InvalidInputLength);
    require!(
        ctx.accounts.strategy_vault.deployed_amount > 0,
        OracleError::InsufficientStrategyBalance
    );
    require!(
        amount <= ctx.accounts.strategy_vault.deployed_amount,
        OracleError::InsufficientStrategyBalance
    );

    let reserve = load_and_validate_kamino_reserve(
        &ctx.accounts.strategy_vault,
        ctx.accounts.market_vault.key(),
        &ctx.accounts.deposit_mint.to_account_info(),
        &ctx.accounts.ctoken_ata,
        &ctx.accounts.ctoken_ata.to_account_info(),
        &ctx.accounts.klend_program.to_account_info(),
        &ctx.accounts.klend_reserve.to_account_info(),
        &ctx.accounts.klend_lending_market.to_account_info(),
        &ctx.accounts
            .klend_lending_market_authority
            .to_account_info(),
        &ctx.accounts.reserve_liquidity_supply.to_account_info(),
        &ctx.accounts.reserve_collateral_mint.to_account_info(),
        &ctx.accounts.pyth_oracle.to_account_info(),
        &ctx.accounts.switchboard_price_oracle.to_account_info(),
        &ctx.accounts.switchboard_twap_oracle.to_account_info(),
        &ctx.accounts.scope_prices.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
    )?;
    require!(
        reserve.config.status == KAMINO_STATUS_ACTIVE,
        OracleError::InvalidExternalState
    );

    let now_ts = u64::try_from(Clock::get()?.unix_timestamp)
        .map_err(|_| error!(OracleError::InvalidExternalState))?;
    if let Some(remaining_capacity) = klend::remaining_withdrawal_capacity(&reserve, now_ts) {
        require!(
            amount <= remaining_capacity,
            OracleError::InvalidExternalState
        );
    }

    let collateral_amount = klend::collateral_amount_for_liquidity(&reserve, amount)
        .ok_or(OracleError::InvalidExternalState)?;
    require!(collateral_amount > 0, OracleError::InvalidExternalState);
    require!(
        collateral_amount <= ctx.accounts.ctoken_ata.amount,
        OracleError::InsufficientStrategyBalance
    );

    invoke_refresh_reserve(
        ctx.accounts.klend_program.key(),
        &ctx.accounts.klend_reserve.to_account_info(),
        &ctx.accounts.klend_lending_market.to_account_info(),
        &ctx.accounts.pyth_oracle.to_account_info(),
        &ctx.accounts.switchboard_price_oracle.to_account_info(),
        &ctx.accounts.switchboard_twap_oracle.to_account_info(),
        &ctx.accounts.scope_prices.to_account_info(),
    )?;

    let protocol_state_key = ctx.accounts.protocol_state.key();
    let market_id_bytes = ctx.accounts.market_vault.market_id.to_le_bytes();
    let market_vault_bump = [ctx.accounts.market_vault.bump];
    let market_vault_signer: &[&[u8]] = &[
        MARKET_VAULT_SEED,
        protocol_state_key.as_ref(),
        market_id_bytes.as_ref(),
        market_vault_bump.as_ref(),
    ];

    let pre_ctoken_balance = ctx.accounts.ctoken_ata.amount;
    let pre_vault_balance = ctx.accounts.vault_usdc_ata.amount;
    invoke_redeem_reserve_collateral(
        ctx.accounts.klend_program.key(),
        RedeemReserveCollateralKeys {
            owner: ctx.accounts.market_vault.key(),
            lending_market: ctx.accounts.klend_lending_market.key(),
            reserve: ctx.accounts.klend_reserve.key(),
            lending_market_authority: ctx.accounts.klend_lending_market_authority.key(),
            reserve_liquidity_mint: ctx.accounts.deposit_mint.key(),
            reserve_collateral_mint: ctx.accounts.reserve_collateral_mint.key(),
            reserve_liquidity_supply: ctx.accounts.reserve_liquidity_supply.key(),
            user_source_collateral: ctx.accounts.ctoken_ata.key(),
            user_destination_liquidity: ctx.accounts.vault_usdc_ata.key(),
            collateral_token_program: ctx.accounts.token_program.key(),
            liquidity_token_program: ctx.accounts.token_program.key(),
            instruction_sysvar_account: ctx.accounts.instruction_sysvar_account.key(),
        },
        collateral_amount,
        &[
            ctx.accounts.market_vault.to_account_info(),
            ctx.accounts.klend_lending_market.to_account_info(),
            ctx.accounts.klend_reserve.to_account_info(),
            ctx.accounts
                .klend_lending_market_authority
                .to_account_info(),
            ctx.accounts.deposit_mint.to_account_info(),
            ctx.accounts.reserve_collateral_mint.to_account_info(),
            ctx.accounts.reserve_liquidity_supply.to_account_info(),
            ctx.accounts.ctoken_ata.to_account_info(),
            ctx.accounts.vault_usdc_ata.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.instruction_sysvar_account.to_account_info(),
        ],
        market_vault_signer,
    )?;

    ctx.accounts.ctoken_ata.reload()?;
    ctx.accounts.vault_usdc_ata.reload()?;
    let received_liquidity = ctx
        .accounts
        .vault_usdc_ata
        .amount
        .saturating_sub(pre_vault_balance);
    require!(received_liquidity > 0, OracleError::InvalidExternalState);

    let old_deployed = ctx.accounts.strategy_vault.deployed_amount;
    let post_ctoken_balance = ctx.accounts.ctoken_ata.amount;
    let new_deployed = if pre_ctoken_balance == 0 || post_ctoken_balance == 0 {
        0
    } else {
        u64::try_from(
            u128::from(old_deployed)
                .checked_mul(u128::from(post_ctoken_balance))
                .ok_or(OracleError::MathOverflow)?
                .checked_div(u128::from(pre_ctoken_balance))
                .ok_or(OracleError::MathOverflow)?,
        )
        .map_err(|_| error!(OracleError::MathOverflow))?
    };

    let clock = Clock::get()?;
    let strategy_vault = &mut ctx.accounts.strategy_vault;
    strategy_vault.deployed_amount = new_deployed;
    strategy_vault.last_withdraw_slot = clock.slot;
    if strategy_vault.status == STRATEGY_STATUS_EMERGENCY {
        strategy_vault.pending_withdraw_amount = new_deployed;
    }

    msg!(
        "WithdrawFromStrategy complete: requested_liquidity={} collateral_burned={} received_liquidity={} remaining_deployed={}",
        amount,
        collateral_amount,
        received_liquidity,
        strategy_vault.deployed_amount
    );
    Ok(())
}

// =============================================================================
// HARVEST STRATEGY YIELD
// =============================================================================

#[derive(Accounts)]
pub struct HarvestStrategyYield<'info> {
    #[account(mut)]
    pub operator_authority: Signer<'info>,

    #[account(
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    #[account(
        seeds = [MARKET_VAULT_SEED, protocol_state.key().as_ref(), &market_vault.market_id.to_le_bytes()],
        bump = market_vault.bump,
    )]
    pub market_vault: Box<Account<'info, MarketVault>>,

    #[account(
        mut,
        seeds = [STRATEGY_VAULT_SEED, market_vault.key().as_ref()],
        bump = strategy_vault.bump,
        has_one = protocol_state,
        has_one = market_vault,
        has_one = operator_authority,
        constraint = strategy_vault.status == STRATEGY_STATUS_ACTIVE @ OracleError::StrategyInactive,
    )]
    pub strategy_vault: Box<Account<'info, StrategyVault>>,

    #[account(address = strategy_vault.deposit_mint)]
    pub deposit_mint: Box<Account<'info, SplMint>>,

    /// MarketVault's USDC ATA — not used for yield destination, but required
    /// by load_and_validate_kamino_reserve to verify ownership constraints.
    #[account(
        constraint = vault_usdc_ata.owner == market_vault.key(),
        constraint = vault_usdc_ata.mint == deposit_mint.key(),
    )]
    pub vault_usdc_ata: Box<Account<'info, SplTokenAccount>>,

    /// Protocol treasury USDC ATA — yield (NAV above principal) flows here.
    #[account(
        mut,
        constraint = treasury_ata.owner == protocol_state.treasury,
        constraint = treasury_ata.mint == deposit_mint.key(),
    )]
    pub treasury_ata: Box<Account<'info, SplTokenAccount>>,

    #[account(
        mut,
        address = strategy_vault.ctoken_ata,
    )]
    pub ctoken_ata: Box<Account<'info, SplTokenAccount>>,

    /// CHECK: Kamino K-Lend program.
    #[account(address = strategy_vault.klend_program)]
    pub klend_program: UncheckedAccount<'info>,

    /// CHECK: Kamino USDC Reserve.
    #[account(mut, address = strategy_vault.klend_reserve)]
    pub klend_reserve: UncheckedAccount<'info>,

    /// CHECK: Kamino lending market.
    #[account(address = strategy_vault.klend_lending_market)]
    pub klend_lending_market: UncheckedAccount<'info>,

    /// CHECK: Kamino lending market authority PDA.
    pub klend_lending_market_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub reserve_liquidity_supply: Box<Account<'info, SplTokenAccount>>,

    #[account(mut)]
    pub reserve_collateral_mint: Box<Account<'info, SplMint>>,

    /// CHECK: Oracle sentinel required by Kamino refresh_reserve.
    pub pyth_oracle: UncheckedAccount<'info>,

    /// CHECK: Oracle sentinel required by Kamino refresh_reserve.
    pub switchboard_price_oracle: UncheckedAccount<'info>,

    /// CHECK: Oracle sentinel required by Kamino refresh_reserve.
    pub switchboard_twap_oracle: UncheckedAccount<'info>,

    /// CHECK: Oracle sentinel required by Kamino refresh_reserve.
    pub scope_prices: UncheckedAccount<'info>,

    /// CHECK: Sysvar instructions account required by Kamino redeem.
    #[account(address = SYSVAR_INSTRUCTIONS_ID)]
    pub instruction_sysvar_account: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

/// Harvest the yield portion of cTokens (NAV above `deployed_amount`) and
/// send the redeemed USDC to the protocol treasury ATA.
///
/// `deployed_amount` is NOT changed — it tracks only principal.
/// `harvested_yield_amount` accumulates the running total of extracted yield.
#[inline(never)]
pub fn harvest_strategy_yield(ctx: Context<HarvestStrategyYield>) -> Result<()> {
    let ctoken_balance = ctx.accounts.ctoken_ata.amount;
    if ctoken_balance == 0 {
        msg!("HarvestStrategyYield: no cTokens held, nothing to harvest");
        return Ok(());
    }

    let reserve = load_and_validate_kamino_reserve(
        &ctx.accounts.strategy_vault,
        ctx.accounts.market_vault.key(),
        &ctx.accounts.deposit_mint.to_account_info(),
        &ctx.accounts.ctoken_ata,
        &ctx.accounts.ctoken_ata.to_account_info(),
        &ctx.accounts.klend_program.to_account_info(),
        &ctx.accounts.klend_reserve.to_account_info(),
        &ctx.accounts.klend_lending_market.to_account_info(),
        &ctx.accounts
            .klend_lending_market_authority
            .to_account_info(),
        &ctx.accounts.reserve_liquidity_supply.to_account_info(),
        &ctx.accounts.reserve_collateral_mint.to_account_info(),
        &ctx.accounts.pyth_oracle.to_account_info(),
        &ctx.accounts.switchboard_price_oracle.to_account_info(),
        &ctx.accounts.switchboard_twap_oracle.to_account_info(),
        &ctx.accounts.scope_prices.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
    )?;

    // NAV = our_ctoken_balance * total_reserve_liquidity / collateral_mint_supply
    let total_liq = klend::total_liquidity(&reserve).ok_or(OracleError::InvalidExternalState)?;
    let mint_supply = reserve.collateral.mint_total_supply;
    if total_liq == 0 || mint_supply == 0 {
        msg!("HarvestStrategyYield: reserve empty, skipping");
        return Ok(());
    }
    let nav_usdc: u64 = u64::try_from(
        u128::from(ctoken_balance)
            .checked_mul(u128::from(total_liq))
            .ok_or(OracleError::MathOverflow)?
            .checked_div(u128::from(mint_supply))
            .ok_or(OracleError::MathOverflow)?,
    )
    .map_err(|_| error!(OracleError::MathOverflow))?;

    let deployed = ctx.accounts.strategy_vault.deployed_amount;
    if nav_usdc <= deployed {
        msg!(
            "HarvestStrategyYield: no yield (nav={} deployed={})",
            nav_usdc,
            deployed
        );
        return Ok(());
    }
    let yield_usdc = nav_usdc.saturating_sub(deployed);

    // Convert yield USDC back to cToken amount (rounds up — safe: we have buffer)
    let yield_ctoken = klend::collateral_amount_for_liquidity(&reserve, yield_usdc)
        .ok_or(OracleError::InvalidExternalState)?;
    require!(yield_ctoken > 0, OracleError::InvalidExternalState);
    require!(
        yield_ctoken <= ctoken_balance,
        OracleError::InsufficientStrategyBalance
    );

    invoke_refresh_reserve(
        ctx.accounts.klend_program.key(),
        &ctx.accounts.klend_reserve.to_account_info(),
        &ctx.accounts.klend_lending_market.to_account_info(),
        &ctx.accounts.pyth_oracle.to_account_info(),
        &ctx.accounts.switchboard_price_oracle.to_account_info(),
        &ctx.accounts.switchboard_twap_oracle.to_account_info(),
        &ctx.accounts.scope_prices.to_account_info(),
    )?;

    // Redeem yield cTokens → treasury_ata (principal cTokens remain in ctoken_ata)
    let protocol_state_key = ctx.accounts.protocol_state.key();
    let market_id_bytes = ctx.accounts.market_vault.market_id.to_le_bytes();
    let market_vault_bump = [ctx.accounts.market_vault.bump];
    let market_vault_signer: &[&[u8]] = &[
        MARKET_VAULT_SEED,
        protocol_state_key.as_ref(),
        market_id_bytes.as_ref(),
        market_vault_bump.as_ref(),
    ];

    let pre_treasury_balance = ctx.accounts.treasury_ata.amount;
    invoke_redeem_reserve_collateral(
        ctx.accounts.klend_program.key(),
        RedeemReserveCollateralKeys {
            owner: ctx.accounts.market_vault.key(),
            lending_market: ctx.accounts.klend_lending_market.key(),
            reserve: ctx.accounts.klend_reserve.key(),
            lending_market_authority: ctx.accounts.klend_lending_market_authority.key(),
            reserve_liquidity_mint: ctx.accounts.deposit_mint.key(),
            reserve_collateral_mint: ctx.accounts.reserve_collateral_mint.key(),
            reserve_liquidity_supply: ctx.accounts.reserve_liquidity_supply.key(),
            user_source_collateral: ctx.accounts.ctoken_ata.key(),
            user_destination_liquidity: ctx.accounts.treasury_ata.key(),
            collateral_token_program: ctx.accounts.token_program.key(),
            liquidity_token_program: ctx.accounts.token_program.key(),
            instruction_sysvar_account: ctx.accounts.instruction_sysvar_account.key(),
        },
        yield_ctoken,
        &[
            ctx.accounts.market_vault.to_account_info(),
            ctx.accounts.klend_lending_market.to_account_info(),
            ctx.accounts.klend_reserve.to_account_info(),
            ctx.accounts
                .klend_lending_market_authority
                .to_account_info(),
            ctx.accounts.deposit_mint.to_account_info(),
            ctx.accounts.reserve_collateral_mint.to_account_info(),
            ctx.accounts.reserve_liquidity_supply.to_account_info(),
            ctx.accounts.ctoken_ata.to_account_info(),
            ctx.accounts.treasury_ata.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.instruction_sysvar_account.to_account_info(),
        ],
        market_vault_signer,
    )?;

    ctx.accounts.treasury_ata.reload()?;
    let received = ctx
        .accounts
        .treasury_ata
        .amount
        .saturating_sub(pre_treasury_balance);
    require!(received > 0, OracleError::InvalidExternalState);

    let clock = Clock::get()?;
    let sv = &mut ctx.accounts.strategy_vault;
    sv.harvested_yield_amount = sv.harvested_yield_amount.saturating_add(received);
    sv.last_harvest_slot = clock.slot;

    msg!(
        "HarvestStrategyYield complete: yield_usdc_est={} ctoken_burned={} received={} total_harvested={}",
        yield_usdc,
        yield_ctoken,
        received,
        sv.harvested_yield_amount,
    );
    Ok(())
}

// =============================================================================
// EMERGENCY UNWIND
// =============================================================================

#[derive(Accounts)]
pub struct EmergencyUnwind<'info> {
    #[account(mut)]
    pub admin_authority: Signer<'info>,

    #[account(
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
        constraint = protocol_state.admin == admin_authority.key() @ OracleError::Unauthorized,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    #[account(
        seeds = [MARKET_VAULT_SEED, protocol_state.key().as_ref(), &market_vault.market_id.to_le_bytes()],
        bump = market_vault.bump,
    )]
    pub market_vault: Box<Account<'info, MarketVault>>,

    #[account(
        mut,
        seeds = [STRATEGY_VAULT_SEED, market_vault.key().as_ref()],
        bump = strategy_vault.bump,
        has_one = protocol_state,
        has_one = market_vault,
        has_one = admin_authority,
    )]
    pub strategy_vault: Box<Account<'info, StrategyVault>>,

    #[account(address = strategy_vault.deposit_mint)]
    pub deposit_mint: Box<Account<'info, SplMint>>,

    /// All redeemed USDC returns to the MarketVault pool so users can settle.
    #[account(
        mut,
        constraint = vault_usdc_ata.owner == market_vault.key(),
        constraint = vault_usdc_ata.mint == deposit_mint.key(),
    )]
    pub vault_usdc_ata: Box<Account<'info, SplTokenAccount>>,

    #[account(
        mut,
        address = strategy_vault.ctoken_ata,
    )]
    pub ctoken_ata: Box<Account<'info, SplTokenAccount>>,

    /// CHECK: Kamino K-Lend program.
    #[account(address = strategy_vault.klend_program)]
    pub klend_program: UncheckedAccount<'info>,

    /// CHECK: Kamino USDC Reserve.
    #[account(mut, address = strategy_vault.klend_reserve)]
    pub klend_reserve: UncheckedAccount<'info>,

    /// CHECK: Kamino lending market.
    #[account(address = strategy_vault.klend_lending_market)]
    pub klend_lending_market: UncheckedAccount<'info>,

    /// CHECK: Kamino lending market authority PDA.
    pub klend_lending_market_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub reserve_liquidity_supply: Box<Account<'info, SplTokenAccount>>,

    #[account(mut)]
    pub reserve_collateral_mint: Box<Account<'info, SplMint>>,

    /// CHECK: Oracle sentinel required by Kamino refresh_reserve.
    pub pyth_oracle: UncheckedAccount<'info>,

    /// CHECK: Oracle sentinel required by Kamino refresh_reserve.
    pub switchboard_price_oracle: UncheckedAccount<'info>,

    /// CHECK: Oracle sentinel required by Kamino refresh_reserve.
    pub switchboard_twap_oracle: UncheckedAccount<'info>,

    /// CHECK: Oracle sentinel required by Kamino refresh_reserve.
    pub scope_prices: UncheckedAccount<'info>,

    /// CHECK: Sysvar instructions account required by Kamino redeem.
    #[account(address = SYSVAR_INSTRUCTIONS_ID)]
    pub instruction_sysvar_account: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

/// Atomically arm EMERGENCY status and redeem ALL cTokens back to
/// `vault_usdc_ata` so users can settle immediately.
///
/// If Kamino rate-limits the redemption, the entire tx reverts and the
/// admin must retry. This is intentional — no partial state.
#[inline(never)]
pub fn emergency_unwind(ctx: Context<EmergencyUnwind>) -> Result<()> {
    let ctoken_balance = ctx.accounts.ctoken_ata.amount;

    // If nothing is deployed, just arm the flag and return.
    if ctoken_balance == 0 {
        let sv = &mut ctx.accounts.strategy_vault;
        sv.status = STRATEGY_STATUS_EMERGENCY;
        sv.pending_withdraw_amount = 0;
        sv.last_withdraw_slot = Clock::get()?.slot;
        msg!("EmergencyUnwind: no cTokens deployed, EMERGENCY armed");
        return Ok(());
    }

    let reserve = load_and_validate_kamino_reserve(
        &ctx.accounts.strategy_vault,
        ctx.accounts.market_vault.key(),
        &ctx.accounts.deposit_mint.to_account_info(),
        &ctx.accounts.ctoken_ata,
        &ctx.accounts.ctoken_ata.to_account_info(),
        &ctx.accounts.klend_program.to_account_info(),
        &ctx.accounts.klend_reserve.to_account_info(),
        &ctx.accounts.klend_lending_market.to_account_info(),
        &ctx.accounts
            .klend_lending_market_authority
            .to_account_info(),
        &ctx.accounts.reserve_liquidity_supply.to_account_info(),
        &ctx.accounts.reserve_collateral_mint.to_account_info(),
        &ctx.accounts.pyth_oracle.to_account_info(),
        &ctx.accounts.switchboard_price_oracle.to_account_info(),
        &ctx.accounts.switchboard_twap_oracle.to_account_info(),
        &ctx.accounts.scope_prices.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
    )?;

    invoke_refresh_reserve(
        ctx.accounts.klend_program.key(),
        &ctx.accounts.klend_reserve.to_account_info(),
        &ctx.accounts.klend_lending_market.to_account_info(),
        &ctx.accounts.pyth_oracle.to_account_info(),
        &ctx.accounts.switchboard_price_oracle.to_account_info(),
        &ctx.accounts.switchboard_twap_oracle.to_account_info(),
        &ctx.accounts.scope_prices.to_account_info(),
    )?;

    // Redeem ALL cTokens → vault_usdc_ata (user-protection: entire pool refunded)
    let protocol_state_key = ctx.accounts.protocol_state.key();
    let market_id_bytes = ctx.accounts.market_vault.market_id.to_le_bytes();
    let market_vault_bump = [ctx.accounts.market_vault.bump];
    let market_vault_signer: &[&[u8]] = &[
        MARKET_VAULT_SEED,
        protocol_state_key.as_ref(),
        market_id_bytes.as_ref(),
        market_vault_bump.as_ref(),
    ];

    let pre_vault_balance = ctx.accounts.vault_usdc_ata.amount;
    invoke_redeem_reserve_collateral(
        ctx.accounts.klend_program.key(),
        RedeemReserveCollateralKeys {
            owner: ctx.accounts.market_vault.key(),
            lending_market: ctx.accounts.klend_lending_market.key(),
            reserve: ctx.accounts.klend_reserve.key(),
            lending_market_authority: ctx.accounts.klend_lending_market_authority.key(),
            reserve_liquidity_mint: ctx.accounts.deposit_mint.key(),
            reserve_collateral_mint: ctx.accounts.reserve_collateral_mint.key(),
            reserve_liquidity_supply: ctx.accounts.reserve_liquidity_supply.key(),
            user_source_collateral: ctx.accounts.ctoken_ata.key(),
            user_destination_liquidity: ctx.accounts.vault_usdc_ata.key(),
            collateral_token_program: ctx.accounts.token_program.key(),
            liquidity_token_program: ctx.accounts.token_program.key(),
            instruction_sysvar_account: ctx.accounts.instruction_sysvar_account.key(),
        },
        ctoken_balance,
        &[
            ctx.accounts.market_vault.to_account_info(),
            ctx.accounts.klend_lending_market.to_account_info(),
            ctx.accounts.klend_reserve.to_account_info(),
            ctx.accounts
                .klend_lending_market_authority
                .to_account_info(),
            ctx.accounts.deposit_mint.to_account_info(),
            ctx.accounts.reserve_collateral_mint.to_account_info(),
            ctx.accounts.reserve_liquidity_supply.to_account_info(),
            ctx.accounts.ctoken_ata.to_account_info(),
            ctx.accounts.vault_usdc_ata.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.instruction_sysvar_account.to_account_info(),
        ],
        market_vault_signer,
    )?;

    ctx.accounts.vault_usdc_ata.reload()?;
    let received = ctx
        .accounts
        .vault_usdc_ata
        .amount
        .saturating_sub(pre_vault_balance);
    require!(received > 0, OracleError::InvalidExternalState);

    // Suppress unused variable warning from reserve validation (needed for PDA checks)
    let _ = reserve;

    let clock = Clock::get()?;
    let sv = &mut ctx.accounts.strategy_vault;
    sv.status = STRATEGY_STATUS_EMERGENCY;
    sv.deployed_amount = 0;
    sv.pending_withdraw_amount = 0;
    sv.last_withdraw_slot = clock.slot;

    msg!(
        "EmergencyUnwind complete: ctoken_burned={} received={} deployed_amount=0",
        ctoken_balance,
        received,
    );
    Ok(())
}

/// Build and invoke Kamino deposit_reserve_liquidity CPI in its own stack frame.
#[inline(never)]
fn invoke_deposit_reserve_liquidity<'info>(
    klend_program_key: Pubkey,
    keys: DepositReserveLiquidityKeys,
    amount: u64,
    accounts: &[AccountInfo<'info>; 12],
    signer_seeds: &[&[u8]],
) -> Result<()> {
    let ix = klend::build_deposit_reserve_liquidity_ix(klend_program_key, keys, amount);
    invoke_signed(&ix, accounts.as_ref(), &[signer_seeds]).map_err(Into::into)
}

/// Build and invoke Kamino redeem_reserve_collateral CPI in its own stack frame.
#[inline(never)]
fn invoke_redeem_reserve_collateral<'info>(
    klend_program_key: Pubkey,
    keys: RedeemReserveCollateralKeys,
    collateral_amount: u64,
    accounts: &[AccountInfo<'info>; 12],
    signer_seeds: &[&[u8]],
) -> Result<()> {
    let ix = klend::build_redeem_reserve_collateral_ix(klend_program_key, keys, collateral_amount);
    invoke_signed(&ix, accounts.as_ref(), &[signer_seeds]).map_err(Into::into)
}

#[inline(never)]
fn invoke_refresh_reserve<'info>(
    klend_program: Pubkey,
    reserve: &AccountInfo<'info>,
    lending_market: &AccountInfo<'info>,
    pyth_oracle: &AccountInfo<'info>,
    switchboard_price_oracle: &AccountInfo<'info>,
    switchboard_twap_oracle: &AccountInfo<'info>,
    scope_prices: &AccountInfo<'info>,
) -> Result<()> {
    let ix = klend::build_refresh_reserve_ix(
        klend_program,
        RefreshReserveKeys {
            reserve: *reserve.key,
            lending_market: *lending_market.key,
            pyth_oracle: *pyth_oracle.key,
            switchboard_price_oracle: *switchboard_price_oracle.key,
            switchboard_twap_oracle: *switchboard_twap_oracle.key,
            scope_prices: *scope_prices.key,
        },
    );

    invoke(
        &ix,
        &[
            reserve.clone(),
            lending_market.clone(),
            pyth_oracle.clone(),
            switchboard_price_oracle.clone(),
            switchboard_twap_oracle.clone(),
            scope_prices.clone(),
        ],
    )
    .map_err(Into::into)
}

#[inline(never)]
fn load_and_validate_kamino_reserve<'info>(
    strategy_vault: &StrategyVault,
    market_vault_key: Pubkey,
    deposit_mint: &AccountInfo<'info>,
    ctoken_ata: &SplTokenAccount,
    ctoken_ata_info: &AccountInfo<'info>,
    klend_program: &AccountInfo<'info>,
    klend_reserve: &AccountInfo<'info>,
    klend_lending_market: &AccountInfo<'info>,
    klend_lending_market_authority: &AccountInfo<'info>,
    reserve_liquidity_supply: &AccountInfo<'info>,
    reserve_collateral_mint: &AccountInfo<'info>,
    pyth_oracle: &AccountInfo<'info>,
    switchboard_price_oracle: &AccountInfo<'info>,
    switchboard_twap_oracle: &AccountInfo<'info>,
    scope_prices: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
) -> Result<Box<klend::Reserve>> {
    require!(
        klend_program.executable,
        OracleError::InvalidExternalAccount
    );
    require!(
        klend_program.key() == strategy_vault.klend_program,
        OracleError::InvalidExternalAccount
    );
    require!(
        *klend_reserve.owner == strategy_vault.klend_program,
        OracleError::InvalidExternalAccount
    );
    require!(
        *klend_lending_market.owner == strategy_vault.klend_program,
        OracleError::InvalidExternalAccount
    );
    require!(
        token_program.executable,
        OracleError::InvalidExternalAccount
    );

    let reserve_data = klend_reserve.try_borrow_data()?;
    let reserve: Box<klend::Reserve> =
        klend::parse_reserve(&reserve_data).ok_or(OracleError::InvalidExternalState)?;
    drop(reserve_data);

    require!(
        reserve.lending_market == strategy_vault.klend_lending_market,
        OracleError::InvalidExternalAccount
    );
    require!(
        *klend_lending_market.key == reserve.lending_market,
        OracleError::InvalidExternalAccount
    );
    require!(
        *deposit_mint.key == strategy_vault.deposit_mint
            && reserve.liquidity.mint_pubkey == *deposit_mint.key,
        OracleError::InvalidMint
    );
    require!(
        *reserve_liquidity_supply.key == reserve.liquidity.supply_vault,
        OracleError::InvalidExternalAccount
    );
    require!(
        *reserve_collateral_mint.key == reserve.collateral.mint_pubkey,
        OracleError::InvalidExternalAccount
    );
    require!(
        ctoken_ata.mint == reserve.collateral.mint_pubkey,
        OracleError::InvalidMint
    );
    require!(
        ctoken_ata.owner == market_vault_key && *ctoken_ata_info.key == strategy_vault.ctoken_ata,
        OracleError::InvalidExternalAccount
    );
    require!(
        *deposit_mint.owner == *token_program.key
            && *reserve_liquidity_supply.owner == *token_program.key
            && *reserve_collateral_mint.owner == *token_program.key
            && *ctoken_ata_info.owner == *token_program.key
            && reserve.liquidity.token_program == *token_program.key,
        OracleError::InvalidTokenProgram
    );
    require!(
        *pyth_oracle.key == reserve.config.token_info.pyth_configuration.price,
        OracleError::InvalidExternalAccount
    );
    require!(
        *switchboard_price_oracle.key
            == reserve
                .config
                .token_info
                .switchboard_configuration
                .price_aggregator,
        OracleError::InvalidExternalAccount
    );
    require!(
        *switchboard_twap_oracle.key
            == reserve
                .config
                .token_info
                .switchboard_configuration
                .twap_aggregator,
        OracleError::InvalidExternalAccount
    );
    require!(
        *scope_prices.key == reserve.config.token_info.scope_configuration.price_feed,
        OracleError::InvalidExternalAccount
    );
    require!(
        *klend_lending_market_authority.key
            == klend::derive_lending_market_authority(
                &strategy_vault.klend_lending_market,
                &strategy_vault.klend_program,
            ),
        OracleError::InvalidExternalAccount
    );

    Ok(reserve)
}
