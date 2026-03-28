use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use borsh::BorshDeserialize;

pub const LENDING_MARKET_AUTHORITY_SEED: &[u8] = b"lma";
pub const RESERVE_ACCOUNT_DISCM: [u8; 8] = [43, 242, 204, 202, 26, 247, 59, 127];
pub const REFRESH_RESERVE_IX_DISCM: [u8; 8] = [2, 218, 138, 235, 79, 201, 25, 102];
pub const DEPOSIT_RESERVE_LIQUIDITY_IX_DISCM: [u8; 8] = [169, 201, 30, 126, 6, 205, 102, 68];
pub const REDEEM_RESERVE_COLLATERAL_IX_DISCM: [u8; 8] = [234, 117, 181, 125, 185, 142, 220, 29];
pub const KAMINO_FRACTIONAL_BITS: u32 = 60;

#[derive(Clone, Debug, BorshDeserialize, PartialEq)]
pub struct LastUpdate {
    pub slot: u64,
    pub stale: u8,
    pub price_status: u8,
    pub placeholder: [u8; 6],
}

#[derive(Clone, Debug, BorshDeserialize, PartialEq)]
pub struct BigFractionBytes {
    pub value: [u64; 4],
    pub padding: [u64; 2],
}

#[derive(Clone, Debug, BorshDeserialize, PartialEq)]
pub struct ReserveFees {
    pub borrow_fee_sf: u64,
    pub flash_loan_fee_sf: u64,
    pub padding: [u8; 8],
}

#[derive(Clone, Debug, BorshDeserialize, PartialEq)]
pub struct CurvePoint {
    pub utilization_rate_bps: u32,
    pub borrow_rate_bps: u32,
}

#[derive(Clone, Debug, BorshDeserialize, PartialEq)]
pub struct BorrowRateCurve {
    pub points: [CurvePoint; 11],
}

#[derive(Clone, Debug, BorshDeserialize, PartialEq)]
pub struct PriceHeuristic {
    pub lower: u64,
    pub upper: u64,
    pub exp: u64,
}

#[derive(Clone, Debug, BorshDeserialize, PartialEq)]
pub struct ScopeConfiguration {
    pub price_feed: Pubkey,
    pub price_chain: [u16; 4],
    pub twap_chain: [u16; 4],
}

#[derive(Clone, Debug, BorshDeserialize, PartialEq)]
pub struct SwitchboardConfiguration {
    pub price_aggregator: Pubkey,
    pub twap_aggregator: Pubkey,
}

#[derive(Clone, Debug, BorshDeserialize, PartialEq)]
pub struct PythConfiguration {
    pub price: Pubkey,
}

#[derive(Clone, Debug, BorshDeserialize, PartialEq)]
pub struct TokenInfo {
    pub name: [u8; 32],
    pub heuristic: PriceHeuristic,
    pub max_twap_divergence_bps: u64,
    pub max_age_price_seconds: u64,
    pub max_age_twap_seconds: u64,
    pub scope_configuration: ScopeConfiguration,
    pub switchboard_configuration: SwitchboardConfiguration,
    pub pyth_configuration: PythConfiguration,
    pub block_price_usage: u8,
    pub reserved: [u8; 7],
    pub padding: [u64; 19],
}

#[derive(Clone, Debug, BorshDeserialize, PartialEq)]
pub struct WithdrawalCaps {
    pub config_capacity: i64,
    pub current_total: i64,
    pub last_interval_start_timestamp: u64,
    pub config_interval_length_seconds: u64,
}

#[derive(Clone, Debug, BorshDeserialize, PartialEq)]
pub struct ReserveLiquidity {
    pub mint_pubkey: Pubkey,
    pub supply_vault: Pubkey,
    pub fee_vault: Pubkey,
    pub available_amount: u64,
    pub borrowed_amount_sf: u128,
    pub market_price_sf: u128,
    pub market_price_last_updated_ts: u64,
    pub mint_decimals: u64,
    pub deposit_limit_crossed_timestamp: u64,
    pub borrow_limit_crossed_timestamp: u64,
    pub cumulative_borrow_rate_bsf: BigFractionBytes,
    pub accumulated_protocol_fees_sf: u128,
    pub accumulated_referrer_fees_sf: u128,
    pub pending_referrer_fees_sf: u128,
    pub absolute_referral_rate_sf: u128,
    pub token_program: Pubkey,
    pub padding2: [u64; 51],
    pub padding3: [u128; 32],
}

#[derive(Clone, Debug, BorshDeserialize, PartialEq)]
pub struct ReserveCollateral {
    pub mint_pubkey: Pubkey,
    pub mint_total_supply: u64,
    pub supply_vault: Pubkey,
    pub padding1: [u128; 32],
    pub padding2: [u128; 32],
}

#[derive(Clone, Debug, BorshDeserialize, PartialEq)]
pub struct ReserveConfig {
    pub status: u8,
    pub asset_tier: u8,
    pub host_fixed_interest_rate_bps: u16,
    pub reserved2: [u8; 2],
    pub reserved3: [u8; 8],
    pub protocol_take_rate_pct: u8,
    pub protocol_liquidation_fee_pct: u8,
    pub loan_to_value_pct: u8,
    pub liquidation_threshold_pct: u8,
    pub min_liquidation_bonus_bps: u16,
    pub max_liquidation_bonus_bps: u16,
    pub bad_debt_liquidation_bonus_bps: u16,
    pub deleveraging_margin_call_period_secs: u64,
    pub deleveraging_threshold_decrease_bps_per_day: u64,
    pub fees: ReserveFees,
    pub borrow_rate_curve: BorrowRateCurve,
    pub borrow_factor_pct: u64,
    pub deposit_limit: u64,
    pub borrow_limit: u64,
    pub token_info: TokenInfo,
    pub deposit_withdrawal_cap: WithdrawalCaps,
}

#[derive(Clone, Debug, BorshDeserialize, PartialEq)]
pub struct Reserve {
    pub version: u64,
    pub last_update: LastUpdate,
    pub lending_market: Pubkey,
    pub farm_collateral: Pubkey,
    pub farm_debt: Pubkey,
    pub liquidity: ReserveLiquidity,
    pub reserve_liquidity_padding: [u64; 150],
    pub collateral: ReserveCollateral,
    pub reserve_collateral_padding: [u64; 150],
    pub config: ReserveConfig,
}

pub struct RefreshReserveKeys {
    pub reserve: Pubkey,
    pub lending_market: Pubkey,
    pub pyth_oracle: Pubkey,
    pub switchboard_price_oracle: Pubkey,
    pub switchboard_twap_oracle: Pubkey,
    pub scope_prices: Pubkey,
}

pub struct DepositReserveLiquidityKeys {
    pub owner: Pubkey,
    pub reserve: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub reserve_liquidity_mint: Pubkey,
    pub reserve_liquidity_supply: Pubkey,
    pub reserve_collateral_mint: Pubkey,
    pub user_source_liquidity: Pubkey,
    pub user_destination_collateral: Pubkey,
    pub collateral_token_program: Pubkey,
    pub liquidity_token_program: Pubkey,
    pub instruction_sysvar_account: Pubkey,
}

pub struct RedeemReserveCollateralKeys {
    pub owner: Pubkey,
    pub lending_market: Pubkey,
    pub reserve: Pubkey,
    pub lending_market_authority: Pubkey,
    pub reserve_liquidity_mint: Pubkey,
    pub reserve_collateral_mint: Pubkey,
    pub reserve_liquidity_supply: Pubkey,
    pub user_source_collateral: Pubkey,
    pub user_destination_liquidity: Pubkey,
    pub collateral_token_program: Pubkey,
    pub liquidity_token_program: Pubkey,
    pub instruction_sysvar_account: Pubkey,
}

#[inline(never)]
pub fn parse_reserve(data: &[u8]) -> Option<Box<Reserve>> {
    if data.len() < 8 || data[..8] != RESERVE_ACCOUNT_DISCM {
        return None;
    }

    let mut slice = &data[8..];
    Reserve::deserialize(&mut slice).ok().map(Box::new)
}

pub fn derive_lending_market_authority(lending_market: &Pubkey, klend_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[LENDING_MARKET_AUTHORITY_SEED, lending_market.as_ref()],
        klend_program,
    )
    .0
}

pub fn scaled_fraction_floor(value: u128) -> u64 {
    (value >> KAMINO_FRACTIONAL_BITS) as u64 // SAFE: shifted value fits u64 for token amounts
}

pub fn total_liquidity(reserve: &Reserve) -> Option<u64> {
    let available = u128::from(reserve.liquidity.available_amount);
    let borrowed = reserve.liquidity.borrowed_amount_sf >> KAMINO_FRACTIONAL_BITS;
    let protocol_fees = reserve.liquidity.accumulated_protocol_fees_sf >> KAMINO_FRACTIONAL_BITS;
    let referrer_fees = reserve.liquidity.accumulated_referrer_fees_sf >> KAMINO_FRACTIONAL_BITS;
    let pending_referrer_fees =
        reserve.liquidity.pending_referrer_fees_sf >> KAMINO_FRACTIONAL_BITS;
    let total = available
        .checked_add(borrowed)?
        .checked_sub(protocol_fees)?
        .checked_sub(referrer_fees)?
        .checked_sub(pending_referrer_fees)?;
    u64::try_from(total).ok()
}

pub fn preview_deposit_collateral(reserve: &Reserve, liquidity_amount: u64) -> Option<u64> {
    let total = u128::from(total_liquidity(reserve)?);
    if total == 0 {
        return None;
    }

    let collateral_supply = u128::from(reserve.collateral.mint_total_supply);
    if collateral_supply == 0 {
        return Some(liquidity_amount);
    }

    let minted = u128::from(liquidity_amount)
        .checked_mul(collateral_supply)?
        .checked_div(total)?;
    u64::try_from(minted).ok()
}

pub fn collateral_amount_for_liquidity(reserve: &Reserve, liquidity_amount: u64) -> Option<u64> {
    let total = u128::from(total_liquidity(reserve)?);
    let collateral_supply = u128::from(reserve.collateral.mint_total_supply);
    if total == 0 || collateral_supply == 0 {
        return None;
    }

    let numerator = u128::from(liquidity_amount).checked_mul(collateral_supply)?;
    let rounded_up = numerator
        .checked_add(total.checked_sub(1)?)?
        .checked_div(total)?;
    u64::try_from(rounded_up).ok()
}

pub fn remaining_withdrawal_capacity(reserve: &Reserve, now_ts: u64) -> Option<u64> {
    let cap = &reserve.config.deposit_withdrawal_cap;
    if cap.config_capacity <= 0 {
        return None;
    }

    let interval_ended = now_ts
        >= cap
            .last_interval_start_timestamp
            .saturating_add(cap.config_interval_length_seconds);
    let current_total = if interval_ended {
        0
    } else {
        u64::try_from(cap.current_total.max(0)).ok()?
    };
    let capacity = u64::try_from(cap.config_capacity).ok()?;
    Some(capacity.saturating_sub(current_total))
}

pub fn build_refresh_reserve_ix(program_id: Pubkey, keys: RefreshReserveKeys) -> Instruction {
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(keys.reserve, false),
            AccountMeta::new_readonly(keys.lending_market, false),
            AccountMeta::new_readonly(keys.pyth_oracle, false),
            AccountMeta::new_readonly(keys.switchboard_price_oracle, false),
            AccountMeta::new_readonly(keys.switchboard_twap_oracle, false),
            AccountMeta::new_readonly(keys.scope_prices, false),
        ],
        data: REFRESH_RESERVE_IX_DISCM.to_vec(),
    }
}

pub fn build_deposit_reserve_liquidity_ix(
    program_id: Pubkey,
    keys: DepositReserveLiquidityKeys,
    liquidity_amount: u64,
) -> Instruction {
    let mut data = Vec::with_capacity(16);
    data.extend_from_slice(&DEPOSIT_RESERVE_LIQUIDITY_IX_DISCM);
    data.extend_from_slice(&liquidity_amount.to_le_bytes());

    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(keys.owner, true),
            AccountMeta::new(keys.reserve, false),
            AccountMeta::new_readonly(keys.lending_market, false),
            AccountMeta::new_readonly(keys.lending_market_authority, false),
            AccountMeta::new_readonly(keys.reserve_liquidity_mint, false),
            AccountMeta::new(keys.reserve_liquidity_supply, false),
            AccountMeta::new(keys.reserve_collateral_mint, false),
            AccountMeta::new(keys.user_source_liquidity, false),
            AccountMeta::new(keys.user_destination_collateral, false),
            AccountMeta::new_readonly(keys.collateral_token_program, false),
            AccountMeta::new_readonly(keys.liquidity_token_program, false),
            AccountMeta::new_readonly(keys.instruction_sysvar_account, false),
        ],
        data,
    }
}

pub fn build_redeem_reserve_collateral_ix(
    program_id: Pubkey,
    keys: RedeemReserveCollateralKeys,
    collateral_amount: u64,
) -> Instruction {
    let mut data = Vec::with_capacity(16);
    data.extend_from_slice(&REDEEM_RESERVE_COLLATERAL_IX_DISCM);
    data.extend_from_slice(&collateral_amount.to_le_bytes());

    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(keys.owner, true),
            AccountMeta::new_readonly(keys.lending_market, false),
            AccountMeta::new(keys.reserve, false),
            AccountMeta::new_readonly(keys.lending_market_authority, false),
            AccountMeta::new_readonly(keys.reserve_liquidity_mint, false),
            AccountMeta::new(keys.reserve_collateral_mint, false),
            AccountMeta::new(keys.reserve_liquidity_supply, false),
            AccountMeta::new(keys.user_source_collateral, false),
            AccountMeta::new(keys.user_destination_liquidity, false),
            AccountMeta::new_readonly(keys.collateral_token_program, false),
            AccountMeta::new_readonly(keys.liquidity_token_program, false),
            AccountMeta::new_readonly(keys.instruction_sysvar_account, false),
        ],
        data,
    }
}
