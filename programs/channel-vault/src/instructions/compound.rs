//! Compound pending deposits into Oracle stake.
//!
//! This is a permissionless instruction - anyone can call it.
//! The compound strategy:
//! 1. If no active position: stake all pending with 7-day lock
//! 2. If active position and lock expired: unstake, then stake total with 7-day lock
//! 3. If active position and lock not expired: wait (revert)

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint as MintInterface, TokenAccount, TokenInterface, TransferChecked},
};

use crate::constants::{
    BPS_DENOMINATOR,
    COMPOUND_BOUNTY_BPS,
    TOKEN_2022_PROGRAM_ID,
    VAULT_CCM_BUFFER_SEED,
    VAULT_ORACLE_POSITION_SEED,
    VAULT_SEED,
};
use crate::errors::VaultError;
use crate::events::{Compounded, CompoundBountyPaid, ExchangeRateUpdated};
use crate::state::{ChannelVault, ExchangeRateOracle, VaultOraclePosition};

// Import Oracle types, CPI, and state for pre-check
use token_2022::{
    self,
    cpi::accounts::{ClaimChannelRewards, StakeChannel, UnstakeChannel},
    ChannelConfigV2, ChannelStakePool, ProtocolState, UserChannelStake,
    BOOST_PRECISION, CHANNEL_STAKE_POOL_SEED, PROTOCOL_SEED,
    REWARD_PRECISION, STAKE_VAULT_SEED,
};

/// Pre-check whether the Oracle has pending rewards for this vault.
/// Simulates `update_pool_rewards` + `calculate_pending_rewards` using
/// on-chain pool state and the vault's UserChannelStake.
///
/// We must validate BEFORE the CPI because Solana CPI errors propagate
/// immediately through the runtime — they cannot be caught by the caller.
#[inline(never)]
fn has_claimable_rewards(
    oracle_stake_pool: &ChannelStakePool,
    oracle_user_stake_info: &AccountInfo,
    current_slot: u64,
    oracle_vault_balance: u64,
) -> bool {
    let pool = oracle_stake_pool;

    // Fast path: no rewards ever configured
    if pool.acc_reward_per_share == 0 && pool.reward_per_slot == 0 {
        return false;
    }

    // Ensure there are spendable rewards beyond principal.
    // This mirrors claim_channel_rewards' excess check to avoid CPI failure.
    let excess_rewards = oracle_vault_balance
        .saturating_sub(pool.total_staked) as u128;
    if excess_rewards == 0 {
        return false;
    }

    // Simulate update_pool_rewards to get the acc after this slot
    let simulated_acc = if pool.reward_per_slot > 0 && pool.total_weighted > 0 {
        let elapsed = current_slot.saturating_sub(pool.last_reward_slot);
        if elapsed > 0 {
            let rewards = (pool.reward_per_slot as u128).saturating_mul(elapsed as u128);
            let increment = rewards
                .saturating_mul(REWARD_PRECISION)
                .checked_div(pool.total_weighted as u128)
                .unwrap_or(0);
            pool.acc_reward_per_share.saturating_add(increment)
        } else {
            pool.acc_reward_per_share
        }
    } else {
        pool.acc_reward_per_share
    };

    if simulated_acc == 0 {
        return false;
    }

    // Deserialize UserChannelStake to calculate pending rewards
    let Ok(data) = oracle_user_stake_info.try_borrow_data() else {
        return false;
    };
    let mut slice: &[u8] = &data;
    let Ok(us) = UserChannelStake::try_deserialize(&mut slice) else {
        return false;
    };

    let weighted = (us.amount as u128)
        .saturating_mul(us.multiplier_bps as u128)
        / (BOOST_PRECISION as u128);
    let accumulated = weighted
        .saturating_mul(simulated_acc)
        / REWARD_PRECISION;
    let pending = accumulated
        .saturating_sub(us.reward_debt)
        .saturating_add(us.pending_rewards as u128);

    pending > 0 && pending <= excess_rewards
}

#[derive(Accounts)]
pub struct Compound<'info> {
    /// Anyone can call this (permissionless crank)
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [VAULT_SEED, vault.channel_config.as_ref()],
        bump = vault.bump,
        constraint = !vault.paused @ VaultError::VaultPaused,
    )]
    pub vault: Box<Account<'info, ChannelVault>>,

    #[account(
        mut,
        seeds = [VAULT_ORACLE_POSITION_SEED, vault.key().as_ref()],
        bump = vault_oracle_position.bump,
    )]
    pub vault_oracle_position: Box<Account<'info, VaultOraclePosition>>,

    /// Vault's CCM buffer
    #[account(
        mut,
        seeds = [VAULT_CCM_BUFFER_SEED, vault.key().as_ref()],
        bump,
    )]
    pub vault_ccm_buffer: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CCM mint
    #[account(address = vault.ccm_mint)]
    pub ccm_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// Payer's CCM token account (receives compound bounty)
    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = ccm_mint,
        associated_token::authority = payer,
        associated_token::token_program = token_2022_program,
    )]
    pub payer_ccm_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    // -------------------------------------------------------------------------
    // Oracle Accounts
    // -------------------------------------------------------------------------

    /// Oracle program
    /// CHECK: Validated by address
    #[account(address = token_2022::ID)]
    pub oracle_program: AccountInfo<'info>,

    /// Oracle protocol state
    #[account(
        seeds = [PROTOCOL_SEED, oracle_protocol.mint.as_ref()],
        bump = oracle_protocol.bump,
        seeds::program = token_2022::ID,
    )]
    pub oracle_protocol: Box<Account<'info, ProtocolState>>,

    /// Oracle channel config
    #[account(address = vault.channel_config)]
    pub oracle_channel_config: Box<Account<'info, ChannelConfigV2>>,

    /// Oracle stake pool for channel
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_POOL_SEED, oracle_channel_config.key().as_ref()],
        bump = oracle_stake_pool.bump,
        seeds::program = token_2022::ID,
    )]
    pub oracle_stake_pool: Box<Account<'info, ChannelStakePool>>,

    /// Oracle vault holding staked tokens
    #[account(
        mut,
        seeds = [STAKE_VAULT_SEED, oracle_stake_pool.key().as_ref()],
        bump,
        seeds::program = token_2022::ID,
    )]
    pub oracle_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Vault's stake position in Oracle (may need init)
    /// CHECK: Validated/created by Oracle CPI
    #[account(mut)]
    pub oracle_user_stake: UncheckedAccount<'info>,

    /// NFT mint for soulbound receipt
    /// CHECK: Created/validated by Oracle CPI
    #[account(mut)]
    pub oracle_nft_mint: UncheckedAccount<'info>,

    /// Vault's NFT ATA
    /// CHECK: Created by Oracle CPI
    #[account(mut)]
    pub vault_nft_ata: UncheckedAccount<'info>,

    // -------------------------------------------------------------------------
    // Programs
    // -------------------------------------------------------------------------

    #[account(
        constraint = token_2022_program.key() == TOKEN_2022_PROGRAM_ID @ VaultError::InvalidTokenProgram,
    )]
    pub token_2022_program: Interface<'info, TokenInterface>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

/// Update the ExchangeRateOracle if provided as a remaining account.
/// Extracted to its own function to avoid exceeding the SBF 4KB stack frame
/// limit in the compound handler. Derives vault_key and program_id internally
/// to minimize the caller's stack footprint.
#[inline(never)]
fn maybe_update_exchange_rate_oracle<'a>(
    vault: &ChannelVault,
    remaining_accounts: &'a [AccountInfo<'a>],
) -> Result<()> {
    let oracle_info = match remaining_accounts.first() {
        Some(info) => info,
        None => return Ok(()),
    };

    let program_id = &crate::id();

    // Re-derive vault PDA key from stored seeds (avoids passing Pubkey on caller stack)
    let vault_key = Pubkey::create_program_address(
        &[VAULT_SEED, vault.channel_config.as_ref(), &[vault.bump]],
        program_id,
    ).map_err(|_| error!(VaultError::MathOverflow))?;

    let expected_oracle = Pubkey::find_program_address(
        &[crate::constants::EXCHANGE_RATE_SEED, vault_key.as_ref()],
        program_id,
    ).0;

    if oracle_info.key() != expected_oracle
        || !oracle_info.is_writable
        || oracle_info.owner != program_id
    {
        return Ok(());
    }

    let mut oracle_account: Account<ExchangeRateOracle> =
        Account::try_from(oracle_info)?;

    if oracle_account.vault != vault_key {
        return Ok(());
    }

    let clock = Clock::get()?;
    oracle_account.update_from_vault(vault, clock.slot, clock.unix_timestamp)?;

    emit!(ExchangeRateUpdated {
        vault: vault_key,
        current_rate: oracle_account.current_rate,
        total_ccm_assets: oracle_account.total_ccm_assets,
        total_vlofi_shares: oracle_account.total_vlofi_shares,
        compound_count: vault.compound_count,
        slot: clock.slot,
        timestamp: clock.unix_timestamp,
    });

    oracle_account.exit(program_id)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// CPI helpers — each gets its own stack frame via #[inline(never)] to stay
// within SBF's 4096-byte-per-frame limit.  The handler orchestrates them
// sequentially so frames are reused, not stacked.
// ---------------------------------------------------------------------------

/// Claim pending Oracle rewards into vault_ccm_buffer.
/// Returns the CCM amount claimed.
#[inline(never)]
fn do_claim_rewards<'info>(
    accs: &mut Compound<'info>,
    signer_seeds: &[&[&[u8]]],
) -> Result<u64> {
    msg!("Claiming pending rewards before unstake...");
    let buffer_before = accs.vault_ccm_buffer.amount;

    let claim_accounts = ClaimChannelRewards {
        user: accs.vault.to_account_info(),
        channel_config: accs.oracle_channel_config.to_account_info(),
        mint: accs.ccm_mint.to_account_info(),
        stake_pool: accs.oracle_stake_pool.to_account_info(),
        user_stake: accs.oracle_user_stake.to_account_info(),
        vault: accs.oracle_vault.to_account_info(),
        user_token_account: accs.vault_ccm_buffer.to_account_info(),
        token_program: accs.token_2022_program.to_account_info(),
    };

    let claim_ctx = CpiContext::new_with_signer(
        accs.oracle_program.to_account_info(),
        claim_accounts,
        signer_seeds,
    );

    token_2022::cpi::claim_channel_rewards(claim_ctx)?;

    accs.vault_ccm_buffer.reload()?;
    let buffer_after = accs.vault_ccm_buffer.amount;
    let claimed = buffer_after.saturating_sub(buffer_before);
    msg!("Claimed {} CCM in rewards", claimed);
    Ok(claimed)
}

/// Unstake from Oracle, returning actual CCM received (net of transfer fees).
#[inline(never)]
fn do_unstake<'info>(
    accs: &mut Compound<'info>,
    signer_seeds: &[&[&[u8]]],
    stake_amount: u64,
) -> Result<u64> {
    msg!("Unstaking {} CCM from Oracle", stake_amount);

    accs.vault_ccm_buffer.reload()?;
    let buffer_before = accs.vault_ccm_buffer.amount;

    let unstake_accounts = UnstakeChannel {
        user: accs.vault.to_account_info(),
        channel_config: accs.oracle_channel_config.to_account_info(),
        mint: accs.ccm_mint.to_account_info(),
        stake_pool: accs.oracle_stake_pool.to_account_info(),
        user_stake: accs.oracle_user_stake.to_account_info(),
        vault: accs.oracle_vault.to_account_info(),
        user_token_account: accs.vault_ccm_buffer.to_account_info(),
        nft_mint: accs.oracle_nft_mint.to_account_info(),
        nft_ata: accs.vault_nft_ata.to_account_info(),
        token_program: accs.token_2022_program.to_account_info(),
        associated_token_program: accs.associated_token_program.to_account_info(),
    };

    let unstake_ctx = CpiContext::new_with_signer(
        accs.oracle_program.to_account_info(),
        unstake_accounts,
        signer_seeds,
    );

    token_2022::cpi::unstake_channel(unstake_ctx)?;

    // Measure actual received (net of transfer fees).
    // Using stake_amount directly would cause phantom inflation since
    // the Oracle holds less due to inbound transfer fee.
    accs.vault_ccm_buffer.reload()?;
    let unstaked_received = accs.vault_ccm_buffer.amount
        .checked_sub(buffer_before)
        .ok_or(VaultError::MathOverflow)?;

    msg!("Unstaked {} CCM from Oracle (actual received)", unstaked_received);
    Ok(unstaked_received)
}

/// Pay compound bounty to keeper from claimed rewards.
/// Returns bounty amount paid (0 if none).
#[inline(never)]
fn do_pay_bounty<'info>(
    accs: &Compound<'info>,
    signer_seeds: &[&[&[u8]]],
    rewards_claimed: u64,
    clock_timestamp: i64,
) -> Result<u64> {
    let bounty_paid = (rewards_claimed as u128)
        .checked_mul(COMPOUND_BOUNTY_BPS as u128)
        .ok_or(VaultError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(VaultError::MathOverflow)? as u64;

    if bounty_paid == 0 {
        return Ok(0);
    }

    let bounty_ctx = CpiContext::new_with_signer(
        accs.token_2022_program.to_account_info(),
        TransferChecked {
            from: accs.vault_ccm_buffer.to_account_info(),
            mint: accs.ccm_mint.to_account_info(),
            to: accs.payer_ccm_ata.to_account_info(),
            authority: accs.vault.to_account_info(),
        },
        signer_seeds,
    );
    anchor_spl::token_interface::transfer_checked(
        bounty_ctx,
        bounty_paid,
        accs.ccm_mint.decimals,
    )?;

    emit!(CompoundBountyPaid {
        vault: accs.vault.key(),
        caller: accs.payer.key(),
        ccm_amount: bounty_paid,
        timestamp: clock_timestamp,
    });

    Ok(bounty_paid)
}

/// Stake CCM into Oracle with lock duration.
#[inline(never)]
fn do_stake<'info>(
    accs: &Compound<'info>,
    signer_seeds: &[&[&[u8]]],
    amount: u64,
    lock_slots: u64,
) -> Result<()> {
    msg!("Staking {} CCM in Oracle with {} slot lock", amount, lock_slots);

    let stake_accounts = StakeChannel {
        user: accs.vault.to_account_info(),
        payer: accs.payer.to_account_info(),
        protocol_state: accs.oracle_protocol.to_account_info(),
        channel_config: accs.oracle_channel_config.to_account_info(),
        mint: accs.ccm_mint.to_account_info(),
        stake_pool: accs.oracle_stake_pool.to_account_info(),
        user_stake: accs.oracle_user_stake.to_account_info(),
        vault: accs.oracle_vault.to_account_info(),
        user_token_account: accs.vault_ccm_buffer.to_account_info(),
        nft_mint: accs.oracle_nft_mint.to_account_info(),
        nft_ata: accs.vault_nft_ata.to_account_info(),
        token_program: accs.token_2022_program.to_account_info(),
        associated_token_program: accs.associated_token_program.to_account_info(),
        system_program: accs.system_program.to_account_info(),
        rent: accs.rent.to_account_info(),
    };

    let stake_ctx = CpiContext::new_with_signer(
        accs.oracle_program.to_account_info(),
        stake_accounts,
        signer_seeds,
    );

    token_2022::cpi::stake_channel(stake_ctx, amount, lock_slots)
}

pub fn handler<'info>(mut ctx: Context<'_, '_, 'info, 'info, Compound<'info>>) -> Result<()> {
    let clock = Clock::get()?;

    // Read state into locals — frees ctx.accounts for sequential CPI borrows
    let pending = ctx.accounts.vault.pending_deposits;
    let reserved_for_withdrawals = ctx.accounts.vault.pending_withdrawals;
    let stakeable_pending = pending.saturating_sub(reserved_for_withdrawals);
    let is_active = ctx.accounts.vault_oracle_position.is_active;

    require!(stakeable_pending > 0 || is_active, VaultError::NothingToCompound);

    let channel_config_key = ctx.accounts.vault.channel_config;
    let vault_bump = ctx.accounts.vault.bump;
    let lock_duration_slots = ctx.accounts.vault.lock_duration_slots;

    // Vault signer seeds
    let signer_seeds: &[&[&[u8]]] = &[&[
        VAULT_SEED,
        channel_config_key.as_ref(),
        &[vault_bump],
    ]];

    let mut amount_to_stake = stakeable_pending;
    let mut rewards_claimed: u64 = 0;

    // If there's an active position, roll over: claim → unstake → re-stake
    if is_active {
        let lock_end = ctx.accounts.vault_oracle_position.lock_end_slot;
        if lock_end > clock.slot {
            msg!("Oracle stake still locked until slot {}", lock_end);
            return Err(VaultError::OracleStakeLocked.into());
        }

        // Pre-check for claimable rewards BEFORE the CPI.
        // Solana CPI errors propagate immediately and cannot be caught.
        if has_claimable_rewards(
            &ctx.accounts.oracle_stake_pool,
            &ctx.accounts.oracle_user_stake.to_account_info(),
            clock.slot,
            ctx.accounts.oracle_vault.amount,
        ) {
            rewards_claimed = do_claim_rewards(&mut ctx.accounts, signer_seeds)?;
        } else {
            msg!("No claimable rewards, skipping claim CPI");
        }

        let stake_amount = ctx.accounts.vault_oracle_position.stake_amount;
        let unstaked_received = do_unstake(&mut ctx.accounts, signer_seeds, stake_amount)?;

        amount_to_stake = amount_to_stake
            .checked_add(unstaked_received)
            .ok_or(VaultError::MathOverflow)?
            .checked_add(rewards_claimed)
            .ok_or(VaultError::MathOverflow)?;
    }

    // Pay keeper bounty from claimed rewards only (never from principal)
    if rewards_claimed > 0 && COMPOUND_BOUNTY_BPS > 0 {
        let bounty_paid = do_pay_bounty(
            &ctx.accounts, signer_seeds, rewards_claimed, clock.unix_timestamp,
        )?;
        if bounty_paid > 0 {
            amount_to_stake = amount_to_stake
                .checked_sub(bounty_paid)
                .ok_or(VaultError::MathOverflow)?;
        }
    }

    if amount_to_stake > 0 {
        do_stake(&ctx.accounts, signer_seeds, amount_to_stake, lock_duration_slots)?;
    }

    // Update vault state
    let vault = &mut ctx.accounts.vault;
    vault.total_staked = amount_to_stake;
    vault.pending_deposits = vault
        .pending_deposits
        .checked_sub(stakeable_pending)
        .ok_or(VaultError::MathOverflow)?;
    vault.compound_count = vault.compound_count.saturating_add(1);
    vault.last_compound_slot = clock.slot;

    // Update position state
    let position = &mut ctx.accounts.vault_oracle_position;
    position.is_active = amount_to_stake > 0;
    position.stake_amount = amount_to_stake;
    position.lock_end_slot = clock.slot.saturating_add(lock_duration_slots);
    position.oracle_user_stake = ctx.accounts.oracle_user_stake.key();
    position.oracle_nft_mint = ctx.accounts.oracle_nft_mint.key();
    position.oracle_nft_ata = ctx.accounts.vault_nft_ata.key();

    // Update exchange rate oracle if provided as remaining account.
    // Best-effort: oracle update failure must never block compounding.
    let _ = maybe_update_exchange_rate_oracle(vault, ctx.remaining_accounts);

    emit!(Compounded {
        vault: vault.key(),
        pending_staked: stakeable_pending,
        total_staked: vault.total_staked,
        rewards_claimed,
        compound_count: vault.compound_count,
        caller: ctx.accounts.payer.key(),
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Compounded: staked={} (reserved {} for withdrawals), total_staked={}, lock_end={}",
        stakeable_pending,
        reserved_for_withdrawals,
        vault.total_staked,
        position.lock_end_slot
    );

    Ok(())
}
