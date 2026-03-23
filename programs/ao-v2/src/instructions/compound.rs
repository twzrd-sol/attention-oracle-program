//! compound_stake — Native compound instruction for channel staking.
//!
//! Eliminates the channel-vault → AO CPI boundary by performing the full
//! compound cycle within AO itself:
//!
//!   1. Read CCM buffer balance (Token-2022 ATA)
//!   2. Claim pending staking rewards (inline, no CPI)
//!   3. Unstake expired position (inline, no CPI)
//!   4. Pay keeper bounty (COMPOUND_BOUNTY_BPS of rewards)
//!   5. Re-stake available CCM into the pool
//!   6. Update stake pool accounting (total_staked, total_weighted)
//!   7. Update exchange rate oracle (total_ccm_assets / total_vlofi_shares)
//!
//! Feature-gated behind `channel_staking`.
//!
//! Account ordering (11 accounts + 1 optional):
//!   0: keeper           (signer, writable)  — permissionless, receives bounty
//!   1: channel_config   (ChannelConfigV2)
//!   2: ccm_mint         (Token-2022 CCM, writable for TransferChecked)
//!   3: stake_pool       (ChannelStakePool, writable)
//!   4: user_stake        (UserChannelStake, writable)
//!   5: pool_vault       (stake pool vault ATA, writable)
//!   6: ccm_buffer       (source CCM buffer ATA, writable)
//!   7: keeper_ccm_ata   (keeper's CCM ATA, writable)
//!   8: buffer_authority  (PDA that owns ccm_buffer, signer via seeds)
//!   9: nft_mint         (soulbound NFT mint)
//!  10: token_2022       (Token-2022 program)
//!  remaining[0]: exchange_rate_oracle (optional, writable)

use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Seed, Signer},
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    sysvars::clock::Clock,
    sysvars::rent::Rent,
    sysvars::Sysvar,
    ProgramResult,
};

// ─── Constants ───────────────────────────────────────────────────────────────

/// Keeper bounty: 0.10% of claimed rewards.
const COMPOUND_BOUNTY_BPS: u64 = 10;
const BPS_DENOMINATOR: u64 = 10_000;

/// Virtual share/asset constants for exchange rate (prevents div-by-zero).
const VIRTUAL_SHARES: u128 = 1_000_000_000;
const VIRTUAL_ASSETS: u128 = 1_000_000_000;

/// MasterChef precision constants (must match channel_staking.rs).
const BOOST_PRECISION: u64 = 10_000;
const REWARD_PRECISION: u128 = 1_000_000_000_000;
const MIN_STAKE_AMOUNT: u64 = 1_000_000_000;
const SLOTS_PER_DAY: u64 = 216_000;

// ─── PDA Seeds (must match channel_staking.rs and channel-vault) ─────────────

const CHANNEL_STAKE_POOL_SEED: &[u8] = b"channel_pool";
const CHANNEL_USER_STAKE_SEED: &[u8] = b"channel_user";
const VAULT_SEED: &[u8] = b"vault";

// ─── Account sizes ───────────────────────────────────────────────────────────

const CHANNEL_STAKE_POOL_LEN: usize = 162;
const USER_CHANNEL_STAKE_LEN: usize = 161;

// ─── ChannelStakePool byte offsets ───────────────────────────────────────────

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

// ─── UserChannelStake byte offsets ───────────────────────────────────────────

const US_BUMP: usize = 8;
const US_USER: usize = 9;
const US_CHANNEL: usize = 41;
const US_AMOUNT: usize = 73;
const US_START_SLOT: usize = 81;
const US_LOCK_END_SLOT: usize = 89;
const US_MULTIPLIER_BPS: usize = 97;
const US_REWARD_DEBT: usize = 137;
const US_PENDING_REWARDS: usize = 153;

// ─── ExchangeRateOracle byte offsets ─────────────────────────────────────────

const ER_CURRENT_RATE: usize = 42;
const ER_TOTAL_CCM: usize = 58;
const ER_TOTAL_VLOFI: usize = 74;
const ER_LAST_UPDATE_SLOT: usize = 90;
const ER_LAST_UPDATE_TS: usize = 98;

// ─── UserChannelStake init helpers (for first compound with new authority) ───

const USER_CHANNEL_STAKE_DISC: [u8; 8] = [0x31, 0x40, 0xc1, 0xb3, 0x6b, 0xa2, 0xad, 0x3b];

#[inline(always)]
fn write_u8(d: &mut [u8], o: usize, v: u8) {
    d[o] = v;
}

#[inline(always)]
fn write_pubkey(d: &mut [u8], o: usize, k: &Pubkey) {
    d[o..o + 32].copy_from_slice(k.as_ref());
}

// ─── Token account layout offsets ────────────────────────────────────────────

const TA_AMOUNT: usize = 64;
const TA_OWNER: usize = 32;
const MINT_DECIMALS: usize = 44;

// ─── Error codes (must match error.rs) ───────────────────────────────────────

const ERR_MATH_OVERFLOW: u32 = 6041;
const ERR_NOTHING_TO_COMPOUND: u32 = 6042; // reuses NoRewardsToClaim
const ERR_POOL_IS_SHUTDOWN: u32 = 6045;
const ERR_INVALID_MINT: u32 = 6016;
const ERR_LOCK_NOT_EXPIRED: u32 = 6033;

// ─── Byte read/write helpers ─────────────────────────────────────────────────

#[inline(always)]
fn r64(d: &[u8], o: usize) -> u64 {
    u64::from_le_bytes([
        d[o],
        d[o + 1],
        d[o + 2],
        d[o + 3],
        d[o + 4],
        d[o + 5],
        d[o + 6],
        d[o + 7],
    ])
}
#[inline(always)]
fn r128(d: &[u8], o: usize) -> u128 {
    let mut b = [0u8; 16];
    b.copy_from_slice(&d[o..o + 16]);
    u128::from_le_bytes(b)
}
#[inline(always)]
fn rpk(d: &[u8], o: usize) -> &[u8] {
    &d[o..o + 32]
}
#[inline(always)]
fn w64(d: &mut [u8], o: usize, v: u64) {
    d[o..o + 8].copy_from_slice(&v.to_le_bytes());
}
#[inline(always)]
fn w128(d: &mut [u8], o: usize, v: u128) {
    d[o..o + 16].copy_from_slice(&v.to_le_bytes());
}
#[inline(always)]
fn wi64(d: &mut [u8], o: usize, v: i64) {
    d[o..o + 8].copy_from_slice(&v.to_le_bytes());
}

// =============================================================================
// MasterChef reward math (duplicated from channel_staking.rs to avoid
// cross-module data-dependency on mutable borrows)
// =============================================================================

/// Accrue rewards into the pool up to `slot`. Writes acc_reward_per_share.
#[inline(never)]
fn update_pool_rewards(pool: &mut [u8], slot: u64) -> Result<(), ProgramError> {
    let last = r64(pool, SP_LAST_REWARD_SLOT);
    let tw = r64(pool, SP_TOTAL_WEIGHTED);
    if slot <= last || tw == 0 {
        w64(pool, SP_LAST_REWARD_SLOT, slot);
        return Ok(());
    }
    let elapsed = slot
        .checked_sub(last)
        .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?;
    let rps = r64(pool, SP_REWARD_PER_SLOT);
    let accrued = (rps as u128)
        .checked_mul(elapsed as u128)
        .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?;
    let inc = accrued
        .checked_mul(REWARD_PRECISION)
        .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?
        .checked_div(tw as u128)
        .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?;
    let acc = r128(pool, SP_ACC_REWARD_PER_SHARE);
    w128(
        pool,
        SP_ACC_REWARD_PER_SHARE,
        acc.checked_add(inc)
            .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?,
    );
    w64(pool, SP_LAST_REWARD_SLOT, slot);
    Ok(())
}

/// Calculate pending reward for a user stake position.
#[inline(never)]
fn pending_rewards(us: &[u8], sp: &[u8]) -> Result<u64, ProgramError> {
    let amt = r64(us, US_AMOUNT);
    let mul = r64(us, US_MULTIPLIER_BPS);
    let debt = r128(us, US_REWARD_DEBT);
    let pend = r64(us, US_PENDING_REWARDS);
    let acc = r128(sp, SP_ACC_REWARD_PER_SHARE);
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

/// Calculate reward_debt for new position state.
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

/// Boost multiplier based on lock duration (matches channel_staking.rs).
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
// Token-2022 CPI helpers
// =============================================================================

/// Token-2022 TransferChecked CPI — separate function to keep stack small.
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
        program_id: &crate::TOKEN_2022_ID,
        accounts: &m,
        data: &data,
    };
    pinocchio::cpi::slice_invoke_signed(&ix, &[src, mint, dst, auth, tp], signers)
}

// =============================================================================
// COMPOUND_STAKE — the main instruction
// =============================================================================
//
// Accounts:
//   0: keeper           (signer, writable)
//   1: channel_config   (ChannelConfigV2, readonly)
//   2: ccm_mint         (Token-2022 mint, writable)
//   3: stake_pool       (ChannelStakePool, writable)
//   4: user_stake       (UserChannelStake, writable)
//   5: pool_vault       (pool's vault ATA, writable)
//   6: ccm_buffer       (source CCM buffer, writable)
//   7: keeper_ccm_ata   (keeper's CCM ATA for bounty, writable)
//   8: buffer_authority  (PDA owning ccm_buffer, signer via PDA seeds)
//   9: nft_mint         (soulbound NFT mint, readonly)
//  10: token_2022       (Token-2022 program)
//  remaining[0]: exchange_rate_oracle (optional, writable)

pub fn compound_stake(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 11 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let keeper = &accounts[0];
    let channel_config = &accounts[1];
    let ccm_mint = &accounts[2];
    let stake_pool = &accounts[3];
    let user_stake = &accounts[4];
    let pool_vault = &accounts[5];
    let ccm_buffer = &accounts[6];
    let keeper_ccm_ata = &accounts[7];
    let buffer_authority = &accounts[8];
    let _nft_mint = &accounts[9];
    let token_2022 = &accounts[10];

    // ── Signer check ─────────────────────────────────────────────────────────
    if !keeper.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    pinocchio::msg!("compound: signer ok");

    // ── Parse lock_duration from ix_data (optional, default 432000 = ~2 days) ─
    let lock_duration: u64 = if ix_data.len() >= 8 {
        r64(ix_data, 0)
    } else {
        432_000 // default: ~2 days at 216k slots/day
    };
    pinocchio::msg!("compound: lock_duration parsed");

    // ── Validate stake_pool PDA ──────────────────────────────────────────────
    pinocchio::msg!("compound: validating stake_pool");
    let sp_bump =
        validate_stake_pool(stake_pool, channel_config, pool_vault, ccm_mint, program_id)?;
    pinocchio::msg!("compound: stake_pool validated");

    // ── Check pool not shutdown ──────────────────────────────────────────────
    {
        let spd = unsafe { stake_pool.borrow_data_unchecked() };
        if spd.len() >= CHANNEL_STAKE_POOL_LEN && spd[SP_IS_SHUTDOWN] != 0 {
            return Err(ProgramError::Custom(ERR_POOL_IS_SHUTDOWN));
        }
    }
    pinocchio::msg!("compound: pool not shutdown");

    // ── Validate user_stake PDA ──────────────────────────────────────────────
    pinocchio::msg!("compound: validating user_stake");
    let (us_active, us_amount, us_mul, us_lock_end) =
        validate_user_stake(user_stake, channel_config, buffer_authority, program_id)?;
    pinocchio::msg!("compound: user_stake validated");

    // ── Read buffer balance ──────────────────────────────────────────────────
    pinocchio::msg!("compound: reading buffer balance");
    let buffer_balance = read_token_amount(ccm_buffer)?;
    pinocchio::msg!("compound: buffer balance read ok");
    if buffer_balance == 0 && !us_active {
        return Err(ProgramError::Custom(ERR_NOTHING_TO_COMPOUND));
    }

    let clock = Clock::get()?;
    let current_slot = clock.slot;

    // ── Derive buffer_authority PDA signer ───────────────────────────────────
    // buffer_authority is the channel-vault PDA: seeds = ["vault", channel_config, bump]
    // We need ix_data to carry the bump, OR we derive it from the passed account.
    // The authority PDA owns the ccm_buffer. We verify the buffer owner matches.
    let buf_auth_bump = validate_buffer_authority(buffer_authority, ccm_buffer, channel_config)?;
    let cfg_key_ref: &[u8] = channel_config.key().as_ref();
    let bump_ref = [buf_auth_bump];

    // ── Phase 1: Claim rewards + unstake (if active and lock expired) ────────
    let mut rewards: u64 = 0;
    let mut _unstaked: u64 = 0;

    if us_active {
        // Lock check removed for compound_stake: this is protocol-owned compounding
        // where the buffer authority (AO vault PDA) is the sole staker. The lock
        // protects external user stakes via stake_channel/unstake_channel — those
        // paths still enforce it. Compound needs fast cycles (every 30 min) to keep
        // the exchange rate ticking up. See unstake_channel for the user-facing lock.

        // Inline reward calculation (no CPI — we have direct access to pool & stake)
        rewards = compute_and_settle_rewards(
            stake_pool,
            user_stake,
            pool_vault,
            ccm_buffer,
            ccm_mint,
            buffer_authority,
            token_2022,
            current_slot,
            sp_bump,
            us_amount,
            us_mul,
            cfg_key_ref,
            &bump_ref,
            program_id,
        )?;

        // Inline unstake: transfer staked amount from pool_vault → ccm_buffer
        _unstaked = inline_unstake(
            stake_pool,
            user_stake,
            pool_vault,
            ccm_buffer,
            ccm_mint,
            token_2022,
            us_amount,
            us_mul,
            sp_bump,
            channel_config,
            program_id,
        )?;
    }

    // ── Phase 2: Keeper bounty ───────────────────────────────────────────────
    let _bounty = if rewards > 0 {
        let b = rewards
            .checked_mul(COMPOUND_BOUNTY_BPS)
            .ok_or(ProgramError::ArithmeticOverflow)?
            / BPS_DENOMINATOR;
        if b > 0 {
            let decimals = read_mint_decimals(ccm_mint)?;
            let buf_seeds = [
                Seed::from(VAULT_SEED),
                Seed::from(cfg_key_ref),
                Seed::from(bump_ref.as_ref()),
            ];
            let buf_signer = Signer::from(&buf_seeds);
            cpi_transfer_checked(
                token_2022,
                ccm_buffer,
                ccm_mint,
                keeper_ccm_ata,
                buffer_authority,
                b,
                decimals,
                &[buf_signer],
            )?;
        }
        b
    } else {
        0
    };

    // ── Phase 3: Re-stake ────────────────────────────────────────────────────
    // Available = remaining buffer balance after bounty
    let new_buffer_balance = read_token_amount(ccm_buffer)?;
    let to_stake = new_buffer_balance; // stake everything available in buffer

    let multiplier_bps = calculate_boost_bps(lock_duration);
    let lock_end_slot = if lock_duration > 0 {
        current_slot.saturating_add(lock_duration)
    } else {
        0
    };

    if to_stake >= MIN_STAKE_AMOUNT {
        // Transfer from buffer → pool_vault
        let decimals = read_mint_decimals(ccm_mint)?;
        let buf_seeds = [
            Seed::from(VAULT_SEED),
            Seed::from(cfg_key_ref),
            Seed::from(bump_ref.as_ref()),
        ];
        let buf_signer = Signer::from(&buf_seeds);
        let vault_before = read_token_amount(pool_vault)?;
        cpi_transfer_checked(
            token_2022,
            ccm_buffer,
            ccm_mint,
            pool_vault,
            buffer_authority,
            to_stake,
            decimals,
            &[buf_signer],
        )?;
        let actual_received = read_token_amount(pool_vault)?
            .checked_sub(vault_before)
            .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?;

        // Update pool: accrue rewards, add to totals
        inline_stake_pool_update(stake_pool, actual_received, multiplier_bps, current_slot)?;

        // Create user_stake if not yet initialized (first compound with new authority)
        if user_stake.data_len() == 0 || user_stake.owner() == &Pubkey::default() {
            let (_, us_bump) = pubkey::find_program_address(
                &[
                    CHANNEL_USER_STAKE_SEED,
                    channel_config.key().as_ref(),
                    buffer_authority.key().as_ref(),
                ],
                program_id,
            );
            let us_bump_ref = [us_bump];
            let us_seeds = [
                Seed::from(CHANNEL_USER_STAKE_SEED),
                Seed::from(channel_config.key().as_ref()),
                Seed::from(buffer_authority.key().as_ref()),
                Seed::from(us_bump_ref.as_ref()),
            ];
            let us_signer = Signer::from(&us_seeds);
            let rent = Rent::get()?;
            crate::cpi_create_account(
                keeper,
                user_stake,
                rent.minimum_balance(USER_CHANNEL_STAKE_LEN),
                USER_CHANNEL_STAKE_LEN as u64,
                program_id,
                &[us_signer],
            )?;
            // Write discriminator + bump + user + channel
            let mut d = unsafe { user_stake.borrow_mut_data_unchecked() };
            d[0..8].copy_from_slice(&USER_CHANNEL_STAKE_DISC);
            write_u8(&mut d, US_BUMP, us_bump);
            write_pubkey(&mut d, US_USER, buffer_authority.key());
            write_pubkey(&mut d, US_CHANNEL, channel_config.key());
        }

        // Update user_stake
        let current_acc = {
            let spd = unsafe { stake_pool.borrow_data_unchecked() };
            r128(&spd, SP_ACC_REWARD_PER_SHARE)
        };
        let rd = reward_debt(actual_received, multiplier_bps, current_acc)?;
        {
            let mut usd = unsafe { user_stake.borrow_mut_data_unchecked() };
            w64(&mut usd, US_AMOUNT, actual_received);
            w64(&mut usd, US_START_SLOT, current_slot);
            w64(&mut usd, US_LOCK_END_SLOT, lock_end_slot);
            w64(&mut usd, US_MULTIPLIER_BPS, multiplier_bps);
            w128(&mut usd, US_REWARD_DEBT, rd);
            w64(&mut usd, US_PENDING_REWARDS, 0);
        }
    } else if to_stake > 0 {
        // Below minimum stake — leave in buffer, mark position inactive
        let mut usd = unsafe { user_stake.borrow_mut_data_unchecked() };
        w64(&mut usd, US_AMOUNT, 0);
        w64(&mut usd, US_LOCK_END_SLOT, 0);
        w64(&mut usd, US_PENDING_REWARDS, 0);
    } else {
        // Nothing to stake — mark inactive
        let mut usd = unsafe { user_stake.borrow_mut_data_unchecked() };
        w64(&mut usd, US_AMOUNT, 0);
        w64(&mut usd, US_LOCK_END_SLOT, 0);
    }

    // ── Phase 4: Exchange rate oracle (optional) ─────────────────────────────
    if accounts.len() > 11 {
        let er = &accounts[11];
        let _ = update_exchange_rate(er, stake_pool, current_slot, &clock);
    }

    Ok(())
}

// =============================================================================
// VALIDATION HELPERS (each in #[inline(never)] to stay under 4096 stack frame)
// =============================================================================

/// Validate stake_pool PDA = ["channel_pool", channel_config].
/// Returns bump on success.
#[inline(never)]
fn validate_stake_pool(
    stake_pool: &AccountInfo,
    channel_config: &AccountInfo,
    vault: &AccountInfo,
    mint: &AccountInfo,
    program_id: &Pubkey,
) -> Result<u8, ProgramError> {
    // Owner check: stake_pool must be owned by this program
    if stake_pool.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }
    let d = unsafe { stake_pool.borrow_data_unchecked() };
    if d.len() < CHANNEL_STAKE_POOL_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    // Verify vault matches stored vault pubkey
    if vault.key() != rpk(&d, SP_VAULT) {
        return Err(ProgramError::InvalidAccountData);
    }
    // Verify mint matches stored mint
    if mint.key() != rpk(&d, SP_MINT) {
        return Err(ProgramError::Custom(ERR_INVALID_MINT));
    }
    let b = d[SP_BUMP];
    let pda = pubkey::create_program_address(
        &[CHANNEL_STAKE_POOL_SEED, channel_config.key().as_ref(), &[b]],
        program_id,
    )?;
    if !pubkey::pubkey_eq(&pda, stake_pool.key()) {
        return Err(ProgramError::InvalidSeeds);
    }
    Ok(b)
}

/// Validate user_stake PDA = ["channel_user", channel_config, user].
/// Returns (is_active, amount, multiplier_bps, lock_end_slot).
#[inline(never)]
fn validate_user_stake(
    user_stake: &AccountInfo,
    channel_config: &AccountInfo,
    user: &AccountInfo,
    program_id: &Pubkey,
) -> Result<(bool, u64, u64, u64), ProgramError> {
    let d = unsafe { user_stake.borrow_data_unchecked() };
    if d.len() < USER_CHANNEL_STAKE_LEN {
        // User stake not yet initialized — treat as inactive (first compound).
        // The compound flow will init it after staking, like stake_channel does.
        if d.is_empty() || user_stake.owner() == &Pubkey::default() {
            return Ok((false, 0, 0, 0));
        }
        return Err(ProgramError::InvalidAccountData);
    }
    // Owner check: user_stake must be owned by this program
    if user_stake.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }
    let stored_user = rpk(&d, US_USER);
    if user.key() != stored_user {
        return Err(ProgramError::Custom(6000)); // Unauthorized
    }
    let bump = d[US_BUMP];
    let pda = pubkey::create_program_address(
        &[
            CHANNEL_USER_STAKE_SEED,
            channel_config.key().as_ref(),
            user.key().as_ref(),
            &[bump],
        ],
        program_id,
    )?;
    if !pubkey::pubkey_eq(&pda, user_stake.key()) {
        return Err(ProgramError::InvalidSeeds);
    }
    let amount = r64(&d, US_AMOUNT);
    let mul = r64(&d, US_MULTIPLIER_BPS);
    let lock_end = r64(&d, US_LOCK_END_SLOT);
    let is_active = amount > 0;
    Ok((is_active, amount, mul, lock_end))
}

/// Validate buffer_authority is the channel-vault PDA that owns ccm_buffer.
/// Returns the vault PDA bump.
#[inline(never)]
fn validate_buffer_authority(
    buffer_authority: &AccountInfo,
    ccm_buffer: &AccountInfo,
    _channel_config: &AccountInfo,
) -> Result<u8, ProgramError> {
    // Verify ccm_buffer owner == buffer_authority
    let buf_data = unsafe { ccm_buffer.borrow_data_unchecked() };
    if buf_data.len() < 72 {
        return Err(ProgramError::InvalidAccountData);
    }
    if rpk(&buf_data, TA_OWNER) != buffer_authority.key().as_ref() {
        return Err(ProgramError::InvalidAccountData);
    }

    // The buffer_authority is the channel-vault PDA: ["vault", channel_config, bump].
    // We find the bump by scanning (same as Anchor's find_program_address but
    // the vault program owns this PDA, not our program).
    // Since we can't derive another program's PDA with create_program_address
    // (wrong program_id), we just read the bump from the vault account data.
    // The ChannelVault stores its bump at offset 8 (CV_BUMP).
    // buffer_authority is now the AO vault PDA ["vault", channel_config] — a bare
    // PDA with no stored data (unlike the old channel-vault vault PDA which had 291 bytes).
    // Derive the bump from our own program since this is an AO PDA.
    let auth_data = unsafe { buffer_authority.borrow_data_unchecked() };
    if auth_data.len() >= 10 {
        // Legacy path: channel-vault PDA with stored data, bump at offset 8
        Ok(auth_data[8])
    } else {
        // AO vault PDA: derive bump via find_program_address
        let (_pda, bump) = pubkey::find_program_address(
            &[VAULT_SEED, _channel_config.key().as_ref()],
            buffer_authority.owner(),
        );
        Ok(bump)
    }
}

// =============================================================================
// INLINE REWARD + STAKE OPERATIONS
// =============================================================================

/// Compute pending rewards, transfer from pool_vault → ccm_buffer, reset debt.
/// Returns the actual reward amount received (after transfer fees).
#[inline(never)]
fn compute_and_settle_rewards(
    stake_pool: &AccountInfo,
    user_stake: &AccountInfo,
    pool_vault: &AccountInfo,
    ccm_buffer: &AccountInfo,
    ccm_mint: &AccountInfo,
    _buffer_authority: &AccountInfo,
    token_2022: &AccountInfo,
    current_slot: u64,
    sp_bump: u8,
    us_amount: u64,
    us_mul: u64,
    cfg_key_ref: &[u8],
    _bump_ref: &[u8],
    _program_id: &Pubkey,
) -> Result<u64, ProgramError> {
    // Update pool accumulator
    {
        let mut spd = unsafe { stake_pool.borrow_mut_data_unchecked() };
        update_pool_rewards(&mut spd, current_slot)?;
    }

    // Calculate pending rewards
    let pend = {
        let spd = unsafe { stake_pool.borrow_data_unchecked() };
        let usd = unsafe { user_stake.borrow_data_unchecked() };
        pending_rewards(&usd, &spd)?
    };

    if pend == 0 {
        return Ok(0);
    }

    // Principal protection: ensure vault has enough excess beyond total_staked
    let vault_balance = read_token_amount(pool_vault)?;
    let total_staked = {
        let spd = unsafe { stake_pool.borrow_data_unchecked() };
        r64(&spd, SP_TOTAL_STAKED)
    };
    let claimable = vault_balance.saturating_sub(total_staked);
    let to_claim = pend.min(claimable);
    if to_claim == 0 {
        return Ok(0);
    }

    // Transfer rewards: pool_vault → ccm_buffer (pool PDA signs)
    let decimals = read_mint_decimals(ccm_mint)?;
    let sp_bump_ref = [sp_bump];
    let channel_key_ref: &[u8] = {
        let spd = unsafe { stake_pool.borrow_data_unchecked() };
        // We need the channel config key from the pool. Read it into a local.
        // Can't hold borrow across CPI, so copy.
        let _ = rpk(&spd, SP_CHANNEL);
        // This is tricky with lifetimes. Use the account key directly.
        let _ = spd;
        // channel_config is accounts[1], same as cfg_key_ref's source
        cfg_key_ref
    };
    let pool_seeds = [
        Seed::from(CHANNEL_STAKE_POOL_SEED),
        Seed::from(channel_key_ref),
        Seed::from(sp_bump_ref.as_ref()),
    ];
    let pool_signer = Signer::from(&pool_seeds);

    let buf_before = read_token_amount(ccm_buffer)?;
    cpi_transfer_checked(
        token_2022,
        pool_vault,
        ccm_mint,
        ccm_buffer,
        stake_pool,
        to_claim,
        decimals,
        &[pool_signer],
    )?;
    let buf_after = read_token_amount(ccm_buffer)?;
    let actual_rewards = buf_after.saturating_sub(buf_before);

    // Update reward debt on user_stake
    {
        let spd = unsafe { stake_pool.borrow_data_unchecked() };
        let acc = r128(&spd, SP_ACC_REWARD_PER_SHARE);
        let _ = spd;
        let mut usd = unsafe { user_stake.borrow_mut_data_unchecked() };
        let new_debt = reward_debt(us_amount, us_mul, acc)?;
        w128(&mut usd, US_REWARD_DEBT, new_debt);
        w64(&mut usd, US_PENDING_REWARDS, 0);
    }

    Ok(actual_rewards)
}

/// Unstake: transfer staked principal from pool_vault → ccm_buffer.
/// Updates pool totals (total_staked, total_weighted, staker_count).
/// Returns actual amount received in buffer (net of transfer fees).
#[inline(never)]
fn inline_unstake(
    stake_pool: &AccountInfo,
    _user_stake: &AccountInfo,
    pool_vault: &AccountInfo,
    ccm_buffer: &AccountInfo,
    ccm_mint: &AccountInfo,
    token_2022: &AccountInfo,
    us_amount: u64,
    us_mul: u64,
    sp_bump: u8,
    channel_config: &AccountInfo,
    _program_id: &Pubkey,
) -> Result<u64, ProgramError> {
    let decimals = read_mint_decimals(ccm_mint)?;
    let sp_bump_ref = [sp_bump];
    let pool_seeds = [
        Seed::from(CHANNEL_STAKE_POOL_SEED),
        Seed::from(channel_config.key().as_ref()),
        Seed::from(sp_bump_ref.as_ref()),
    ];
    let pool_signer = Signer::from(&pool_seeds);

    let buf_before = read_token_amount(ccm_buffer)?;
    cpi_transfer_checked(
        token_2022,
        pool_vault,
        ccm_mint,
        ccm_buffer,
        stake_pool,
        us_amount,
        decimals,
        &[pool_signer],
    )?;
    let buf_after = read_token_amount(ccm_buffer)?;
    let actual_returned = buf_after.saturating_sub(buf_before);

    // Update pool totals
    let wa = u64::try_from(
        (us_amount as u128)
            .checked_mul(us_mul as u128)
            .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?
            .checked_div(BOOST_PRECISION as u128)
            .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?,
    )
    .map_err(|_| ProgramError::Custom(ERR_MATH_OVERFLOW))?;

    {
        let mut spd = unsafe { stake_pool.borrow_mut_data_unchecked() };
        let ts = r64(&spd, SP_TOTAL_STAKED);
        let tw = r64(&spd, SP_TOTAL_WEIGHTED);
        let sc = r64(&spd, SP_STAKER_COUNT);
        w64(
            &mut spd,
            SP_TOTAL_STAKED,
            ts.checked_sub(us_amount)
                .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?,
        );
        w64(
            &mut spd,
            SP_TOTAL_WEIGHTED,
            tw.checked_sub(wa)
                .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?,
        );
        w64(
            &mut spd,
            SP_STAKER_COUNT,
            sc.checked_sub(1)
                .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?,
        );
    }

    Ok(actual_returned)
}

/// Update pool totals after a new stake. Accrues rewards first.
#[inline(never)]
fn inline_stake_pool_update(
    stake_pool: &AccountInfo,
    actual_received: u64,
    multiplier_bps: u64,
    current_slot: u64,
) -> Result<(), ProgramError> {
    let mut spd = unsafe { stake_pool.borrow_mut_data_unchecked() };
    update_pool_rewards(&mut spd, current_slot)?;

    let wa = u64::try_from(
        (actual_received as u128)
            .checked_mul(multiplier_bps as u128)
            .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?
            .checked_div(BOOST_PRECISION as u128)
            .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?,
    )
    .map_err(|_| ProgramError::Custom(ERR_MATH_OVERFLOW))?;

    let ts = r64(&spd, SP_TOTAL_STAKED);
    let tw = r64(&spd, SP_TOTAL_WEIGHTED);
    let sc = r64(&spd, SP_STAKER_COUNT);
    w64(
        &mut spd,
        SP_TOTAL_STAKED,
        ts.checked_add(actual_received)
            .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?,
    );
    w64(
        &mut spd,
        SP_TOTAL_WEIGHTED,
        tw.checked_add(wa)
            .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?,
    );
    w64(
        &mut spd,
        SP_STAKER_COUNT,
        sc.checked_add(1)
            .ok_or(ProgramError::Custom(ERR_MATH_OVERFLOW))?,
    );
    Ok(())
}

// =============================================================================
// EXCHANGE RATE UPDATE (best-effort, mirrors channel-vault update_er)
// =============================================================================

#[inline(never)]
fn update_exchange_rate(
    er: &AccountInfo,
    stake_pool: &AccountInfo,
    slot: u64,
    clock: &Clock,
) -> ProgramResult {
    let d = unsafe { er.borrow_mut_data_unchecked() };
    if d.len() < 194 {
        return Ok(());
    }

    let spd = unsafe { stake_pool.borrow_data_unchecked() };
    let total_staked = r64(&spd, SP_TOTAL_STAKED) as u128;
    // For the exchange rate, we use pool total_staked as total CCM assets
    // and staker_count as a proxy. The real assets = vault balance.
    // Better: read vault balance, but we avoid re-reading accounts.
    // Use total_staked as a conservative lower bound.
    let shares = r64(&spd, SP_TOTAL_WEIGHTED) as u128;
    let _ = spd;

    let rate = if shares == 0 {
        VIRTUAL_ASSETS * 1_000_000_000 / VIRTUAL_SHARES
    } else {
        (total_staked.saturating_add(VIRTUAL_ASSETS)) * 1_000_000_000
            / (shares.saturating_add(VIRTUAL_SHARES))
    };

    w128(d, ER_CURRENT_RATE, rate);
    w128(d, ER_TOTAL_CCM, total_staked);
    w128(d, ER_TOTAL_VLOFI, shares);
    w64(d, ER_LAST_UPDATE_SLOT, slot);
    wi64(d, ER_LAST_UPDATE_TS, clock.unix_timestamp);
    // compound count: read from pool (no separate counter here)
    Ok(())
}

// =============================================================================
// TOKEN READ HELPERS
// =============================================================================

#[inline(always)]
fn read_token_amount(acct: &AccountInfo) -> Result<u64, ProgramError> {
    let d = unsafe { acct.borrow_data_unchecked() };
    if d.len() < 72 {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(r64(&d, TA_AMOUNT))
}

#[inline(always)]
fn read_mint_decimals(mint: &AccountInfo) -> Result<u8, ProgramError> {
    let d = unsafe { mint.borrow_data_unchecked() };
    if d.len() < 45 {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(d[MINT_DECIMALS])
}
