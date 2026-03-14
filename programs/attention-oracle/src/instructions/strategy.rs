//! Kamino K-Lend strategy vault instructions (Pinocchio).
//!
//! Wire-compatible with the Anchor program. Same discriminators, same PDA seeds,
//! same account layouts. Every handler validates accounts manually and uses
//! klend CPI helpers for Kamino interactions.
//!
//! Handlers:
//!   initialize_strategy_vault — creates StrategyVault PDA
//!   deploy_to_strategy        — deposit USDC into Kamino reserve
//!   withdraw_from_strategy    — redeem collateral from Kamino
//!   harvest_strategy_yield    — collect yield from Kamino
//!   emergency_unwind          — admin emergency withdrawal

use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

use crate::error::OracleError;
use crate::klend;
use crate::state::{
    MarketVault, ProtocolState, StrategyVault,
    DISC_STRATEGY_VAULT, MARKET_VAULT_SEED, STRATEGY_VAULT_SEED,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const STRATEGY_VAULT_VERSION: u8 = 1;
const STRATEGY_STATUS_ACTIVE: u8 = 0;
const STRATEGY_STATUS_EMERGENCY: u8 = 1;
const MIN_DEPLOY_AMOUNT: u64 = 1_000_000; // 1 USDC (6 decimals)
const KAMINO_STATUS_ACTIVE: u8 = 0;
const BPS_DENOMINATOR: u64 = 10_000;
const ZERO_PUBKEY: Pubkey = [0u8; 32];

/// Sysvar Instructions program ID.
const SYSVAR_INSTRUCTIONS_ID: Pubkey = [
    0x06, 0xa7, 0xd5, 0x17, 0x18, 0x7b, 0xd1, 0x60,
    0x35, 0xfe, 0x6b, 0x0a, 0x62, 0xda, 0x8a, 0x09,
    0x1f, 0x4d, 0x31, 0x07, 0x08, 0xb4, 0xc2, 0xfb,
    0x2f, 0x7e, 0x44, 0x73, 0x00, 0x00, 0x00, 0x00,
];

// ---------------------------------------------------------------------------
// Helpers — inline readers for SPL token accounts
// ---------------------------------------------------------------------------

/// Read the mint pubkey from an SPL token account (offset 0).
#[inline(always)]
fn read_token_mint(data: &[u8]) -> Pubkey {
    let mut key = [0u8; 32];
    key.copy_from_slice(&data[..32]);
    key
}

/// Read the owner pubkey from an SPL token account (offset 32).
#[inline(always)]
fn read_token_owner(data: &[u8]) -> Pubkey {
    let mut key = [0u8; 32];
    key.copy_from_slice(&data[32..64]);
    key
}

/// Read the amount from an SPL token account (offset 64).
#[inline(always)]
fn read_token_amount(data: &[u8]) -> u64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&data[64..72]);
    u64::from_le_bytes(buf)
}

// =============================================================================
// 1. INITIALIZE STRATEGY VAULT
// =============================================================================
//
// Accounts:
//   0. [SIGNER, WRITE] admin_authority
//   1. []               protocol_state
//   2. []               market_vault
//   3. []               deposit_mint
//   4. [WRITE]          strategy_vault (uninitialized, will be created)
//   5. []               system_program
//
// Instruction data (after 8-byte discriminator):
//   [0..2]   reserve_ratio_bps: u16 LE
//   [2..4]   utilization_cap_bps: u16 LE
//   [4..36]  operator_authority: Pubkey
//   [36..68] klend_program: Pubkey
//   [68..100] klend_reserve: Pubkey
//   [100..132] klend_lending_market: Pubkey
//   [132..164] ctoken_ata: Pubkey

pub fn initialize_strategy_vault(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 6 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    if ix_data.len() < 164 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let admin = &accounts[0];
    let protocol_state_acc = &accounts[1];
    let market_vault_acc = &accounts[2];
    let _deposit_mint = &accounts[3];
    let strategy_vault_acc = &accounts[4];
    let _system_program = &accounts[5];

    if !admin.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Parse instruction data
    let reserve_ratio_bps = u16::from_le_bytes([ix_data[0], ix_data[1]]);
    let utilization_cap_bps = u16::from_le_bytes([ix_data[2], ix_data[3]]);
    let mut operator_authority = [0u8; 32];
    operator_authority.copy_from_slice(&ix_data[4..36]);
    let mut klend_program_key = [0u8; 32];
    klend_program_key.copy_from_slice(&ix_data[36..68]);
    let mut klend_reserve_key = [0u8; 32];
    klend_reserve_key.copy_from_slice(&ix_data[68..100]);
    let mut klend_lending_market = [0u8; 32];
    klend_lending_market.copy_from_slice(&ix_data[100..132]);
    let mut ctoken_ata = [0u8; 32];
    ctoken_ata.copy_from_slice(&ix_data[132..164]);

    // Validate BPS inputs
    if reserve_ratio_bps > 10_000 {
        return Err(OracleError::InvalidInputLength.into());
    }
    if utilization_cap_bps > 10_000 {
        return Err(OracleError::InvalidInputLength.into());
    }
    let sum = (reserve_ratio_bps as u32)
        .checked_add(utilization_cap_bps as u32)
        .ok_or(ProgramError::from(OracleError::MathOverflow))?;
    if sum > 10_000 {
        return Err(OracleError::InvalidInputLength.into());
    }

    // Validate pubkeys are non-zero
    if pubkey::pubkey_eq(&operator_authority, &ZERO_PUBKEY) {
        return Err(OracleError::InvalidPubkey.into());
    }
    if pubkey::pubkey_eq(&klend_program_key, &ZERO_PUBKEY) {
        return Err(OracleError::InvalidPubkey.into());
    }
    if pubkey::pubkey_eq(&klend_reserve_key, &ZERO_PUBKEY) {
        return Err(OracleError::InvalidPubkey.into());
    }
    if pubkey::pubkey_eq(&klend_lending_market, &ZERO_PUBKEY) {
        return Err(OracleError::InvalidPubkey.into());
    }
    if pubkey::pubkey_eq(&ctoken_ata, &ZERO_PUBKEY) {
        return Err(OracleError::InvalidPubkey.into());
    }

    // Verify protocol_state PDA and admin auth
    if !protocol_state_acc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let ps = ProtocolState::from_account(protocol_state_acc)?;
    if !pubkey::pubkey_eq(&ps.admin, admin.key()) {
        return Err(OracleError::Unauthorized.into());
    }
    if ps.bump == 0 {
        return Err(ProgramError::InvalidAccountData);
    }
    let ps_pda = pubkey::create_program_address(
        &[b"protocol_state", &[ps.bump]],
        program_id,
    )?;
    if !pubkey::pubkey_eq(&ps_pda, protocol_state_acc.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Verify market_vault PDA
    if !market_vault_acc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let mv = MarketVault::from_account(market_vault_acc)?;
    let mv_pda = pubkey::create_program_address(
        &[
            MARKET_VAULT_SEED,
            protocol_state_acc.key(),
            &mv.market_id,
            &[mv.bump],
        ],
        program_id,
    )?;
    if !pubkey::pubkey_eq(&mv_pda, market_vault_acc.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Verify deposit_mint matches market_vault
    if !pubkey::pubkey_eq(_deposit_mint.key(), &mv.deposit_mint) {
        return Err(OracleError::InvalidMint.into());
    }

    // Derive strategy_vault PDA
    let (expected_sv_pda, sv_bump) = pubkey::find_program_address(
        &[STRATEGY_VAULT_SEED, market_vault_acc.key()],
        program_id,
    );
    if !pubkey::pubkey_eq(&expected_sv_pda, strategy_vault_acc.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Create the strategy_vault account via system program
    let bump_ref = [sv_bump];
    let mv_key_ref = market_vault_acc.key();
    let seeds = [
        Seed::from(STRATEGY_VAULT_SEED),
        Seed::from(mv_key_ref),
        Seed::from(bump_ref.as_ref()),
    ];
    let pda_signer = Signer::from(&seeds);

    let rent = pinocchio::sysvars::rent::Rent::get()?;
    let lamports = rent.minimum_balance(StrategyVault::LEN);

    crate::cpi_create_account(
        admin,
        strategy_vault_acc,
        lamports,
        StrategyVault::LEN as u64,
        program_id,
        &[pda_signer],
    )?;

    // Write initial data
    {
        let sv = StrategyVault::from_account_mut(strategy_vault_acc)?;
        sv.discriminator = DISC_STRATEGY_VAULT;
        sv.version = STRATEGY_VAULT_VERSION;
        sv.bump = sv_bump;
        sv.status = STRATEGY_STATUS_ACTIVE;
        sv.set_reserve_ratio_bps(reserve_ratio_bps);
        sv.set_utilization_cap_bps(utilization_cap_bps);
        sv.protocol_state.copy_from_slice(protocol_state_acc.key());
        sv.market_vault.copy_from_slice(market_vault_acc.key());
        sv.deposit_mint.copy_from_slice(&mv.deposit_mint);
        sv.admin_authority.copy_from_slice(admin.key());
        sv.operator_authority = operator_authority;
        sv.klend_program = klend_program_key;
        sv.klend_reserve = klend_reserve_key;
        sv.klend_lending_market = klend_lending_market;
        sv.ctoken_ata = ctoken_ata;
        sv.set_deployed_amount(0);
        sv.set_pending_withdraw_amount(0);
        sv.set_harvested_yield_amount(0);
        sv.set_last_deploy_slot(0);
        sv.set_last_withdraw_slot(0);
        sv.set_last_harvest_slot(0);
    }

    Ok(())
}

// =============================================================================
// 2. DEPLOY TO STRATEGY (USDC -> Kamino cUSDC)
// =============================================================================
//
// Accounts:
//   0.  [SIGNER, WRITE] operator_authority
//   1.  []               protocol_state
//   2.  []               market_vault
//   3.  [WRITE]          strategy_vault
//   4.  []               deposit_mint
//   5.  [WRITE]          vault_usdc_ata
//   6.  [WRITE]          ctoken_ata
//   7.  []               klend_program
//   8.  [WRITE]          klend_reserve
//   9.  []               klend_lending_market
//  10.  []               klend_lending_market_authority
//  11.  [WRITE]          reserve_liquidity_supply
//  12.  [WRITE]          reserve_collateral_mint
//  13.  []               pyth_oracle
//  14.  []               switchboard_price_oracle
//  15.  []               switchboard_twap_oracle
//  16.  []               scope_prices
//  17.  []               instruction_sysvar_account
//  18.  []               token_program
//
// Instruction data: [0..8] amount: u64 LE

#[inline(never)]
pub fn deploy_to_strategy(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 19 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    if ix_data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let operator = &accounts[0];
    let protocol_state_acc = &accounts[1];
    let market_vault_acc = &accounts[2];
    let strategy_vault_acc = &accounts[3];
    let deposit_mint = &accounts[4];
    let vault_usdc_ata = &accounts[5];
    let ctoken_ata = &accounts[6];
    let klend_program = &accounts[7];
    let klend_reserve = &accounts[8];
    let klend_lending_market = &accounts[9];
    let klend_lma = &accounts[10];
    let reserve_liq_supply = &accounts[11];
    let reserve_coll_mint = &accounts[12];
    let pyth_oracle = &accounts[13];
    let sb_price = &accounts[14];
    let sb_twap = &accounts[15];
    let scope_prices = &accounts[16];
    let ix_sysvar = &accounts[17];
    let token_program = &accounts[18];

    let amount = {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&ix_data[0..8]);
        u64::from_le_bytes(buf)
    };

    if !operator.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if amount < MIN_DEPLOY_AMOUNT {
        return Err(OracleError::StrategyAmountTooSmall.into());
    }

    // Validate strategy vault
    if !strategy_vault_acc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let sv = StrategyVault::from_account(strategy_vault_acc)?;

    // Verify has_one constraints
    if !pubkey::pubkey_eq(&sv.operator_authority, operator.key()) {
        return Err(OracleError::Unauthorized.into());
    }
    if !pubkey::pubkey_eq(&sv.protocol_state, protocol_state_acc.key()) {
        return Err(OracleError::InvalidExternalAccount.into());
    }
    if !pubkey::pubkey_eq(&sv.market_vault, market_vault_acc.key()) {
        return Err(OracleError::InvalidExternalAccount.into());
    }
    if sv.status != STRATEGY_STATUS_ACTIVE {
        return Err(OracleError::StrategyInactive.into());
    }

    // Verify strategy PDA
    let sv_pda = pubkey::create_program_address(
        &[STRATEGY_VAULT_SEED, market_vault_acc.key(), &[sv.bump]],
        program_id,
    )?;
    if !pubkey::pubkey_eq(&sv_pda, strategy_vault_acc.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Verify external account addresses
    verify_kamino_accounts(sv, deposit_mint, klend_program, klend_reserve, klend_lending_market)?;
    if !pubkey::pubkey_eq(ix_sysvar.key(), &SYSVAR_INSTRUCTIONS_ID) {
        return Err(OracleError::InvalidExternalAccount.into());
    }

    // Read vault balance and compute limits
    let reserve_balance = {
        let data = unsafe { vault_usdc_ata.borrow_data_unchecked() };
        if data.len() < 72 { return Err(ProgramError::InvalidAccountData); }
        // Verify owner = market_vault
        let owner = read_token_owner(&data);
        if !pubkey::pubkey_eq(&owner, market_vault_acc.key()) {
            return Err(OracleError::InvalidExternalAccount.into());
        }
        // Verify mint = deposit_mint
        let mint = read_token_mint(&data);
        if !pubkey::pubkey_eq(&mint, &sv.deposit_mint) {
            return Err(OracleError::InvalidMint.into());
        }
        read_token_amount(&data)
    };

    let deployed = sv.get_deployed_amount();
    let total_managed = reserve_balance
        .checked_add(deployed)
        .ok_or(ProgramError::from(OracleError::MathOverflow))?;
    let reserve_floor = total_managed
        .checked_mul(u64::from(sv.get_reserve_ratio_bps()))
        .ok_or(ProgramError::from(OracleError::MathOverflow))?
        .checked_div(BPS_DENOMINATOR)
        .ok_or(ProgramError::from(OracleError::MathOverflow))?;
    let new_deployed = deployed
        .checked_add(amount)
        .ok_or(ProgramError::from(OracleError::MathOverflow))?;
    let max_deployed = total_managed
        .checked_mul(u64::from(sv.get_utilization_cap_bps()))
        .ok_or(ProgramError::from(OracleError::MathOverflow))?
        .checked_div(BPS_DENOMINATOR)
        .ok_or(ProgramError::from(OracleError::MathOverflow))?;

    if reserve_balance.saturating_sub(amount) < reserve_floor {
        return Err(OracleError::InsufficientReserve.into());
    }
    if new_deployed > max_deployed {
        return Err(OracleError::UtilizationCapExceeded.into());
    }

    // Validate Kamino reserve data
    validate_reserve_data(sv, market_vault_acc, deposit_mint, ctoken_ata, klend_program, klend_reserve, klend_lending_market, klend_lma, reserve_liq_supply, reserve_coll_mint, pyth_oracle, sb_price, sb_twap, scope_prices, token_program)?;

    // Preview collateral to verify deposit is meaningful
    {
        let reserve_data = unsafe { klend_reserve.borrow_data_unchecked() };
        let preview = klend::preview_deposit_collateral(&reserve_data, amount)
            .ok_or(ProgramError::from(OracleError::InvalidExternalState))?;
        if preview == 0 {
            return Err(OracleError::StrategyAmountTooSmall.into());
        }
    }

    // Read pre-CPI ctoken balance
    let pre_ctoken = {
        let data = unsafe { ctoken_ata.borrow_data_unchecked() };
        read_token_amount(&data)
    };

    // Refresh reserve
    klend::invoke_refresh_reserve(
        klend_program, klend_reserve, klend_lending_market,
        pyth_oracle, sb_price, sb_twap, scope_prices,
    )?;

    // Build market_vault PDA signer
    let mv = MarketVault::from_account(market_vault_acc)?;
    let mv_bump_ref = [mv.bump];
    let mv_seeds = [
        Seed::from(MARKET_VAULT_SEED),
        Seed::from(protocol_state_acc.key()),
        Seed::from(mv.market_id.as_ref()),
        Seed::from(mv_bump_ref.as_ref()),
    ];
    let mv_signer = Signer::from(&mv_seeds);

    // Deposit into Kamino
    klend::invoke_deposit_reserve_liquidity(
        klend_program, market_vault_acc, klend_reserve, klend_lending_market,
        klend_lma, deposit_mint, reserve_liq_supply, reserve_coll_mint,
        vault_usdc_ata, ctoken_ata, token_program, ix_sysvar,
        amount, &[mv_signer],
    )?;

    // Verify ctoken balance increased
    let post_ctoken = {
        let data = unsafe { ctoken_ata.borrow_data_unchecked() };
        read_token_amount(&data)
    };
    if post_ctoken <= pre_ctoken {
        return Err(OracleError::StrategyAmountTooSmall.into());
    }

    // Update strategy vault state
    let clock = Clock::get()?;
    {
        let sv_mut = StrategyVault::from_account_mut(strategy_vault_acc)?;
        sv_mut.set_deployed_amount(new_deployed);
        sv_mut.set_last_deploy_slot(clock.slot);
    }

    Ok(())
}

// =============================================================================
// 3. WITHDRAW FROM STRATEGY (Kamino cUSDC -> USDC)
// =============================================================================
//
// Same account layout as deploy_to_strategy.
//
// Instruction data: [0..8] amount: u64 LE (liquidity amount to withdraw)

#[inline(never)]
pub fn withdraw_from_strategy(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 19 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    if ix_data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let operator = &accounts[0];
    let protocol_state_acc = &accounts[1];
    let market_vault_acc = &accounts[2];
    let strategy_vault_acc = &accounts[3];
    let deposit_mint = &accounts[4];
    let vault_usdc_ata = &accounts[5];
    let ctoken_ata = &accounts[6];
    let klend_program = &accounts[7];
    let klend_reserve = &accounts[8];
    let klend_lending_market = &accounts[9];
    let klend_lma = &accounts[10];
    let reserve_liq_supply = &accounts[11];
    let reserve_coll_mint = &accounts[12];
    let pyth_oracle = &accounts[13];
    let sb_price = &accounts[14];
    let sb_twap = &accounts[15];
    let scope_prices = &accounts[16];
    let ix_sysvar = &accounts[17];
    let token_program = &accounts[18];

    let amount = {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&ix_data[0..8]);
        u64::from_le_bytes(buf)
    };

    if !operator.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if amount == 0 {
        return Err(OracleError::InvalidInputLength.into());
    }

    // Validate strategy vault
    if !strategy_vault_acc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let sv = StrategyVault::from_account(strategy_vault_acc)?;

    if !pubkey::pubkey_eq(&sv.operator_authority, operator.key()) {
        return Err(OracleError::Unauthorized.into());
    }
    if !pubkey::pubkey_eq(&sv.protocol_state, protocol_state_acc.key()) {
        return Err(OracleError::InvalidExternalAccount.into());
    }
    if !pubkey::pubkey_eq(&sv.market_vault, market_vault_acc.key()) {
        return Err(OracleError::InvalidExternalAccount.into());
    }

    let sv_pda = pubkey::create_program_address(
        &[STRATEGY_VAULT_SEED, market_vault_acc.key(), &[sv.bump]],
        program_id,
    )?;
    if !pubkey::pubkey_eq(&sv_pda, strategy_vault_acc.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    let deployed = sv.get_deployed_amount();
    if deployed == 0 {
        return Err(OracleError::InsufficientStrategyBalance.into());
    }
    if amount > deployed {
        return Err(OracleError::InsufficientStrategyBalance.into());
    }

    // Validate external accounts
    verify_kamino_accounts(sv, deposit_mint, klend_program, klend_reserve, klend_lending_market)?;
    if !pubkey::pubkey_eq(ix_sysvar.key(), &SYSVAR_INSTRUCTIONS_ID) {
        return Err(OracleError::InvalidExternalAccount.into());
    }
    validate_reserve_data(sv, market_vault_acc, deposit_mint, ctoken_ata, klend_program, klend_reserve, klend_lending_market, klend_lma, reserve_liq_supply, reserve_coll_mint, pyth_oracle, sb_price, sb_twap, scope_prices, token_program)?;

    // Check Kamino reserve status
    {
        let reserve_data = unsafe { klend_reserve.borrow_data_unchecked() };
        if klend::reserve_config_status(&reserve_data) != KAMINO_STATUS_ACTIVE {
            return Err(OracleError::InvalidExternalState.into());
        }
    }

    // Check withdrawal capacity
    let now_ts = u64::try_from(Clock::get()?.unix_timestamp)
        .map_err(|_| ProgramError::from(OracleError::InvalidExternalState))?;
    {
        let reserve_data = unsafe { klend_reserve.borrow_data_unchecked() };
        if let Some(remaining) = klend::remaining_withdrawal_capacity(&reserve_data, now_ts) {
            if amount > remaining {
                return Err(OracleError::InvalidExternalState.into());
            }
        }
    }

    // Compute collateral to redeem
    let collateral_amount = {
        let reserve_data = unsafe { klend_reserve.borrow_data_unchecked() };
        klend::collateral_amount_for_liquidity(&reserve_data, amount)
            .ok_or(ProgramError::from(OracleError::InvalidExternalState))?
    };
    if collateral_amount == 0 {
        return Err(OracleError::InvalidExternalState.into());
    }

    // Verify ctoken balance sufficient
    let pre_ctoken = {
        let data = unsafe { ctoken_ata.borrow_data_unchecked() };
        read_token_amount(&data)
    };
    if collateral_amount > pre_ctoken {
        return Err(OracleError::InsufficientStrategyBalance.into());
    }

    let pre_vault = {
        let data = unsafe { vault_usdc_ata.borrow_data_unchecked() };
        read_token_amount(&data)
    };

    // Refresh reserve
    klend::invoke_refresh_reserve(
        klend_program, klend_reserve, klend_lending_market,
        pyth_oracle, sb_price, sb_twap, scope_prices,
    )?;

    // Build market_vault PDA signer
    let mv = MarketVault::from_account(market_vault_acc)?;
    let mv_bump_ref = [mv.bump];
    let mv_seeds = [
        Seed::from(MARKET_VAULT_SEED),
        Seed::from(protocol_state_acc.key()),
        Seed::from(mv.market_id.as_ref()),
        Seed::from(mv_bump_ref.as_ref()),
    ];
    let mv_signer = Signer::from(&mv_seeds);

    // Redeem from Kamino
    klend::invoke_redeem_reserve_collateral(
        klend_program, market_vault_acc, klend_lending_market, klend_reserve,
        klend_lma, deposit_mint, reserve_coll_mint, reserve_liq_supply,
        ctoken_ata, vault_usdc_ata, token_program, ix_sysvar,
        collateral_amount, &[mv_signer],
    )?;

    // Verify received liquidity
    let post_vault = {
        let data = unsafe { vault_usdc_ata.borrow_data_unchecked() };
        read_token_amount(&data)
    };
    let received = post_vault.saturating_sub(pre_vault);
    if received == 0 {
        return Err(OracleError::InvalidExternalState.into());
    }

    // Proportional deployed_amount update
    let post_ctoken = {
        let data = unsafe { ctoken_ata.borrow_data_unchecked() };
        read_token_amount(&data)
    };
    let new_deployed = if pre_ctoken == 0 || post_ctoken == 0 {
        0u64
    } else {
        u64::try_from(
            u128::from(deployed)
                .checked_mul(u128::from(post_ctoken))
                .ok_or(ProgramError::from(OracleError::MathOverflow))?
                .checked_div(u128::from(pre_ctoken))
                .ok_or(ProgramError::from(OracleError::MathOverflow))?,
        )
        .map_err(|_| ProgramError::from(OracleError::MathOverflow))?
    };

    let clock = Clock::get()?;
    let status = sv.status;
    {
        let sv_mut = StrategyVault::from_account_mut(strategy_vault_acc)?;
        sv_mut.set_deployed_amount(new_deployed);
        sv_mut.set_last_withdraw_slot(clock.slot);
        if status == STRATEGY_STATUS_EMERGENCY {
            sv_mut.set_pending_withdraw_amount(new_deployed);
        }
    }

    Ok(())
}

// =============================================================================
// 4. HARVEST STRATEGY YIELD
// =============================================================================
//
// Accounts:
//   0.  [SIGNER, WRITE] operator_authority
//   1.  []               protocol_state
//   2.  []               market_vault
//   3.  [WRITE]          strategy_vault
//   4.  []               deposit_mint
//   5.  []               vault_usdc_ata (read-only, for validation)
//   6.  [WRITE]          treasury_ata
//   7.  [WRITE]          ctoken_ata
//   8.  []               klend_program
//   9.  [WRITE]          klend_reserve
//  10.  []               klend_lending_market
//  11.  []               klend_lending_market_authority
//  12.  [WRITE]          reserve_liquidity_supply
//  13.  [WRITE]          reserve_collateral_mint
//  14.  []               pyth_oracle
//  15.  []               switchboard_price_oracle
//  16.  []               switchboard_twap_oracle
//  17.  []               scope_prices
//  18.  []               instruction_sysvar_account
//  19.  []               token_program
//
// No instruction data (beyond discriminator).

#[inline(never)]
pub fn harvest_strategy_yield(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 20 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let operator = &accounts[0];
    let protocol_state_acc = &accounts[1];
    let market_vault_acc = &accounts[2];
    let strategy_vault_acc = &accounts[3];
    let deposit_mint = &accounts[4];
    let _vault_usdc_ata = &accounts[5];
    let treasury_ata = &accounts[6];
    let ctoken_ata = &accounts[7];
    let klend_program = &accounts[8];
    let klend_reserve = &accounts[9];
    let klend_lending_market = &accounts[10];
    let klend_lma = &accounts[11];
    let reserve_liq_supply = &accounts[12];
    let reserve_coll_mint = &accounts[13];
    let pyth_oracle = &accounts[14];
    let sb_price = &accounts[15];
    let sb_twap = &accounts[16];
    let scope_prices = &accounts[17];
    let ix_sysvar = &accounts[18];
    let token_program = &accounts[19];

    if !operator.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Validate strategy vault
    if !strategy_vault_acc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let sv = StrategyVault::from_account(strategy_vault_acc)?;

    if !pubkey::pubkey_eq(&sv.operator_authority, operator.key()) {
        return Err(OracleError::Unauthorized.into());
    }
    if !pubkey::pubkey_eq(&sv.protocol_state, protocol_state_acc.key()) {
        return Err(OracleError::InvalidExternalAccount.into());
    }
    if !pubkey::pubkey_eq(&sv.market_vault, market_vault_acc.key()) {
        return Err(OracleError::InvalidExternalAccount.into());
    }
    if sv.status != STRATEGY_STATUS_ACTIVE {
        return Err(OracleError::StrategyInactive.into());
    }

    let sv_pda = pubkey::create_program_address(
        &[STRATEGY_VAULT_SEED, market_vault_acc.key(), &[sv.bump]],
        program_id,
    )?;
    if !pubkey::pubkey_eq(&sv_pda, strategy_vault_acc.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    verify_kamino_accounts(sv, deposit_mint, klend_program, klend_reserve, klend_lending_market)?;
    if !pubkey::pubkey_eq(ix_sysvar.key(), &SYSVAR_INSTRUCTIONS_ID) {
        return Err(OracleError::InvalidExternalAccount.into());
    }

    // Verify treasury_ata owned by protocol treasury
    {
        let ps = ProtocolState::from_account(protocol_state_acc)?;
        let ata_data = unsafe { treasury_ata.borrow_data_unchecked() };
        if ata_data.len() < 72 {
            return Err(ProgramError::InvalidAccountData);
        }
        let ata_owner = read_token_owner(&ata_data);
        if !pubkey::pubkey_eq(&ata_owner, &ps.treasury) {
            return Err(OracleError::Unauthorized.into());
        }
        let ata_mint = read_token_mint(&ata_data);
        if !pubkey::pubkey_eq(&ata_mint, &sv.deposit_mint) {
            return Err(OracleError::InvalidMint.into());
        }
    }

    // Read ctoken balance
    let ctoken_balance = {
        let data = unsafe { ctoken_ata.borrow_data_unchecked() };
        read_token_amount(&data)
    };
    if ctoken_balance == 0 {
        // No cTokens, nothing to harvest
        return Ok(());
    }

    // Compute NAV and yield
    let deployed = sv.get_deployed_amount();
    let (yield_usdc, yield_ctoken) = {
        let reserve_data = unsafe { klend_reserve.borrow_data_unchecked() };
        if !klend::validate_reserve_disc(&reserve_data) {
            return Err(OracleError::InvalidExternalState.into());
        }

        let total_liq = klend::total_liquidity(&reserve_data)
            .ok_or(ProgramError::from(OracleError::InvalidExternalState))?;
        let mint_supply = klend::reserve_coll_supply(&reserve_data);

        if total_liq == 0 || mint_supply == 0 {
            return Ok(()); // Reserve empty, skip
        }

        let nav_usdc: u64 = u64::try_from(
            u128::from(ctoken_balance)
                .checked_mul(u128::from(total_liq))
                .ok_or(ProgramError::from(OracleError::MathOverflow))?
                .checked_div(u128::from(mint_supply))
                .ok_or(ProgramError::from(OracleError::MathOverflow))?,
        )
        .map_err(|_| ProgramError::from(OracleError::MathOverflow))?;

        if nav_usdc <= deployed {
            return Ok(()); // No yield
        }
        let y_usdc = nav_usdc.saturating_sub(deployed);

        let y_ctoken = klend::collateral_amount_for_liquidity(&reserve_data, y_usdc)
            .ok_or(ProgramError::from(OracleError::InvalidExternalState))?;
        if y_ctoken == 0 {
            return Err(OracleError::InvalidExternalState.into());
        }
        if y_ctoken > ctoken_balance {
            return Err(OracleError::InsufficientStrategyBalance.into());
        }

        (y_usdc, y_ctoken)
    };
    // Suppress unused warning — yield_usdc is the estimated amount for logging
    let _ = yield_usdc;

    // Refresh reserve
    klend::invoke_refresh_reserve(
        klend_program, klend_reserve, klend_lending_market,
        pyth_oracle, sb_price, sb_twap, scope_prices,
    )?;

    // Build market_vault PDA signer
    let mv = MarketVault::from_account(market_vault_acc)?;
    let mv_bump_ref = [mv.bump];
    let mv_seeds = [
        Seed::from(MARKET_VAULT_SEED),
        Seed::from(protocol_state_acc.key()),
        Seed::from(mv.market_id.as_ref()),
        Seed::from(mv_bump_ref.as_ref()),
    ];
    let mv_signer = Signer::from(&mv_seeds);

    // Read pre-CPI treasury balance
    let pre_treasury = {
        let data = unsafe { treasury_ata.borrow_data_unchecked() };
        read_token_amount(&data)
    };

    // Redeem yield cTokens to treasury_ata
    klend::invoke_redeem_reserve_collateral(
        klend_program, market_vault_acc, klend_lending_market, klend_reserve,
        klend_lma, deposit_mint, reserve_coll_mint, reserve_liq_supply,
        ctoken_ata, treasury_ata, token_program, ix_sysvar,
        yield_ctoken, &[mv_signer],
    )?;

    // Verify received
    let post_treasury = {
        let data = unsafe { treasury_ata.borrow_data_unchecked() };
        read_token_amount(&data)
    };
    let received = post_treasury.saturating_sub(pre_treasury);
    if received == 0 {
        return Err(OracleError::InvalidExternalState.into());
    }

    // Update strategy vault state
    let clock = Clock::get()?;
    {
        let sv_mut = StrategyVault::from_account_mut(strategy_vault_acc)?;
        let prev_harvested = sv_mut.get_harvested_yield_amount();
        sv_mut.set_harvested_yield_amount(prev_harvested.saturating_add(received));
        sv_mut.set_last_harvest_slot(clock.slot);
    }

    Ok(())
}

// =============================================================================
// 5. EMERGENCY UNWIND
// =============================================================================
//
// Accounts (same as deploy/withdraw but admin_authority instead of operator):
//   0.  [SIGNER, WRITE] admin_authority
//   1.  []               protocol_state
//   2.  []               market_vault
//   3.  [WRITE]          strategy_vault
//   4.  []               deposit_mint
//   5.  [WRITE]          vault_usdc_ata
//   6.  [WRITE]          ctoken_ata
//   7.  []               klend_program
//   8.  [WRITE]          klend_reserve
//   9.  []               klend_lending_market
//  10.  []               klend_lending_market_authority
//  11.  [WRITE]          reserve_liquidity_supply
//  12.  [WRITE]          reserve_collateral_mint
//  13.  []               pyth_oracle
//  14.  []               switchboard_price_oracle
//  15.  []               switchboard_twap_oracle
//  16.  []               scope_prices
//  17.  []               instruction_sysvar_account
//  18.  []               token_program
//
// No instruction data (beyond discriminator).

#[inline(never)]
pub fn emergency_unwind(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 19 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let admin = &accounts[0];
    let protocol_state_acc = &accounts[1];
    let market_vault_acc = &accounts[2];
    let strategy_vault_acc = &accounts[3];
    let deposit_mint = &accounts[4];
    let vault_usdc_ata = &accounts[5];
    let ctoken_ata = &accounts[6];
    let klend_program = &accounts[7];
    let klend_reserve = &accounts[8];
    let klend_lending_market = &accounts[9];
    let klend_lma = &accounts[10];
    let reserve_liq_supply = &accounts[11];
    let reserve_coll_mint = &accounts[12];
    let pyth_oracle = &accounts[13];
    let sb_price = &accounts[14];
    let sb_twap = &accounts[15];
    let scope_prices = &accounts[16];
    let ix_sysvar = &accounts[17];
    let token_program = &accounts[18];

    if !admin.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Verify admin via protocol_state
    if !protocol_state_acc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let ps = ProtocolState::from_account(protocol_state_acc)?;
    if !pubkey::pubkey_eq(&ps.admin, admin.key()) {
        return Err(OracleError::Unauthorized.into());
    }

    // Validate strategy vault
    if !strategy_vault_acc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let sv = StrategyVault::from_account(strategy_vault_acc)?;

    if !pubkey::pubkey_eq(&sv.admin_authority, admin.key()) {
        return Err(OracleError::Unauthorized.into());
    }
    if !pubkey::pubkey_eq(&sv.protocol_state, protocol_state_acc.key()) {
        return Err(OracleError::InvalidExternalAccount.into());
    }
    if !pubkey::pubkey_eq(&sv.market_vault, market_vault_acc.key()) {
        return Err(OracleError::InvalidExternalAccount.into());
    }

    let sv_pda = pubkey::create_program_address(
        &[STRATEGY_VAULT_SEED, market_vault_acc.key(), &[sv.bump]],
        program_id,
    )?;
    if !pubkey::pubkey_eq(&sv_pda, strategy_vault_acc.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Read ctoken balance
    let ctoken_balance = {
        let data = unsafe { ctoken_ata.borrow_data_unchecked() };
        read_token_amount(&data)
    };

    // If nothing deployed, just set emergency flag
    if ctoken_balance == 0 {
        let clock = Clock::get()?;
        let sv_mut = StrategyVault::from_account_mut(strategy_vault_acc)?;
        sv_mut.status = STRATEGY_STATUS_EMERGENCY;
        sv_mut.set_pending_withdraw_amount(0);
        sv_mut.set_last_withdraw_slot(clock.slot);
        return Ok(());
    }

    // Validate external accounts
    verify_kamino_accounts(sv, deposit_mint, klend_program, klend_reserve, klend_lending_market)?;
    if !pubkey::pubkey_eq(ix_sysvar.key(), &SYSVAR_INSTRUCTIONS_ID) {
        return Err(OracleError::InvalidExternalAccount.into());
    }

    // Refresh reserve
    klend::invoke_refresh_reserve(
        klend_program, klend_reserve, klend_lending_market,
        pyth_oracle, sb_price, sb_twap, scope_prices,
    )?;

    // Read pre-CPI vault balance
    let pre_vault = {
        let data = unsafe { vault_usdc_ata.borrow_data_unchecked() };
        read_token_amount(&data)
    };

    // Build market_vault PDA signer
    let mv = MarketVault::from_account(market_vault_acc)?;
    let mv_bump_ref = [mv.bump];
    let mv_seeds = [
        Seed::from(MARKET_VAULT_SEED),
        Seed::from(protocol_state_acc.key()),
        Seed::from(mv.market_id.as_ref()),
        Seed::from(mv_bump_ref.as_ref()),
    ];
    let mv_signer = Signer::from(&mv_seeds);

    // Redeem ALL cTokens back to vault_usdc_ata
    klend::invoke_redeem_reserve_collateral(
        klend_program, market_vault_acc, klend_lending_market, klend_reserve,
        klend_lma, deposit_mint, reserve_coll_mint, reserve_liq_supply,
        ctoken_ata, vault_usdc_ata, token_program, ix_sysvar,
        ctoken_balance, &[mv_signer],
    )?;

    // Verify received
    let post_vault = {
        let data = unsafe { vault_usdc_ata.borrow_data_unchecked() };
        read_token_amount(&data)
    };
    let received = post_vault.saturating_sub(pre_vault);
    if received == 0 {
        return Err(OracleError::InvalidExternalState.into());
    }

    // Update state: emergency, zero deployed
    let clock = Clock::get()?;
    {
        let sv_mut = StrategyVault::from_account_mut(strategy_vault_acc)?;
        sv_mut.status = STRATEGY_STATUS_EMERGENCY;
        sv_mut.set_deployed_amount(0);
        sv_mut.set_pending_withdraw_amount(0);
        sv_mut.set_last_withdraw_slot(clock.slot);
    }

    Ok(())
}

// =============================================================================
// SHARED VALIDATION HELPERS
// =============================================================================

/// Verify pinned Kamino account addresses match strategy vault config.
#[inline(never)]
fn verify_kamino_accounts(
    sv: &StrategyVault,
    deposit_mint: &AccountInfo,
    klend_program: &AccountInfo,
    klend_reserve: &AccountInfo,
    klend_lending_market: &AccountInfo,
) -> Result<(), ProgramError> {
    if !pubkey::pubkey_eq(deposit_mint.key(), &sv.deposit_mint) {
        return Err(OracleError::InvalidMint.into());
    }
    if !pubkey::pubkey_eq(klend_program.key(), &sv.klend_program) {
        return Err(OracleError::InvalidExternalAccount.into());
    }
    if !klend_program.executable() {
        return Err(OracleError::InvalidExternalAccount.into());
    }
    if !pubkey::pubkey_eq(klend_reserve.key(), &sv.klend_reserve) {
        return Err(OracleError::InvalidExternalAccount.into());
    }
    if !klend_reserve.is_owned_by(&sv.klend_program) {
        return Err(OracleError::InvalidExternalAccount.into());
    }
    if !pubkey::pubkey_eq(klend_lending_market.key(), &sv.klend_lending_market) {
        return Err(OracleError::InvalidExternalAccount.into());
    }
    if !klend_lending_market.is_owned_by(&sv.klend_program) {
        return Err(OracleError::InvalidExternalAccount.into());
    }
    Ok(())
}

/// Validate Kamino reserve data against strategy vault config and passed accounts.
#[inline(never)]
fn validate_reserve_data(
    sv: &StrategyVault,
    market_vault_acc: &AccountInfo,
    deposit_mint: &AccountInfo,
    ctoken_ata: &AccountInfo,
    klend_program: &AccountInfo,
    klend_reserve: &AccountInfo,
    klend_lending_market: &AccountInfo,
    klend_lma: &AccountInfo,
    reserve_liq_supply: &AccountInfo,
    reserve_coll_mint: &AccountInfo,
    pyth_oracle: &AccountInfo,
    sb_price: &AccountInfo,
    sb_twap: &AccountInfo,
    scope_prices: &AccountInfo,
    token_program: &AccountInfo,
) -> Result<(), ProgramError> {
    let reserve_data = unsafe { klend_reserve.borrow_data_unchecked() };
    if !klend::validate_reserve_disc(&reserve_data) {
        return Err(OracleError::InvalidExternalState.into());
    }

    // Verify reserve config status is active
    if klend::reserve_config_status(&reserve_data) != KAMINO_STATUS_ACTIVE {
        return Err(OracleError::InvalidExternalState.into());
    }

    // Verify reserve lending market matches
    let r_lm = klend::reserve_lending_market(&reserve_data);
    if !pubkey::pubkey_eq(&r_lm, &sv.klend_lending_market) {
        return Err(OracleError::InvalidExternalAccount.into());
    }
    if !pubkey::pubkey_eq(klend_lending_market.key(), &r_lm) {
        return Err(OracleError::InvalidExternalAccount.into());
    }

    // Verify deposit mint matches reserve liquidity mint
    let r_liq_mint = klend::reserve_liq_mint(&reserve_data);
    if !pubkey::pubkey_eq(deposit_mint.key(), &sv.deposit_mint) || !pubkey::pubkey_eq(&r_liq_mint, deposit_mint.key()) {
        return Err(OracleError::InvalidMint.into());
    }

    // Verify reserve liquidity supply
    let r_liq_supply = klend::reserve_liq_supply_vault(&reserve_data);
    if !pubkey::pubkey_eq(reserve_liq_supply.key(), &r_liq_supply) {
        return Err(OracleError::InvalidExternalAccount.into());
    }

    // Verify collateral mint
    let r_coll_mint = klend::reserve_coll_mint(&reserve_data);
    if !pubkey::pubkey_eq(reserve_coll_mint.key(), &r_coll_mint) {
        return Err(OracleError::InvalidExternalAccount.into());
    }

    // Verify ctoken_ata
    {
        let ct_data = unsafe { ctoken_ata.borrow_data_unchecked() };
        if ct_data.len() < 72 {
            return Err(ProgramError::InvalidAccountData);
        }
        let ct_mint = read_token_mint(&ct_data);
        if !pubkey::pubkey_eq(&ct_mint, &r_coll_mint) {
            return Err(OracleError::InvalidMint.into());
        }
        let ct_owner = read_token_owner(&ct_data);
        if !pubkey::pubkey_eq(&ct_owner, market_vault_acc.key()) {
            return Err(OracleError::InvalidExternalAccount.into());
        }
        if !pubkey::pubkey_eq(ctoken_ata.key(), &sv.ctoken_ata) {
            return Err(OracleError::InvalidExternalAccount.into());
        }
    }

    // Verify token program ownership
    let r_token_prog = klend::reserve_liq_token_program(&reserve_data);
    if !pubkey::pubkey_eq(token_program.key(), &r_token_prog) {
        return Err(OracleError::InvalidTokenProgram.into());
    }
    if !token_program.executable() {
        return Err(OracleError::InvalidExternalAccount.into());
    }

    // Verify oracle accounts from reserve config
    let r_pyth = klend::reserve_pyth_price(&reserve_data);
    if !pubkey::pubkey_eq(pyth_oracle.key(), &r_pyth) {
        return Err(OracleError::InvalidExternalAccount.into());
    }
    let r_sb_price = klend::reserve_sb_price(&reserve_data);
    if !pubkey::pubkey_eq(sb_price.key(), &r_sb_price) {
        return Err(OracleError::InvalidExternalAccount.into());
    }
    let r_sb_twap = klend::reserve_sb_twap(&reserve_data);
    if !pubkey::pubkey_eq(sb_twap.key(), &r_sb_twap) {
        return Err(OracleError::InvalidExternalAccount.into());
    }
    let r_scope = klend::reserve_scope_feed(&reserve_data);
    if !pubkey::pubkey_eq(scope_prices.key(), &r_scope) {
        return Err(OracleError::InvalidExternalAccount.into());
    }

    // Verify lending market authority PDA
    let expected_lma = klend::derive_lending_market_authority(
        &sv.klend_lending_market,
        klend_program.key(),
    );
    if !pubkey::pubkey_eq(klend_lma.key(), &expected_lma) {
        return Err(OracleError::InvalidExternalAccount.into());
    }

    Ok(())
}
