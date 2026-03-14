//! Kamino K-Lend CPI helpers for Pinocchio.
//!
//! Manual instruction encoding — no Borsh, no Anchor, no alloc.
//! All instruction data is built on the stack as fixed-size byte arrays.
//!
//! Reserve account parsing uses raw byte offsets to avoid pulling in
//! the full Kamino SDK. Offsets are validated against the deployed
//! Kamino K-Lend program (KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD).

use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Signer},
    program_error::ProgramError,
    pubkey::Pubkey,
};

// ============================================================================
// CONSTANTS
// ============================================================================

/// Kamino lending market authority PDA seed.
pub const LENDING_MARKET_AUTHORITY_SEED: &[u8] = b"lma";

/// Kamino Reserve account discriminator (first 8 bytes).
pub const RESERVE_ACCOUNT_DISCM: [u8; 8] = [43, 242, 204, 202, 26, 247, 59, 127];

/// Kamino refresh_reserve instruction discriminator.
pub const REFRESH_RESERVE_IX_DISCM: [u8; 8] = [2, 218, 138, 235, 79, 201, 25, 102];

/// Kamino deposit_reserve_liquidity instruction discriminator.
pub const DEPOSIT_RESERVE_LIQUIDITY_IX_DISCM: [u8; 8] = [169, 201, 30, 126, 6, 205, 102, 68];

/// Kamino redeem_reserve_collateral instruction discriminator.
pub const REDEEM_RESERVE_COLLATERAL_IX_DISCM: [u8; 8] = [234, 117, 181, 125, 185, 142, 220, 29];

/// Fractional bit precision for Kamino scaled fractions (SF values).
pub const KAMINO_FRACTIONAL_BITS: u32 = 60;

// ============================================================================
// RESERVE ACCOUNT PARSING — raw byte offsets
// ============================================================================
//
// Reserve account layout (8616 bytes total, Borsh-encoded after 8-byte disc):
//
// Offset  Size  Field
// 0       8     discriminator
// 8       8     version (u64)
// 16      8+1+1+6 = 16  last_update (slot u64, stale u8, price_status u8, placeholder [u8;6])
// 32      32    lending_market
// 64      32    farm_collateral
// 96      32    farm_debt
//
// === ReserveLiquidity (starts at 128) ===
// 128     32    mint_pubkey
// 160     32    supply_vault
// 192     32    fee_vault
// 224     8     available_amount (u64)
// 232     16    borrowed_amount_sf (u128)
// 248     16    market_price_sf (u128)
// 264     8     market_price_last_updated_ts (u64)
// 272     8     mint_decimals (u64)
// 280     8     deposit_limit_crossed_timestamp (u64)
// 288     8     borrow_limit_crossed_timestamp (u64)
// 296     48    cumulative_borrow_rate_bsf (BigFractionBytes: [u64;4]+[u64;2])
// 344     16    accumulated_protocol_fees_sf (u128)
// 360     16    accumulated_referrer_fees_sf (u128)
// 376     16    pending_referrer_fees_sf (u128)
// 392     16    absolute_referral_rate_sf (u128)
// 408     32    token_program
// 440     408   padding2 ([u64;51])
// 848     512   padding3 ([u128;32])
//
// === reserve_liquidity_padding (starts at 1360) ===
// 1360    1200  reserve_liquidity_padding ([u64;150])
//
// === ReserveCollateral (starts at 2560) ===
// 2560    32    collateral.mint_pubkey
// 2592    8     collateral.mint_total_supply (u64)
// 2600    32    collateral.supply_vault
// 2632    512   padding1 ([u128;32])
// 3144    512   padding2 ([u128;32])
//
// === reserve_collateral_padding (starts at 3656) ===
// 3656    1200  reserve_collateral_padding ([u64;150])
//
// === ReserveConfig (starts at 4856) ===
// 4856    1     config.status
// ...     (remaining config fields follow)
//
// For oracle addresses in TokenInfo, the offsets relative to ReserveConfig start:
//   config starts at 4856
//   TokenInfo starts at config + 1+1+2+2+8+1+1+1+1+2+2+2+8+8+24+8+8+8 = config + 36 + name(32) + ...
//
// We use direct byte offsets computed from the Anchor `Reserve` struct definition.

// Key byte offsets into the Reserve account (after 8-byte discriminator):
const OFF_VERSION: usize = 8;
const OFF_LAST_UPDATE: usize = 16;
const OFF_LENDING_MARKET: usize = 32;

// ReserveLiquidity offsets
const OFF_LIQ_MINT: usize = 128;
const OFF_LIQ_SUPPLY_VAULT: usize = 160;
const OFF_LIQ_AVAILABLE: usize = 224;
const OFF_LIQ_BORROWED_SF: usize = 232;
const OFF_LIQ_PROTOCOL_FEES_SF: usize = 344;
const OFF_LIQ_REFERRER_FEES_SF: usize = 360;
const OFF_LIQ_PENDING_REFERRER_SF: usize = 376;
const OFF_LIQ_TOKEN_PROGRAM: usize = 408;

// ReserveCollateral offsets
const OFF_COLL_MINT: usize = 2560;
const OFF_COLL_SUPPLY: usize = 2592;

// ReserveConfig offsets
const OFF_CONFIG_STATUS: usize = 4856;

// TokenInfo offsets within ReserveConfig (config starts at 4856):
//   status(1) + asset_tier(1) + host_fixed(2) + reserved2(2) + reserved3(8) +
//   protocol_take(1) + protocol_liq(1) + ltv(1) + liq_threshold(1) + min_liq_bonus(2) +
//   max_liq_bonus(2) + bad_debt_bonus(2) + deleveraging_period(8) + deleveraging_threshold(8) +
//   fees(24) + borrow_rate_curve(88) + borrow_factor(8) + deposit_limit(8) + borrow_limit(8)
//   = 176 bytes to reach TokenInfo
//   TokenInfo: name(32) + heuristic(24) + max_twap(8) + max_age_price(8) + max_age_twap(8) +
//   scope_configuration: price_feed(32) + price_chain(8) + twap_chain(8) = 48
//   switchboard_configuration: price_aggregator(32) + twap_aggregator(32) = 64
//   pyth_configuration: price(32) = 32
const OFF_CONFIG: usize = 4856;
const OFF_TOKEN_INFO: usize = OFF_CONFIG + 176;
// name(32) = OFF_TOKEN_INFO + 0
// heuristic(24) = OFF_TOKEN_INFO + 32
// max_twap_divergence(8) = OFF_TOKEN_INFO + 56
// max_age_price(8) = OFF_TOKEN_INFO + 64
// max_age_twap(8) = OFF_TOKEN_INFO + 72
// ScopeConfiguration starts at OFF_TOKEN_INFO + 80
const OFF_SCOPE_PRICE_FEED: usize = OFF_TOKEN_INFO + 80;
// SwitchboardConfiguration starts at OFF_TOKEN_INFO + 80 + 48 = OFF_TOKEN_INFO + 128
const OFF_SB_PRICE: usize = OFF_TOKEN_INFO + 128;
const OFF_SB_TWAP: usize = OFF_TOKEN_INFO + 160;
// PythConfiguration starts at OFF_TOKEN_INFO + 128 + 64 = OFF_TOKEN_INFO + 192
const OFF_PYTH_PRICE: usize = OFF_TOKEN_INFO + 192;

// DepositWithdrawalCap starts after PythConfiguration + block_price_usage(1) + reserved(7) + padding(152)
// = OFF_TOKEN_INFO + 192 + 32 + 1 + 7 + 152 = OFF_TOKEN_INFO + 384
const OFF_WITHDRAWAL_CAP: usize = OFF_TOKEN_INFO + 384;
// WithdrawalCaps: config_capacity(i64=8) + current_total(i64=8) + last_interval_start(u64=8) + config_interval_length(u64=8) = 32

/// Minimum account data length to parse a Kamino Reserve.
const RESERVE_MIN_LEN: usize = OFF_WITHDRAWAL_CAP + 32;

// ============================================================================
// INLINE HELPERS — read from reserve data
// ============================================================================

#[inline(always)]
fn read_pubkey(data: &[u8], offset: usize) -> Pubkey {
    let mut key = [0u8; 32];
    key.copy_from_slice(&data[offset..offset + 32]);
    key
}

#[inline(always)]
fn read_u8(data: &[u8], offset: usize) -> u8 {
    data[offset]
}

#[inline(always)]
fn read_u64_le(data: &[u8], offset: usize) -> u64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&data[offset..offset + 8]);
    u64::from_le_bytes(buf)
}

#[inline(always)]
fn read_u128_le(data: &[u8], offset: usize) -> u128 {
    let mut buf = [0u8; 16];
    buf.copy_from_slice(&data[offset..offset + 16]);
    u128::from_le_bytes(buf)
}

#[inline(always)]
fn read_i64_le(data: &[u8], offset: usize) -> i64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&data[offset..offset + 8]);
    i64::from_le_bytes(buf)
}

// ============================================================================
// RESERVE DATA ACCESSORS
// ============================================================================

/// Validate discriminator and return a reference to the raw reserve data.
#[inline(never)]
pub fn validate_reserve_disc(data: &[u8]) -> bool {
    data.len() >= RESERVE_MIN_LEN && data[..8] == RESERVE_ACCOUNT_DISCM
}

/// Read the reserve config status byte.
#[inline]
pub fn reserve_config_status(data: &[u8]) -> u8 {
    read_u8(data, OFF_CONFIG_STATUS)
}

/// Read the reserve's lending_market pubkey.
#[inline]
pub fn reserve_lending_market(data: &[u8]) -> Pubkey {
    read_pubkey(data, OFF_LENDING_MARKET)
}

/// Read the reserve liquidity mint pubkey.
#[inline]
pub fn reserve_liq_mint(data: &[u8]) -> Pubkey {
    read_pubkey(data, OFF_LIQ_MINT)
}

/// Read the reserve liquidity supply vault pubkey.
#[inline]
pub fn reserve_liq_supply_vault(data: &[u8]) -> Pubkey {
    read_pubkey(data, OFF_LIQ_SUPPLY_VAULT)
}

/// Read the reserve liquidity token program.
#[inline]
pub fn reserve_liq_token_program(data: &[u8]) -> Pubkey {
    read_pubkey(data, OFF_LIQ_TOKEN_PROGRAM)
}

/// Read the collateral mint pubkey.
#[inline]
pub fn reserve_coll_mint(data: &[u8]) -> Pubkey {
    read_pubkey(data, OFF_COLL_MINT)
}

/// Read the collateral mint total supply.
#[inline]
pub fn reserve_coll_supply(data: &[u8]) -> u64 {
    read_u64_le(data, OFF_COLL_SUPPLY)
}

/// Read the pyth oracle price key from config.
#[inline]
pub fn reserve_pyth_price(data: &[u8]) -> Pubkey {
    read_pubkey(data, OFF_PYTH_PRICE)
}

/// Read the switchboard price aggregator key.
#[inline]
pub fn reserve_sb_price(data: &[u8]) -> Pubkey {
    read_pubkey(data, OFF_SB_PRICE)
}

/// Read the switchboard TWAP aggregator key.
#[inline]
pub fn reserve_sb_twap(data: &[u8]) -> Pubkey {
    read_pubkey(data, OFF_SB_TWAP)
}

/// Read the scope prices feed key.
#[inline]
pub fn reserve_scope_feed(data: &[u8]) -> Pubkey {
    read_pubkey(data, OFF_SCOPE_PRICE_FEED)
}

// ============================================================================
// LIQUIDITY / NAV CALCULATIONS
// ============================================================================

/// Compute total reserve liquidity: available + borrowed - fees.
#[inline(never)]
pub fn total_liquidity(data: &[u8]) -> Option<u64> {
    let available = u128::from(read_u64_le(data, OFF_LIQ_AVAILABLE));
    let borrowed = read_u128_le(data, OFF_LIQ_BORROWED_SF) >> KAMINO_FRACTIONAL_BITS;
    let protocol_fees = read_u128_le(data, OFF_LIQ_PROTOCOL_FEES_SF) >> KAMINO_FRACTIONAL_BITS;
    let referrer_fees = read_u128_le(data, OFF_LIQ_REFERRER_FEES_SF) >> KAMINO_FRACTIONAL_BITS;
    let pending_referrer = read_u128_le(data, OFF_LIQ_PENDING_REFERRER_SF) >> KAMINO_FRACTIONAL_BITS;
    let total = available
        .checked_add(borrowed)?
        .checked_sub(protocol_fees)?
        .checked_sub(referrer_fees)?
        .checked_sub(pending_referrer)?;
    u64::try_from(total).ok()
}

/// Preview how many cTokens a liquidity deposit would mint.
#[inline(never)]
pub fn preview_deposit_collateral(data: &[u8], liquidity_amount: u64) -> Option<u64> {
    let total = u128::from(total_liquidity(data)?);
    if total == 0 {
        return None;
    }
    let coll_supply = u128::from(reserve_coll_supply(data));
    if coll_supply == 0 {
        return Some(liquidity_amount);
    }
    let minted = u128::from(liquidity_amount)
        .checked_mul(coll_supply)?
        .checked_div(total)?;
    u64::try_from(minted).ok()
}

/// Compute how many cTokens are needed to redeem a given liquidity amount (rounds up).
#[inline(never)]
pub fn collateral_amount_for_liquidity(data: &[u8], liquidity_amount: u64) -> Option<u64> {
    let total = u128::from(total_liquidity(data)?);
    let coll_supply = u128::from(reserve_coll_supply(data));
    if total == 0 || coll_supply == 0 {
        return None;
    }
    let numerator = u128::from(liquidity_amount).checked_mul(coll_supply)?;
    let rounded_up = numerator
        .checked_add(total.checked_sub(1)?)?
        .checked_div(total)?;
    u64::try_from(rounded_up).ok()
}

/// Remaining withdrawal capacity for the current interval.
#[inline(never)]
pub fn remaining_withdrawal_capacity(data: &[u8], now_ts: u64) -> Option<u64> {
    let config_capacity = read_i64_le(data, OFF_WITHDRAWAL_CAP);
    if config_capacity <= 0 {
        return None;
    }
    let current_total = read_i64_le(data, OFF_WITHDRAWAL_CAP + 8);
    let last_interval_start = read_u64_le(data, OFF_WITHDRAWAL_CAP + 16);
    let interval_length = read_u64_le(data, OFF_WITHDRAWAL_CAP + 24);

    let interval_ended = now_ts >= last_interval_start.saturating_add(interval_length);
    let current = if interval_ended {
        0u64
    } else {
        u64::try_from(current_total.max(0)).ok()?
    };
    let capacity = u64::try_from(config_capacity).ok()?;
    Some(capacity.saturating_sub(current))
}

/// Derive the Kamino lending market authority PDA.
#[inline(never)]
pub fn derive_lending_market_authority(
    lending_market: &Pubkey,
    klend_program: &Pubkey,
) -> Pubkey {
    pinocchio::pubkey::find_program_address(
        &[LENDING_MARKET_AUTHORITY_SEED, lending_market.as_ref()],
        klend_program,
    )
    .0
}

// ============================================================================
// CPI INSTRUCTION BUILDERS — stack-based, no alloc
// ============================================================================

/// Build and invoke Kamino refresh_reserve CPI.
///
/// Account order: [reserve(W), lending_market(R), pyth(R), sb_price(R), sb_twap(R), scope(R)]
#[inline(never)]
pub fn invoke_refresh_reserve(
    klend_program: &AccountInfo,
    reserve: &AccountInfo,
    lending_market: &AccountInfo,
    pyth_oracle: &AccountInfo,
    switchboard_price: &AccountInfo,
    switchboard_twap: &AccountInfo,
    scope_prices: &AccountInfo,
) -> Result<(), ProgramError> {
    let metas = [
        AccountMeta::writable(reserve.key()),
        AccountMeta::readonly(lending_market.key()),
        AccountMeta::readonly(pyth_oracle.key()),
        AccountMeta::readonly(switchboard_price.key()),
        AccountMeta::readonly(switchboard_twap.key()),
        AccountMeta::readonly(scope_prices.key()),
    ];
    let ix = Instruction {
        program_id: klend_program.key(),
        accounts: &metas,
        data: &REFRESH_RESERVE_IX_DISCM,
    };
    pinocchio::cpi::slice_invoke_signed(
        &ix,
        &[reserve, lending_market, pyth_oracle, switchboard_price, switchboard_twap, scope_prices],
        &[],
    )
}

/// Build and invoke Kamino deposit_reserve_liquidity CPI.
///
/// Account order: [owner(S), reserve(W), lending_market(R), lma(R), liq_mint(R),
///                 liq_supply(W), coll_mint(W), user_source_liq(W), user_dest_coll(W),
///                 coll_token_prog(R), liq_token_prog(R), ix_sysvar(R)]
#[inline(never)]
pub fn invoke_deposit_reserve_liquidity(
    klend_program: &AccountInfo,
    owner: &AccountInfo,        // market_vault PDA
    reserve: &AccountInfo,
    lending_market: &AccountInfo,
    lma: &AccountInfo,          // lending market authority
    liq_mint: &AccountInfo,
    liq_supply: &AccountInfo,
    coll_mint: &AccountInfo,
    user_source_liq: &AccountInfo,
    user_dest_coll: &AccountInfo,
    token_program: &AccountInfo,
    ix_sysvar: &AccountInfo,
    amount: u64,
    signer: &[Signer],
) -> Result<(), ProgramError> {
    let mut data = [0u8; 16];
    data[..8].copy_from_slice(&DEPOSIT_RESERVE_LIQUIDITY_IX_DISCM);
    data[8..16].copy_from_slice(&amount.to_le_bytes());

    let metas = [
        AccountMeta::readonly_signer(owner.key()),
        AccountMeta::writable(reserve.key()),
        AccountMeta::readonly(lending_market.key()),
        AccountMeta::readonly(lma.key()),
        AccountMeta::readonly(liq_mint.key()),
        AccountMeta::writable(liq_supply.key()),
        AccountMeta::writable(coll_mint.key()),
        AccountMeta::writable(user_source_liq.key()),
        AccountMeta::writable(user_dest_coll.key()),
        AccountMeta::readonly(token_program.key()),
        AccountMeta::readonly(token_program.key()),
        AccountMeta::readonly(ix_sysvar.key()),
    ];
    let ix = Instruction {
        program_id: klend_program.key(),
        accounts: &metas,
        data: &data,
    };
    pinocchio::cpi::slice_invoke_signed(
        &ix,
        &[owner, reserve, lending_market, lma, liq_mint, liq_supply,
          coll_mint, user_source_liq, user_dest_coll, token_program,
          token_program, ix_sysvar],
        signer,
    )
}

/// Build and invoke Kamino redeem_reserve_collateral CPI.
///
/// Account order: [owner(S), lending_market(R), reserve(W), lma(R), liq_mint(R),
///                 coll_mint(W), liq_supply(W), user_source_coll(W), user_dest_liq(W),
///                 coll_token_prog(R), liq_token_prog(R), ix_sysvar(R)]
#[inline(never)]
pub fn invoke_redeem_reserve_collateral(
    klend_program: &AccountInfo,
    owner: &AccountInfo,        // market_vault PDA
    lending_market: &AccountInfo,
    reserve: &AccountInfo,
    lma: &AccountInfo,          // lending market authority
    liq_mint: &AccountInfo,
    coll_mint: &AccountInfo,
    liq_supply: &AccountInfo,
    user_source_coll: &AccountInfo,
    user_dest_liq: &AccountInfo,
    token_program: &AccountInfo,
    ix_sysvar: &AccountInfo,
    collateral_amount: u64,
    signer: &[Signer],
) -> Result<(), ProgramError> {
    let mut data = [0u8; 16];
    data[..8].copy_from_slice(&REDEEM_RESERVE_COLLATERAL_IX_DISCM);
    data[8..16].copy_from_slice(&collateral_amount.to_le_bytes());

    let metas = [
        AccountMeta::readonly_signer(owner.key()),
        AccountMeta::readonly(lending_market.key()),
        AccountMeta::writable(reserve.key()),
        AccountMeta::readonly(lma.key()),
        AccountMeta::readonly(liq_mint.key()),
        AccountMeta::writable(coll_mint.key()),
        AccountMeta::writable(liq_supply.key()),
        AccountMeta::writable(user_source_coll.key()),
        AccountMeta::writable(user_dest_liq.key()),
        AccountMeta::readonly(token_program.key()),
        AccountMeta::readonly(token_program.key()),
        AccountMeta::readonly(ix_sysvar.key()),
    ];
    let ix = Instruction {
        program_id: klend_program.key(),
        accounts: &metas,
        data: &data,
    };
    pinocchio::cpi::slice_invoke_signed(
        &ix,
        &[owner, lending_market, reserve, lma, liq_mint, coll_mint,
          liq_supply, user_source_coll, user_dest_liq, token_program,
          token_program, ix_sysvar],
        signer,
    )
}
