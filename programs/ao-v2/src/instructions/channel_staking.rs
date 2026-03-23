//! Channel staking instructions (feature-gated behind `channel_staking`):
//! initialize_fee_config, initialize_stake_pool,
//! stake_channel, unstake_channel, claim_channel_rewards.
//!
//! Note: create_channel_config_v2 lives in admin.rs.
//!
//! These are CPI targets called by channel-vault compound().
//! Account order MUST match the Anchor #[derive(Accounts)] structs exactly.
//! All account layouts are byte-compatible with the Anchor program.

use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Seed, Signer},
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    ProgramResult,
};

// =============================================================================
// CONSTANTS (mirrors programs/attention-oracle/src/constants.rs)
// =============================================================================

const PROTOCOL_SEED: &[u8] = b"protocol";
const CHANNEL_STAKE_POOL_SEED: &[u8] = b"channel_pool";
const CHANNEL_USER_STAKE_SEED: &[u8] = b"channel_user";
const STAKE_NFT_MINT_SEED: &[u8] = b"stake_nft";
const STAKE_VAULT_SEED: &[u8] = b"stake_vault";

const MIN_STAKE_AMOUNT: u64 = 1_000_000_000;
const MAX_LOCK_SLOTS: u64 = 432_000 * 365;
const BOOST_PRECISION: u64 = 10_000;
const SLOTS_PER_DAY: u64 = 216_000;
const REWARD_PRECISION: u128 = 1_000_000_000_000;

// =============================================================================
// ACCOUNT SIZES (including 8-byte Anchor discriminator)
// =============================================================================

const PROTOCOL_STATE_LEN: usize = 173;
const FEE_CONFIG_LEN: usize = 55;
const CHANNEL_CONFIG_V2_LEN: usize = 482;
const CHANNEL_STAKE_POOL_LEN: usize = 162;
const USER_CHANNEL_STAKE_LEN: usize = 161;

// =============================================================================
// BYTE OFFSETS — ProtocolState
// =============================================================================

const PS_ADMIN: usize = 10;
const PS_PUBLISHER: usize = 42;
const PS_MINT: usize = 138;
const PS_PAUSED: usize = 170;
const PS_BUMP: usize = 172;

// =============================================================================
// BYTE OFFSETS — FeeConfig
// =============================================================================

const FC_BASIS_POINTS: usize = 8;
const FC_MAX_FEE: usize = 10;
const FC_DRIP_THRESHOLD: usize = 18;
const FC_TREASURY_FEE_BPS: usize = 26;
const FC_CREATOR_FEE_BPS: usize = 28;
const FC_TIER_MULTIPLIERS: usize = 30;
const FC_BUMP: usize = 54;

// =============================================================================
// BYTE OFFSETS — ChannelConfigV2
// =============================================================================

const CC_MINT: usize = 10;

// =============================================================================
// BYTE OFFSETS — ChannelStakePool
// =============================================================================

const SP_BUMP: usize = 8;
const SP_CHANNEL: usize = 9;
const SP_MINT: usize = 41;
const SP_VAULT: usize = 73;
const SP_TOTAL_STAKED: usize = 105;
const SP_TOTAL_WEIGHTED: usize = 113;
const SP_STAKER_COUNT: usize = 121;
const SP_ACC_REWARD_PER_SHARE: usize = 129;
const SP_LAST_REWARD_SLOT: usize = 145;
const SP_REWARD_PER_SLOT: usize = 153;
const SP_IS_SHUTDOWN: usize = 161;

// =============================================================================
// BYTE OFFSETS — UserChannelStake
// =============================================================================

const US_BUMP: usize = 8;
const US_USER: usize = 9;
const US_CHANNEL: usize = 41;
const US_AMOUNT: usize = 73;
const US_START_SLOT: usize = 81;
const US_LOCK_END_SLOT: usize = 89;
const US_MULTIPLIER_BPS: usize = 97;
const US_NFT_MINT: usize = 105;
const US_REWARD_DEBT: usize = 137;
const US_PENDING_REWARDS: usize = 153;

// =============================================================================
// ANCHOR DISCRIMINATORS
// To verify: echo -n "account:FeeConfig" | openssl dgst -sha256 -binary | xxd -p -l 8
// =============================================================================

/// Pre-computed Anchor account discriminators: SHA-256("account:<Name>")[..8].
const FEE_CONFIG_DISC: [u8; 8] = [0x8f, 0x34, 0x92, 0xbb, 0xdb, 0x7b, 0x4c, 0x9b];
const CHANNEL_STAKE_POOL_DISC: [u8; 8] = [0x1a, 0x17, 0xde, 0x25, 0x4a, 0xa6, 0x02, 0xfe];
const USER_CHANNEL_STAKE_DISC: [u8; 8] = [0x31, 0x40, 0xc1, 0xb3, 0x6b, 0xa2, 0xad, 0x3b];

// =============================================================================
// WELL-KNOWN PROGRAM IDS
// =============================================================================

use crate::TOKEN_2022_ID as TOKEN_2022_PROGRAM_ID;

const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey = [
    0x8c, 0x97, 0x25, 0x8f, 0x4e, 0x24, 0x89, 0xf1, 0xbb, 0x3d, 0x10, 0x29, 0x14, 0x8e, 0x0d, 0x83,
    0x0b, 0x5a, 0x13, 0x99, 0xda, 0xff, 0x10, 0x84, 0x04, 0x8e, 0x7b, 0xd8, 0xdb, 0xe9, 0xf8, 0x59,
];

// =============================================================================
// ERROR CODES (Anchor: 6000 + variant index from OracleError)
// =============================================================================

const ERR_UNAUTHORIZED: u32 = 6000;
const ERR_PROTOCOL_PAUSED: u32 = 6002;
const ERR_INVALID_MINT: u32 = 6016;
const ERR_INVALID_TOKEN_PROGRAM: u32 = 6019;
const ERR_STAKE_BELOW_MINIMUM: u32 = 6024;
const ERR_LOCK_PERIOD_TOO_LONG: u32 = 6025;
const ERR_LOCK_NOT_EXPIRED: u32 = 6033;
const ERR_INVALID_INPUT_LENGTH: u32 = 6040;
const ERR_MATH_OVERFLOW: u32 = 6041;
const ERR_NO_REWARDS_TO_CLAIM: u32 = 6042;
const ERR_POOL_IS_SHUTDOWN: u32 = 6045;
const ERR_PENDING_REWARDS_ON_UNSTAKE: u32 = 6046;
const ERR_CLAIM_EXCEEDS_AVAILABLE: u32 = 6047;

// =============================================================================
// BYTE READ/WRITE HELPERS
// =============================================================================

#[inline(always)]
fn read_u8(d: &[u8], o: usize) -> u8 {
    d[o]
}
#[inline(always)]
fn read_u16_le(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([d[o], d[o + 1]])
}
#[inline(always)]
fn read_u32_le(d: &[u8], o: usize) -> u32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&d[o..o + 4]);
    u32::from_le_bytes(b)
}
#[inline(always)]
fn read_u64_le(d: &[u8], o: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&d[o..o + 8]);
    u64::from_le_bytes(b)
}
#[inline(always)]
fn read_u128_le(d: &[u8], o: usize) -> u128 {
    let mut b = [0u8; 16];
    b.copy_from_slice(&d[o..o + 16]);
    u128::from_le_bytes(b)
}
#[inline(always)]
fn read_pubkey(d: &[u8], o: usize) -> Pubkey {
    let mut k = [0u8; 32];
    k.copy_from_slice(&d[o..o + 32]);
    k
}
#[inline(always)]
fn write_u8(d: &mut [u8], o: usize, v: u8) {
    d[o] = v;
}
#[inline(always)]
fn write_u16_le(d: &mut [u8], o: usize, v: u16) {
    d[o..o + 2].copy_from_slice(&v.to_le_bytes());
}
#[inline(always)]
fn write_u32_le(d: &mut [u8], o: usize, v: u32) {
    d[o..o + 4].copy_from_slice(&v.to_le_bytes());
}
#[inline(always)]
fn write_u64_le(d: &mut [u8], o: usize, v: u64) {
    d[o..o + 8].copy_from_slice(&v.to_le_bytes());
}
#[inline(always)]
fn write_u128_le(d: &mut [u8], o: usize, v: u128) {
    d[o..o + 16].copy_from_slice(&v.to_le_bytes());
}
#[inline(always)]
fn write_pubkey(d: &mut [u8], o: usize, k: &Pubkey) {
    d[o..o + 32].copy_from_slice(k);
}

// =============================================================================
// TOKEN ACCOUNT HELPERS (SPL layout: mint@0, owner@32, amount@64)
// =============================================================================

#[inline(always)]
fn token_amount(ai: &AccountInfo) -> Result<u64, ProgramError> {
    let d = unsafe { ai.borrow_data_unchecked() };
    if d.len() < 72 {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(read_u64_le(&d, 64))
}
#[inline(always)]
fn token_owner_key(ai: &AccountInfo) -> Result<Pubkey, ProgramError> {
    let d = unsafe { ai.borrow_data_unchecked() };
    if d.len() < 64 {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(read_pubkey(&d, 32))
}
#[inline(always)]
fn token_mint_key(ai: &AccountInfo) -> Result<Pubkey, ProgramError> {
    let d = unsafe { ai.borrow_data_unchecked() };
    if d.len() < 32 {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(read_pubkey(&d, 0))
}
#[inline(always)]
fn mint_decimals(ai: &AccountInfo) -> Result<u8, ProgramError> {
    let d = unsafe { ai.borrow_data_unchecked() };
    if d.len() < 45 {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(d[44])
}

// =============================================================================
// SHARED VALIDATION HELPERS
// =============================================================================

/// Verify legacy protocol_state PDA (seeds = ["protocol", mint]) and check admin.
/// Returns the CCM mint pubkey.
#[inline(never)]
fn verify_legacy_protocol_admin(
    admin: &AccountInfo,
    protocol_state: &AccountInfo,
    mint: &AccountInfo,
    program_id: &Pubkey,
) -> Result<Pubkey, ProgramError> {
    let d = unsafe { protocol_state.borrow_data_unchecked() };
    if d.len() < PROTOCOL_STATE_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    if admin.key() != &read_pubkey(&d, PS_ADMIN) {
        return Err(ProgramError::Custom(ERR_UNAUTHORIZED));
    }
    let bump = read_u8(&d, PS_BUMP);
    let pda =
        pubkey::create_program_address(&[PROTOCOL_SEED, mint.key().as_ref(), &[bump]], program_id)?;
    if !pubkey::pubkey_eq(&pda, protocol_state.key()) {
        return Err(ProgramError::InvalidSeeds);
    }
    Ok(read_pubkey(&d, PS_MINT))
}

/// Verify live protocol_state PDA (seeds = ["protocol_state"]).
/// Returns (mint, paused, bump).
#[inline(never)]
fn verify_live_protocol(
    signer: &AccountInfo,
    protocol_state: &AccountInfo,
    program_id: &Pubkey,
    require_admin_or_publisher: bool,
) -> Result<(Pubkey, bool), ProgramError> {
    let d = unsafe { protocol_state.borrow_data_unchecked() };
    if d.len() < PROTOCOL_STATE_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    if require_admin_or_publisher {
        let a = read_pubkey(&d, PS_ADMIN);
        let p = read_pubkey(&d, PS_PUBLISHER);
        if signer.key() != &a && signer.key() != &p {
            return Err(ProgramError::Custom(ERR_UNAUTHORIZED));
        }
    }
    let paused = d[PS_PAUSED] != 0;
    let bump = read_u8(&d, PS_BUMP);
    let m = read_pubkey(&d, PS_MINT);
    let pda = pubkey::create_program_address(&[b"protocol_state", &[bump]], program_id)?;
    if !pubkey::pubkey_eq(&pda, protocol_state.key()) {
        return Err(ProgramError::InvalidSeeds);
    }
    Ok((m, paused))
}

/// Verify stake_pool PDA and return bump. Also validates vault and optionally mint.
#[inline(never)]
fn verify_stake_pool(
    stake_pool: &AccountInfo,
    channel_config: &AccountInfo,
    vault: &AccountInfo,
    mint: Option<&AccountInfo>,
    program_id: &Pubkey,
) -> Result<u8, ProgramError> {
    let d = unsafe { stake_pool.borrow_data_unchecked() };
    if d.len() < CHANNEL_STAKE_POOL_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    if vault.key() != &read_pubkey(&d, SP_VAULT) {
        return Err(ProgramError::InvalidAccountData);
    }
    if let Some(m) = mint {
        if m.key() != &read_pubkey(&d, SP_MINT) {
            return Err(ProgramError::Custom(ERR_INVALID_MINT));
        }
    }
    let b = read_u8(&d, SP_BUMP);
    let pda = pubkey::create_program_address(
        &[CHANNEL_STAKE_POOL_SEED, channel_config.key().as_ref(), &[b]],
        program_id,
    )?;
    if !pubkey::pubkey_eq(&pda, stake_pool.key()) {
        return Err(ProgramError::InvalidSeeds);
    }
    Ok(b)
}

// =============================================================================
// BOOST CALCULATOR
// =============================================================================

fn calculate_boost_bps(lock_duration: u64) -> u64 {
    match lock_duration / SLOTS_PER_DAY {
        0..=6 => 10_000,
        7..=29 => 12_500,
        30..=89 => 15_000,
        90..=179 => 20_000,
        180..=364 => 25_000,
        _ => 30_000,
    }
}

// =============================================================================
// MASTERCHEF REWARD MATH
// =============================================================================

#[inline(never)]
fn update_pool_rewards(pool: &mut [u8], slot: u64) -> Result<(), ProgramError> {
    let last = read_u64_le(pool, SP_LAST_REWARD_SLOT);
    let tw = read_u64_le(pool, SP_TOTAL_WEIGHTED);
    if slot <= last || tw == 0 {
        write_u64_le(pool, SP_LAST_REWARD_SLOT, slot);
        return Ok(());
    }
    let elapsed = slot
        .checked_sub(last)
        .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?;
    let rps = read_u64_le(pool, SP_REWARD_PER_SLOT);
    let accrued = (rps as u128)
        .checked_mul(elapsed as u128)
        .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?;
    let inc = accrued
        .checked_mul(REWARD_PRECISION)
        .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?
        .checked_div(tw as u128)
        .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?;
    let acc = read_u128_le(pool, SP_ACC_REWARD_PER_SHARE);
    write_u128_le(
        pool,
        SP_ACC_REWARD_PER_SHARE,
        acc.checked_add(inc)
            .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?,
    );
    write_u64_le(pool, SP_LAST_REWARD_SLOT, slot);
    Ok(())
}

#[inline(never)]
fn pending_rewards(us: &[u8], sp: &[u8]) -> Result<u64, ProgramError> {
    let amt = read_u64_le(us, US_AMOUNT);
    let mul = read_u64_le(us, US_MULTIPLIER_BPS);
    let debt = read_u128_le(us, US_REWARD_DEBT);
    let pend = read_u64_le(us, US_PENDING_REWARDS);
    let acc = read_u128_le(sp, SP_ACC_REWARD_PER_SHARE);
    let w = (amt as u128)
        .checked_mul(mul as u128)
        .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?
        .checked_div(BOOST_PRECISION as u128)
        .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?;
    let total = w
        .checked_mul(acc)
        .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?
        .checked_div(REWARD_PRECISION)
        .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?;
    let p = total
        .checked_sub(debt)
        .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?
        .checked_add(pend as u128)
        .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?;
    u64::try_from(p).map_err(|_| ProgramError::Custom(ERR_MATH_OVERFLOW))
}

#[inline(never)]
fn reward_debt(amt: u64, mul: u64, acc: u128) -> Result<u128, ProgramError> {
    let w = (amt as u128)
        .checked_mul(mul as u128)
        .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?
        .checked_div(BOOST_PRECISION as u128)
        .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?;
    w.checked_mul(acc)
        .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?
        .checked_div(REWARD_PRECISION)
        .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))
}

// =============================================================================
// TOKEN CPI HELPERS
// =============================================================================

#[inline(never)]
fn cpi_transfer_checked(
    tp: &AccountInfo,
    src: &AccountInfo,
    mint: &AccountInfo,
    dst: &AccountInfo,
    auth: &AccountInfo,
    amount: u64,
    decimals: u8,
    signers: &[Signer],
) -> ProgramResult {
    let mut data = [0u8; 10];
    data[0] = 12; // TransferChecked
    data[1..9].copy_from_slice(&amount.to_le_bytes());
    data[9] = decimals;
    let m = [
        AccountMeta::writable(src.key()),
        AccountMeta::readonly(mint.key()),
        AccountMeta::writable(dst.key()),
        AccountMeta::readonly_signer(auth.key()),
    ];
    let ix = Instruction {
        program_id: &TOKEN_2022_PROGRAM_ID,
        accounts: &m,
        data: &data,
    };
    pinocchio::cpi::slice_invoke_signed(&ix, &[src, mint, dst, auth, tp], signers)
}

#[inline(never)]
fn cpi_mint_to(
    tp: &AccountInfo,
    mint: &AccountInfo,
    dst: &AccountInfo,
    auth: &AccountInfo,
    amount: u64,
    signers: &[Signer],
) -> ProgramResult {
    let mut data = [0u8; 9];
    data[0] = 7; // MintTo
    data[1..9].copy_from_slice(&amount.to_le_bytes());
    let m = [
        AccountMeta::writable(mint.key()),
        AccountMeta::writable(dst.key()),
        AccountMeta::readonly_signer(auth.key()),
    ];
    let ix = Instruction {
        program_id: &TOKEN_2022_PROGRAM_ID,
        accounts: &m,
        data: &data,
    };
    pinocchio::cpi::slice_invoke_signed(&ix, &[mint, dst, auth, tp], signers)
}

#[inline(never)]
fn cpi_burn(
    tp: &AccountInfo,
    acct: &AccountInfo,
    mint: &AccountInfo,
    auth: &AccountInfo,
    amount: u64,
    signers: &[Signer],
) -> ProgramResult {
    let mut data = [0u8; 9];
    data[0] = 8; // Burn
    data[1..9].copy_from_slice(&amount.to_le_bytes());
    let m = [
        AccountMeta::writable(acct.key()),
        AccountMeta::writable(mint.key()),
        AccountMeta::readonly_signer(auth.key()),
    ];
    let ix = Instruction {
        program_id: &TOKEN_2022_PROGRAM_ID,
        accounts: &m,
        data: &data,
    };
    pinocchio::cpi::slice_invoke_signed(&ix, &[acct, mint, auth, tp], signers)
}

#[inline(never)]
fn cpi_create_ata(
    payer: &AccountInfo,
    owner: &AccountInfo,
    mint: &AccountInfo,
    tp: &AccountInfo,
    ata: &AccountInfo,
    sys: &AccountInfo,
    atp: &AccountInfo,
) -> ProgramResult {
    let m = [
        AccountMeta::writable_signer(payer.key()),
        AccountMeta::writable(ata.key()),
        AccountMeta::readonly(owner.key()),
        AccountMeta::readonly(mint.key()),
        AccountMeta::readonly(sys.key()),
        AccountMeta::readonly(tp.key()),
    ];
    let ix = Instruction {
        program_id: &ASSOCIATED_TOKEN_PROGRAM_ID,
        accounts: &m,
        data: &[1],
    };
    pinocchio::cpi::slice_invoke_signed(&ix, &[payer, ata, owner, mint, sys, tp, atp], &[])
}

/// Close account: transfer lamports, zero data, assign to system program.
#[inline(never)]
fn close_account(acct: &AccountInfo, dest: &AccountInfo) -> ProgramResult {
    let s = unsafe { acct.borrow_mut_lamports_unchecked() };
    let d = unsafe { dest.borrow_mut_lamports_unchecked() };
    *d = d.checked_add(*s).ok_or(ProgramError::ArithmeticOverflow)?;
    *s = 0;
    let mut data = unsafe { acct.borrow_mut_data_unchecked() };
    for b in data.iter_mut() {
        *b = 0;
    }
    // SAFETY: We have exclusive access to the account (lamports zeroed, data zeroed).
    unsafe {
        acct.assign(&crate::SYSTEM_ID);
    }
    Ok(())
}

// =============================================================================
// 1. INITIALIZE FEE CONFIG
// =============================================================================
//
// Anchor accounts (governance.rs InitializeFeeConfig):
//   0: admin          (signer, mut)
//   1: protocol_state (PDA: ["protocol", mint])
//   2: mint           (Token-2022)
//   3: fee_config     (PDA: ["protocol", mint, "fee_config"], init)
//   4: system_program
//
// ix_data: basis_points(u16) + treasury_fee_bps(u16) + creator_fee_bps(u16)
//          + tier_multipliers([u32; 6]) = 30 bytes

pub fn initialize_fee_config(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 5 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let admin = &accounts[0];
    let protocol_state = &accounts[1];
    let mint = &accounts[2];
    let fee_config = &accounts[3];

    if !admin.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if ix_data.len() < 30 {
        return Err(ProgramError::Custom(ERR_INVALID_INPUT_LENGTH));
    }

    let basis_points = read_u16_le(ix_data, 0);
    let treasury_fee_bps = read_u16_le(ix_data, 2);
    let creator_fee_bps = read_u16_le(ix_data, 4);
    let mut tier_mults = [0u32; 6];
    for i in 0..6 {
        tier_mults[i] = read_u32_le(ix_data, 6 + i * 4);
    }

    if treasury_fee_bps > 10_000
        || creator_fee_bps > 10_000
        || treasury_fee_bps + creator_fee_bps > 10_000
    {
        return Err(ProgramError::Custom(ERR_INVALID_INPUT_LENGTH));
    }

    // Verify protocol_state (legacy PDA: ["protocol", mint])
    verify_legacy_protocol_admin(admin, protocol_state, mint, program_id)?;

    // Derive + create fee_config PDA
    let (expected, fc_bump) = pubkey::find_program_address(
        &[PROTOCOL_SEED, mint.key().as_ref(), b"fee_config"],
        program_id,
    );
    if !pubkey::pubkey_eq(&expected, fee_config.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    let bump_ref = [fc_bump];
    let seeds = [
        Seed::from(PROTOCOL_SEED),
        Seed::from(mint.key().as_ref()),
        Seed::from(b"fee_config" as &[u8]),
        Seed::from(bump_ref.as_ref()),
    ];
    let signer = Signer::from(&seeds);

    crate::cpi_create_account(
        admin,
        fee_config,
        Rent::get()?.minimum_balance(FEE_CONFIG_LEN),
        FEE_CONFIG_LEN as u64,
        program_id,
        &[signer],
    )?;

    {
        let mut d = unsafe { fee_config.borrow_mut_data_unchecked() };
        d[0..8].copy_from_slice(&FEE_CONFIG_DISC);
        write_u16_le(&mut d, FC_BASIS_POINTS, basis_points);
        write_u64_le(&mut d, FC_MAX_FEE, 1_000_000_000);
        write_u64_le(&mut d, FC_DRIP_THRESHOLD, 1_000_000);
        write_u16_le(&mut d, FC_TREASURY_FEE_BPS, treasury_fee_bps);
        write_u16_le(&mut d, FC_CREATOR_FEE_BPS, creator_fee_bps);
        for i in 0..6 {
            write_u32_le(&mut d, FC_TIER_MULTIPLIERS + i * 4, tier_mults[i]);
        }
        write_u8(&mut d, FC_BUMP, fc_bump);
    }

    Ok(())
}

// =============================================================================
// 2. INITIALIZE STAKE POOL
// =============================================================================
//
// Anchor accounts (staking.rs InitializeStakePool):
//   0: payer           (signer, mut)
//   1: protocol_state  (PDA: ["protocol_state"])
//   2: channel_config  (ChannelConfigV2)
//   3: mint            (Token-2022 CCM)
//   4: stake_pool      (PDA: ["channel_pool", channel_config], init)
//   5: vault           (PDA: ["stake_vault", stake_pool], init token acct)
//   6: token_program   (Token-2022)
//   7: system_program

pub fn initialize_stake_pool(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 8 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let payer = &accounts[0];
    let protocol_state = &accounts[1];
    let channel_config = &accounts[2];
    let mint = &accounts[3];
    let stake_pool = &accounts[4];
    let vault = &accounts[5];
    let token_program = &accounts[6];

    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if token_program.key() != &TOKEN_2022_PROGRAM_ID {
        return Err(ProgramError::Custom(ERR_INVALID_TOKEN_PROGRAM));
    }

    // Verify protocol_state PDA ["protocol_state"]
    let (ps_mint, _) = verify_live_protocol(payer, protocol_state, program_id, true)?;

    // Verify channel_config.mint
    {
        let d = unsafe { channel_config.borrow_data_unchecked() };
        if d.len() < CHANNEL_CONFIG_V2_LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        if read_pubkey(&d, CC_MINT) != ps_mint {
            return Err(ProgramError::Custom(ERR_INVALID_MINT));
        }
    }
    if mint.key() != &ps_mint {
        return Err(ProgramError::Custom(ERR_INVALID_MINT));
    }

    // Derive + create stake_pool
    let (exp_sp, sp_bump) = pubkey::find_program_address(
        &[CHANNEL_STAKE_POOL_SEED, channel_config.key().as_ref()],
        program_id,
    );
    if !pubkey::pubkey_eq(&exp_sp, stake_pool.key()) {
        return Err(ProgramError::InvalidSeeds);
    }
    let sp_bump_ref = [sp_bump];
    let sp_seeds = [
        Seed::from(CHANNEL_STAKE_POOL_SEED),
        Seed::from(channel_config.key().as_ref()),
        Seed::from(sp_bump_ref.as_ref()),
    ];
    let sp_signer = Signer::from(&sp_seeds);
    let rent = Rent::get()?;
    crate::cpi_create_account(
        payer,
        stake_pool,
        rent.minimum_balance(CHANNEL_STAKE_POOL_LEN),
        CHANNEL_STAKE_POOL_LEN as u64,
        program_id,
        &[sp_signer],
    )?;

    // Derive + create vault token account
    let (exp_v, v_bump) =
        pubkey::find_program_address(&[STAKE_VAULT_SEED, stake_pool.key().as_ref()], program_id);
    if !pubkey::pubkey_eq(&exp_v, vault.key()) {
        return Err(ProgramError::InvalidSeeds);
    }
    let v_bump_ref = [v_bump];
    let v_seeds = [
        Seed::from(STAKE_VAULT_SEED),
        Seed::from(stake_pool.key().as_ref()),
        Seed::from(v_bump_ref.as_ref()),
    ];
    let v_signer = Signer::from(&v_seeds);
    crate::cpi_create_account(
        payer,
        vault,
        rent.minimum_balance(165),
        165,
        &TOKEN_2022_PROGRAM_ID,
        &[v_signer],
    )?;

    // InitializeAccount3: [18, owner(32)]
    {
        let mut id = [0u8; 33];
        id[0] = 18;
        id[1..33].copy_from_slice(stake_pool.key());
        let m = [
            AccountMeta::writable(vault.key()),
            AccountMeta::readonly(mint.key()),
        ];
        let ix = Instruction {
            program_id: &TOKEN_2022_PROGRAM_ID,
            accounts: &m,
            data: &id,
        };
        pinocchio::cpi::slice_invoke_signed(&ix, &[vault, mint, token_program], &[])?;
    }

    // Write stake pool data
    let clock = Clock::get()?;
    {
        let mut d = unsafe { stake_pool.borrow_mut_data_unchecked() };
        d[0..8].copy_from_slice(&CHANNEL_STAKE_POOL_DISC);
        write_u8(&mut d, SP_BUMP, sp_bump);
        write_pubkey(&mut d, SP_CHANNEL, channel_config.key());
        write_pubkey(&mut d, SP_MINT, mint.key());
        write_pubkey(&mut d, SP_VAULT, vault.key());
        write_u64_le(&mut d, SP_TOTAL_STAKED, 0);
        write_u64_le(&mut d, SP_TOTAL_WEIGHTED, 0);
        write_u64_le(&mut d, SP_STAKER_COUNT, 0);
        write_u128_le(&mut d, SP_ACC_REWARD_PER_SHARE, 0);
        write_u64_le(&mut d, SP_LAST_REWARD_SLOT, clock.slot);
        write_u64_le(&mut d, SP_REWARD_PER_SLOT, 0);
        write_u8(&mut d, SP_IS_SHUTDOWN, 0);
    }

    Ok(())
}

// =============================================================================
// 3. STAKE CHANNEL
// =============================================================================
//
// Anchor accounts (staking.rs StakeChannel):
//   0:  user, 1: payer, 2: protocol_state, 3: channel_config,
//   4:  mint, 5: stake_pool, 6: user_stake, 7: vault,
//   8:  user_token_account, 9: nft_mint, 10: nft_ata,
//   11: token_program, 12: associated_token_program,
//   13: system_program, 14: rent
//
// ix_data: amount(u64) + lock_duration(u64) = 16 bytes

pub fn stake_channel(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 15 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let user = &accounts[0];
    let payer = &accounts[1];
    let protocol_state = &accounts[2];
    let channel_config = &accounts[3];
    let mint = &accounts[4];
    let stake_pool = &accounts[5];
    let user_stake = &accounts[6];
    let vault = &accounts[7];
    let user_token_account = &accounts[8];
    let nft_mint = &accounts[9];
    let nft_ata = &accounts[10];
    let token_program = &accounts[11];
    let ata_program = &accounts[12];
    let system_program = &accounts[13];

    if !user.is_signer() || !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if token_program.key() != &TOKEN_2022_PROGRAM_ID {
        return Err(ProgramError::Custom(ERR_INVALID_TOKEN_PROGRAM));
    }
    if ix_data.len() < 16 {
        return Err(ProgramError::Custom(ERR_INVALID_INPUT_LENGTH));
    }

    let amount = read_u64_le(ix_data, 0);
    let lock_duration = read_u64_le(ix_data, 8);
    if amount < MIN_STAKE_AMOUNT {
        return Err(ProgramError::Custom(ERR_STAKE_BELOW_MINIMUM));
    }
    if lock_duration > MAX_LOCK_SLOTS {
        return Err(ProgramError::Custom(ERR_LOCK_PERIOD_TOO_LONG));
    }

    // Verify protocol_state not paused
    let (ps_mint, paused) = verify_live_protocol(user, protocol_state, program_id, false)?;
    if paused {
        return Err(ProgramError::Custom(ERR_PROTOCOL_PAUSED));
    }

    // Verify channel_config + mint
    {
        let d = unsafe { channel_config.borrow_data_unchecked() };
        if d.len() < CHANNEL_CONFIG_V2_LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        if read_pubkey(&d, CC_MINT) != ps_mint {
            return Err(ProgramError::Custom(ERR_INVALID_MINT));
        }
    }
    if mint.key() != &ps_mint {
        return Err(ProgramError::Custom(ERR_INVALID_MINT));
    }

    // Verify stake_pool + not shutdown + vault
    let sp_d = unsafe { stake_pool.borrow_data_unchecked() };
    if sp_d.len() >= CHANNEL_STAKE_POOL_LEN && sp_d[SP_IS_SHUTDOWN] != 0 {
        return Err(ProgramError::Custom(ERR_POOL_IS_SHUTDOWN));
    }
    drop(sp_d);
    let sp_bump = verify_stake_pool(stake_pool, channel_config, vault, None, program_id)?;

    // Verify user_token_account
    if &token_owner_key(user_token_account)? != user.key() {
        return Err(ProgramError::Custom(ERR_UNAUTHORIZED));
    }
    if &token_mint_key(user_token_account)? != mint.key() {
        return Err(ProgramError::Custom(ERR_INVALID_MINT));
    }

    let clock = Clock::get()?;
    let current_slot = clock.slot;
    let multiplier_bps = calculate_boost_bps(lock_duration);
    let lock_end_slot = if lock_duration > 0 {
        current_slot
            .checked_add(lock_duration)
            .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?
    } else {
        0
    };
    let decimals = mint_decimals(mint)?;

    // Capture vault balance, transfer, measure actual received
    let vault_before = token_amount(vault)?;
    cpi_transfer_checked(
        token_program,
        user_token_account,
        mint,
        vault,
        user,
        amount,
        decimals,
        &[],
    )?;
    let actual_received = token_amount(vault)?
        .checked_sub(vault_before)
        .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?;
    if actual_received == 0 {
        return Err(ProgramError::Custom(ERR_STAKE_BELOW_MINIMUM));
    }

    // Handle NFT mint
    let (_, nft_bump) = pubkey::find_program_address(
        &[
            STAKE_NFT_MINT_SEED,
            stake_pool.key().as_ref(),
            user.key().as_ref(),
        ],
        program_id,
    );
    let nft_bump_ref = [nft_bump];
    let pool_bump_ref = [sp_bump];

    if nft_mint.data_len() > 0 {
        // Re-stake: ATA + conditional mint
        cpi_create_ata(
            payer,
            user,
            nft_mint,
            token_program,
            nft_ata,
            system_program,
            ata_program,
        )?;
        let has_auth = {
            let d = unsafe { nft_mint.borrow_data_unchecked() };
            d.len() >= 36 && read_u32_le(&d, 0) == 1 && &read_pubkey(&d, 4) == stake_pool.key()
        };
        if has_auth {
            let ps = [
                Seed::from(CHANNEL_STAKE_POOL_SEED),
                Seed::from(channel_config.key().as_ref()),
                Seed::from(pool_bump_ref.as_ref()),
            ];
            let psig = Signer::from(&ps);
            cpi_mint_to(token_program, nft_mint, nft_ata, stake_pool, 1, &[psig])?;
        }
    } else {
        // Fresh: create mint with NonTransferable
        let rent = Rent::get()?;
        let ns = [
            Seed::from(STAKE_NFT_MINT_SEED),
            Seed::from(stake_pool.key().as_ref()),
            Seed::from(user.key().as_ref()),
            Seed::from(nft_bump_ref.as_ref()),
        ];
        let nsig = Signer::from(&ns);
        crate::cpi_create_account(
            payer,
            nft_mint,
            rent.minimum_balance(170),
            170,
            &TOKEN_2022_PROGRAM_ID,
            &[nsig],
        )?;

        // InitializeNonTransferableMint (32 = 0x20)
        let m1 = [AccountMeta::writable(nft_mint.key())];
        pinocchio::cpi::slice_invoke_signed(
            &Instruction {
                program_id: &TOKEN_2022_PROGRAM_ID,
                accounts: &m1,
                data: &[32u8],
            },
            &[nft_mint, token_program],
            &[],
        )?;

        // InitializeMint2 (20): [20, dec, auth(32), option(1), freeze(32)]
        let mut imd = [0u8; 67];
        imd[0] = 20;
        imd[1] = 0;
        imd[2..34].copy_from_slice(stake_pool.key());
        imd[34] = 1;
        imd[35..67].copy_from_slice(stake_pool.key());
        let m2 = [AccountMeta::writable(nft_mint.key())];
        pinocchio::cpi::slice_invoke_signed(
            &Instruction {
                program_id: &TOKEN_2022_PROGRAM_ID,
                accounts: &m2,
                data: &imd,
            },
            &[nft_mint, token_program],
            &[],
        )?;

        cpi_create_ata(
            payer,
            user,
            nft_mint,
            token_program,
            nft_ata,
            system_program,
            ata_program,
        )?;

        let ps = [
            Seed::from(CHANNEL_STAKE_POOL_SEED),
            Seed::from(channel_config.key().as_ref()),
            Seed::from(pool_bump_ref.as_ref()),
        ];
        let psig = Signer::from(&ps);
        cpi_mint_to(token_program, nft_mint, nft_ata, stake_pool, 1, &[psig])?;
    }

    // Update pool rewards + totals
    let current_acc = {
        let mut sp = unsafe { stake_pool.borrow_mut_data_unchecked() };
        update_pool_rewards(&mut sp, current_slot)?;
        let acc = read_u128_le(&sp, SP_ACC_REWARD_PER_SHARE);
        let wa = u64::try_from(
            (actual_received as u128)
                .checked_mul(multiplier_bps as u128)
                .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?
                .checked_div(BOOST_PRECISION as u128)
                .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?,
        )
        .map_err(|_| ProgramError::Custom(ERR_MATH_OVERFLOW))?;
        let ts = read_u64_le(&sp, SP_TOTAL_STAKED);
        let tw = read_u64_le(&sp, SP_TOTAL_WEIGHTED);
        let sc = read_u64_le(&sp, SP_STAKER_COUNT);
        write_u64_le(
            &mut sp,
            SP_TOTAL_STAKED,
            ts.checked_add(actual_received)
                .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?,
        );
        write_u64_le(
            &mut sp,
            SP_TOTAL_WEIGHTED,
            tw.checked_add(wa)
                .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?,
        );
        write_u64_le(
            &mut sp,
            SP_STAKER_COUNT,
            sc.checked_add(1)
                .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?,
        );
        acc
    };

    // Create user_stake account (init_if_needed: skip if already allocated)
    let (_, us_bump) = pubkey::find_program_address(
        &[
            CHANNEL_USER_STAKE_SEED,
            channel_config.key().as_ref(),
            user.key().as_ref(),
        ],
        program_id,
    );
    let us_bump_ref = [us_bump];
    if user_stake.data_len() == 0 {
        let us_seeds = [
            Seed::from(CHANNEL_USER_STAKE_SEED),
            Seed::from(channel_config.key().as_ref()),
            Seed::from(user.key().as_ref()),
            Seed::from(us_bump_ref.as_ref()),
        ];
        let us_signer = Signer::from(&us_seeds);
        let rent = Rent::get()?;
        crate::cpi_create_account(
            payer,
            user_stake,
            rent.minimum_balance(USER_CHANNEL_STAKE_LEN),
            USER_CHANNEL_STAKE_LEN as u64,
            program_id,
            &[us_signer],
        )?;
    }

    let rd = reward_debt(actual_received, multiplier_bps, current_acc)?;
    {
        let mut d = unsafe { user_stake.borrow_mut_data_unchecked() };
        d[0..8].copy_from_slice(&USER_CHANNEL_STAKE_DISC);
        write_u8(&mut d, US_BUMP, us_bump);
        write_pubkey(&mut d, US_USER, user.key());
        write_pubkey(&mut d, US_CHANNEL, channel_config.key());
        write_u64_le(&mut d, US_AMOUNT, actual_received);
        write_u64_le(&mut d, US_START_SLOT, current_slot);
        write_u64_le(&mut d, US_LOCK_END_SLOT, lock_end_slot);
        write_u64_le(&mut d, US_MULTIPLIER_BPS, multiplier_bps);
        write_pubkey(&mut d, US_NFT_MINT, nft_mint.key());
        write_u128_le(&mut d, US_REWARD_DEBT, rd);
        write_u64_le(&mut d, US_PENDING_REWARDS, 0);
    }

    Ok(())
}

// =============================================================================
// 4. UNSTAKE CHANNEL
// =============================================================================
//
// Anchor accounts (staking.rs UnstakeChannel):
//   0: user, 1: channel_config, 2: mint, 3: stake_pool,
//   4: user_stake (close=user), 5: vault, 6: user_token_account,
//   7: nft_mint, 8: nft_ata, 9: token_program,
//   10: associated_token_program

pub fn unstake_channel(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 11 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let user = &accounts[0];
    let channel_config = &accounts[1];
    let mint = &accounts[2];
    let stake_pool = &accounts[3];
    let user_stake = &accounts[4];
    let vault = &accounts[5];
    let user_token_account = &accounts[6];
    let nft_mint = &accounts[7];
    let nft_ata = &accounts[8];
    let token_program = &accounts[9];

    if !user.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if token_program.key() != &TOKEN_2022_PROGRAM_ID {
        return Err(ProgramError::Custom(ERR_INVALID_TOKEN_PROGRAM));
    }

    let clock = Clock::get()?;
    let current_slot = clock.slot;

    // Read user_stake fields
    let (us_amount, us_mul, us_lock, us_nft, us_user, us_bump) = {
        let d = unsafe { user_stake.borrow_data_unchecked() };
        if d.len() < USER_CHANNEL_STAKE_LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        (
            read_u64_le(&d, US_AMOUNT),
            read_u64_le(&d, US_MULTIPLIER_BPS),
            read_u64_le(&d, US_LOCK_END_SLOT),
            read_pubkey(&d, US_NFT_MINT),
            read_pubkey(&d, US_USER),
            read_u8(&d, US_BUMP),
        )
    };
    if &us_user != user.key() {
        return Err(ProgramError::Custom(ERR_UNAUTHORIZED));
    }
    let pda = pubkey::create_program_address(
        &[
            CHANNEL_USER_STAKE_SEED,
            channel_config.key().as_ref(),
            user.key().as_ref(),
            &[us_bump],
        ],
        program_id,
    )?;
    if !pubkey::pubkey_eq(&pda, user_stake.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Read stake_pool
    let sp_shutdown = {
        let d = unsafe { stake_pool.borrow_data_unchecked() };
        d.len() >= CHANNEL_STAKE_POOL_LEN && d[SP_IS_SHUTDOWN] != 0
    };
    let sp_bump = verify_stake_pool(stake_pool, channel_config, vault, Some(mint), program_id)?;

    if &us_nft != nft_mint.key() {
        return Err(ProgramError::InvalidAccountData);
    }
    if &token_owner_key(user_token_account)? != user.key() {
        return Err(ProgramError::Custom(ERR_UNAUTHORIZED));
    }
    if &token_mint_key(user_token_account)? != mint.key() {
        return Err(ProgramError::Custom(ERR_INVALID_MINT));
    }

    // Lock check (waived if shutdown)
    if !sp_shutdown && us_lock > 0 && current_slot < us_lock {
        return Err(ProgramError::Custom(ERR_LOCK_NOT_EXPIRED));
    }

    // Update pool + calculate pending
    let pend = {
        let mut sp = unsafe { stake_pool.borrow_mut_data_unchecked() };
        update_pool_rewards(&mut sp, current_slot)?;
        let us = unsafe { user_stake.borrow_data_unchecked() };
        pending_rewards(&us, &sp)?
    };

    // Block if pending claimable (unless shutdown or underfunded)
    if pend > 0 && !sp_shutdown {
        let vb = token_amount(vault)?;
        let ts = {
            let d = unsafe { stake_pool.borrow_data_unchecked() };
            read_u64_le(&d, SP_TOTAL_STAKED)
        };
        if vb.saturating_sub(ts) >= pend {
            return Err(ProgramError::Custom(ERR_PENDING_REWARDS_ON_UNSTAKE));
        }
    }

    let wa = u64::try_from(
        (us_amount as u128)
            .checked_mul(us_mul as u128)
            .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?
            .checked_div(BOOST_PRECISION as u128)
            .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?,
    )
    .map_err(|_| ProgramError::Custom(ERR_MATH_OVERFLOW))?;

    let decimals = mint_decimals(mint)?;

    // Burn NFT if present
    if token_amount(nft_ata)? > 0 {
        cpi_burn(token_program, nft_ata, nft_mint, user, 1, &[])?;
    }

    // Transfer from vault to user
    let pb = [sp_bump];
    let ps = [
        Seed::from(CHANNEL_STAKE_POOL_SEED),
        Seed::from(channel_config.key().as_ref()),
        Seed::from(pb.as_ref()),
    ];
    let psig = Signer::from(&ps);
    cpi_transfer_checked(
        token_program,
        vault,
        mint,
        user_token_account,
        stake_pool,
        us_amount,
        decimals,
        &[psig],
    )?;

    // Update pool totals
    {
        let mut sp = unsafe { stake_pool.borrow_mut_data_unchecked() };
        let ts = read_u64_le(&sp, SP_TOTAL_STAKED);
        let tw = read_u64_le(&sp, SP_TOTAL_WEIGHTED);
        let sc = read_u64_le(&sp, SP_STAKER_COUNT);
        write_u64_le(
            &mut sp,
            SP_TOTAL_STAKED,
            ts.checked_sub(us_amount)
                .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?,
        );
        write_u64_le(
            &mut sp,
            SP_TOTAL_WEIGHTED,
            tw.checked_sub(wa)
                .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?,
        );
        write_u64_le(
            &mut sp,
            SP_STAKER_COUNT,
            sc.checked_sub(1)
                .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?,
        );
    }

    close_account(user_stake, user)?;
    Ok(())
}

// =============================================================================
// 5. CLAIM CHANNEL REWARDS
// =============================================================================
//
// Anchor accounts (staking.rs ClaimChannelRewards):
//   0: user, 1: channel_config, 2: mint, 3: stake_pool,
//   4: user_stake, 5: vault, 6: user_token_account,
//   7: token_program

pub fn claim_channel_rewards(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 8 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let user = &accounts[0];
    let channel_config = &accounts[1];
    let mint = &accounts[2];
    let stake_pool = &accounts[3];
    let user_stake = &accounts[4];
    let vault = &accounts[5];
    let user_token_account = &accounts[6];
    let token_program = &accounts[7];

    if !user.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if token_program.key() != &TOKEN_2022_PROGRAM_ID {
        return Err(ProgramError::Custom(ERR_INVALID_TOKEN_PROGRAM));
    }

    // Verify user_stake
    {
        let d = unsafe { user_stake.borrow_data_unchecked() };
        if d.len() < USER_CHANNEL_STAKE_LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        if &read_pubkey(&d, US_USER) != user.key() {
            return Err(ProgramError::Custom(ERR_UNAUTHORIZED));
        }
        let b = read_u8(&d, US_BUMP);
        drop(d);
        let pda = pubkey::create_program_address(
            &[
                CHANNEL_USER_STAKE_SEED,
                channel_config.key().as_ref(),
                user.key().as_ref(),
                &[b],
            ],
            program_id,
        )?;
        if !pubkey::pubkey_eq(&pda, user_stake.key()) {
            return Err(ProgramError::InvalidSeeds);
        }
    }

    // Verify stake_pool
    let sp_bump = verify_stake_pool(stake_pool, channel_config, vault, Some(mint), program_id)?;

    if &token_owner_key(user_token_account)? != user.key() {
        return Err(ProgramError::Custom(ERR_UNAUTHORIZED));
    }
    if &token_mint_key(user_token_account)? != mint.key() {
        return Err(ProgramError::Custom(ERR_INVALID_MINT));
    }

    let clock = Clock::get()?;

    // Update pool rewards
    {
        let mut sp = unsafe { stake_pool.borrow_mut_data_unchecked() };
        update_pool_rewards(&mut sp, clock.slot)?;
    }

    // Calculate pending
    let pend = {
        let sp = unsafe { stake_pool.borrow_data_unchecked() };
        let us = unsafe { user_stake.borrow_data_unchecked() };
        pending_rewards(&us, &sp)?
    };
    if pend == 0 {
        return Err(ProgramError::Custom(ERR_NO_REWARDS_TO_CLAIM));
    }

    // Principal protection invariant
    let vb = token_amount(vault)?;
    let ts = {
        let d = unsafe { stake_pool.borrow_data_unchecked() };
        read_u64_le(&d, SP_TOTAL_STAKED)
    };
    if vb.saturating_sub(ts) < pend {
        return Err(ProgramError::Custom(ERR_CLAIM_EXCEEDS_AVAILABLE));
    }

    let decimals = mint_decimals(mint)?;
    let pb = [sp_bump];
    let ps = [
        Seed::from(CHANNEL_STAKE_POOL_SEED),
        Seed::from(channel_config.key().as_ref()),
        Seed::from(pb.as_ref()),
    ];
    let psig = Signer::from(&ps);
    cpi_transfer_checked(
        token_program,
        vault,
        mint,
        user_token_account,
        stake_pool,
        pend,
        decimals,
        &[psig],
    )?;

    // Update reward debt
    {
        let sp = unsafe { stake_pool.borrow_data_unchecked() };
        let acc = read_u128_le(&sp, SP_ACC_REWARD_PER_SHARE);
        drop(sp);
        let mut us = unsafe { user_stake.borrow_mut_data_unchecked() };
        let amt = read_u64_le(&us, US_AMOUNT);
        let mul = read_u64_le(&us, US_MULTIPLIER_BPS);
        let new_debt = reward_debt(amt, mul, acc)?;
        write_u128_le(&mut us, US_REWARD_DEBT, new_debt);
        write_u64_le(&mut us, US_PENDING_REWARDS, 0);
    }

    Ok(())
}
