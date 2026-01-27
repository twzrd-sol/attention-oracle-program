use crate::{
    constants::{
        calculate_boost_bps, BOOST_PRECISION, MAX_LOCK_SLOTS, MIN_STAKE_AMOUNT, PROTOCOL_SEED,
        REWARD_PRECISION, STAKE_POOL_SEED, STAKE_VAULT_SEED, USER_STAKE_SEED,
    },
    errors::OracleError,
    state::{ProtocolState, StakePool, UserStake},
};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

// =============================================================================
// EVENTS
// =============================================================================

#[event]
pub struct StakePoolInitialized {
    pub mint: Pubkey,
    pub authority: Pubkey,
    pub reward_rate: u64,
    pub timestamp: i64,
}

#[event]
pub struct Staked {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub lock_end_slot: u64,
    pub total_staked: u64,
    pub timestamp: i64,
}

#[event]
pub struct Unstaked {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub remaining_stake: u64,
    pub timestamp: i64,
}

#[event]
pub struct DelegationChanged {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub old_subject: Option<[u8; 32]>,
    pub new_subject: Option<[u8; 32]>,
    pub staked_amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct RewardsClaimed {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

// =============================================================================
// INITIALIZE STAKE POOL
// =============================================================================

#[derive(Accounts)]
pub struct InitializeStakePool<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// Protocol state (verifies admin authority)
    #[account(
        seeds = [PROTOCOL_SEED, mint.key().as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// CCM Token-2022 mint
    pub mint: InterfaceAccount<'info, Mint>,

    /// Stake pool PDA (mint-keyed)
    #[account(
        init,
        payer = admin,
        space = StakePool::LEN,
        seeds = [STAKE_POOL_SEED, mint.key().as_ref()],
        bump,
    )]
    pub stake_pool: Account<'info, StakePool>,

    /// Stake vault token account (holds staked CCM)
    #[account(
        init,
        payer = admin,
        token::mint = mint,
        token::authority = stake_pool,
        seeds = [STAKE_VAULT_SEED, mint.key().as_ref()],
        bump,
    )]
    pub stake_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_stake_pool(ctx: Context<InitializeStakePool>, reward_rate: u64) -> Result<()> {
    let pool = &mut ctx.accounts.stake_pool;
    let ts = Clock::get()?.unix_timestamp;

    pool.version = 1;
    pool.bump = ctx.bumps.stake_pool;
    pool.mint = ctx.accounts.mint.key();
    pool.total_staked = 0;
    pool.acc_reward_per_share = 0;
    pool.last_reward_time = ts;
    pool.reward_rate = reward_rate;
    pool.authority = ctx.accounts.admin.key();
    pool.total_weighted_stake = 0;
    pool._reserved = [0u8; 56];

    emit!(StakePoolInitialized {
        mint: pool.mint,
        authority: pool.authority,
        reward_rate,
        timestamp: ts,
    });

    msg!("Stake pool initialized: mint={}, rate={}/sec", pool.mint, reward_rate);
    Ok(())
}

// =============================================================================
// STAKE
// =============================================================================

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// CCM Token-2022 mint
    pub mint: InterfaceAccount<'info, Mint>,

    /// Protocol state (for pause check)
    #[account(
        seeds = [PROTOCOL_SEED, mint.key().as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Stake pool PDA
    #[account(
        mut,
        seeds = [STAKE_POOL_SEED, mint.key().as_ref()],
        bump = stake_pool.bump,
        constraint = stake_pool.mint == mint.key() @ OracleError::InvalidMint,
    )]
    pub stake_pool: Account<'info, StakePool>,

    /// User stake PDA (created if not exists)
    #[account(
        init_if_needed,
        payer = user,
        space = UserStake::LEN,
        seeds = [USER_STAKE_SEED, user.key().as_ref(), mint.key().as_ref()],
        bump,
    )]
    pub user_stake: Account<'info, UserStake>,

    /// User's CCM token account (source)
    #[account(
        mut,
        token::mint = mint,
        token::authority = user,
    )]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    /// Stake vault (destination)
    #[account(
        mut,
        token::mint = mint,
        token::authority = stake_pool,
        seeds = [STAKE_VAULT_SEED, mint.key().as_ref()],
        bump,
    )]
    pub stake_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn stake<'info>(
    ctx: Context<'_, '_, '_, 'info, Stake<'info>>,
    amount: u64,
    lock_slots: u64,
) -> Result<()> {
    // Block staking while paused
    require!(!ctx.accounts.protocol_state.paused, OracleError::ProtocolPaused);

    require!(amount >= MIN_STAKE_AMOUNT, OracleError::StakeBelowMinimum);
    require!(lock_slots <= MAX_LOCK_SLOTS, OracleError::LockPeriodTooLong);

    let pool = &mut ctx.accounts.stake_pool;
    let user_stake = &mut ctx.accounts.user_stake;
    let clock = Clock::get()?;
    let ts = clock.unix_timestamp;
    let current_slot = clock.slot;

    // Update pool rewards before state change
    update_pool_rewards(pool, ts)?;

    // Initialize user_stake if new
    if user_stake.version == 0 {
        user_stake.version = 1;
        user_stake.bump = ctx.bumps.user_stake;
        user_stake.user = ctx.accounts.user.key();
        user_stake.mint = ctx.accounts.mint.key();
        user_stake.staked_amount = 0;
        user_stake.delegated_subject = None;
        user_stake.lock_end_slot = 0;
        user_stake.reward_debt = 0;
        user_stake.pending_rewards = 0;
        user_stake.last_action_time = ts;
        user_stake.weighted_stake = 0;
        user_stake._reserved = [0u8; 24];
    }

    // Capture old weighted stake BEFORE any modifications
    let old_weight = user_stake.get_effective_stake();

    // Harvest pending rewards before adding new stake
    harvest_pending(user_stake, pool)?;

    // Record vault balance BEFORE transfer to compute actual received amount
    // (Token-2022 transfer fees mean vault receives less than `amount`)
    let vault_balance_before = ctx.accounts.stake_vault.amount;

    // Transfer tokens to vault (forward remaining accounts for Token-2022 hooks/extensions).
    let token_program = ctx.accounts.token_program.to_account_info();
    let from = ctx.accounts.user_token_account.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let to = ctx.accounts.stake_vault.to_account_info();
    let authority = ctx.accounts.user.to_account_info();
    let no_signer_seeds: &[&[&[u8]]] = &[];
    crate::transfer_checked_with_remaining(
        &token_program,
        &from,
        &mint,
        &to,
        &authority,
        amount,
        ctx.accounts.mint.decimals,
        no_signer_seeds,
        ctx.remaining_accounts,
    )?;

    // Reload vault to get post-transfer balance
    ctx.accounts.stake_vault.reload()?;
    let vault_balance_after = ctx.accounts.stake_vault.amount;

    // Calculate actual received amount (accounts for transfer fees)
    let actual_received = vault_balance_after
        .checked_sub(vault_balance_before)
        .ok_or(OracleError::MathOverflow)?;

    // Update raw staked amount
    user_stake.staked_amount = user_stake
        .staked_amount
        .checked_add(actual_received)
        .ok_or(OracleError::MathOverflow)?;

    // Set lock if provided (extend only, never reduce)
    let new_lock_end = current_slot.saturating_add(lock_slots);
    if new_lock_end > user_stake.lock_end_slot {
        user_stake.lock_end_slot = new_lock_end;
    }

    // Calculate new boost and weighted stake
    let new_boost = calculate_boost_bps(user_stake.lock_end_slot, current_slot);
    let new_weight = (user_stake.staked_amount as u128)
        .checked_mul(new_boost as u128)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(BOOST_PRECISION as u128)
        .ok_or(OracleError::MathOverflow)? as u64;

    // Update pool totals: remove old weight, add new weight
    let effective_total = pool.get_effective_total();
    pool.total_weighted_stake = effective_total
        .checked_sub(old_weight)
        .ok_or(OracleError::MathOverflow)?
        .checked_add(new_weight)
        .ok_or(OracleError::MathOverflow)?;

    // Update raw total_staked for backwards compat
    pool.total_staked = pool
        .total_staked
        .checked_add(actual_received)
        .ok_or(OracleError::MathOverflow)?;

    // Save user's new weighted stake
    user_stake.weighted_stake = new_weight;
    user_stake.last_action_time = ts;
    user_stake.reward_debt = calculate_reward_debt(new_weight, pool.acc_reward_per_share)?;

    emit!(Staked {
        user: ctx.accounts.user.key(),
        mint: ctx.accounts.mint.key(),
        amount: actual_received, // Emit actual received, not input amount
        lock_end_slot: user_stake.lock_end_slot,
        total_staked: user_stake.staked_amount,
        timestamp: ts,
    });

    msg!(
        "Staked {} CCM (requested {}), total={}, lock_end={}",
        actual_received,
        amount,
        user_stake.staked_amount,
        user_stake.lock_end_slot
    );
    Ok(())
}

// =============================================================================
// UNSTAKE
// =============================================================================

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// CCM Token-2022 mint
    pub mint: InterfaceAccount<'info, Mint>,

    /// Protocol state (for pause check)
    #[account(
        seeds = [PROTOCOL_SEED, mint.key().as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Stake pool PDA
    #[account(
        mut,
        seeds = [STAKE_POOL_SEED, mint.key().as_ref()],
        bump = stake_pool.bump,
    )]
    pub stake_pool: Account<'info, StakePool>,

    /// User stake PDA
    #[account(
        mut,
        seeds = [USER_STAKE_SEED, user.key().as_ref(), mint.key().as_ref()],
        bump = user_stake.bump,
        constraint = user_stake.user == user.key() @ OracleError::Unauthorized,
    )]
    pub user_stake: Account<'info, UserStake>,

    /// User's CCM token account (destination)
    #[account(
        mut,
        token::mint = mint,
        token::authority = user,
    )]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    /// Stake vault (source)
    #[account(
        mut,
        token::mint = mint,
        token::authority = stake_pool,
        seeds = [STAKE_VAULT_SEED, mint.key().as_ref()],
        bump,
    )]
    pub stake_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn unstake<'info>(ctx: Context<'_, '_, '_, 'info, Unstake<'info>>, amount: u64) -> Result<()> {
    // Block unstaking while paused
    require!(!ctx.accounts.protocol_state.paused, OracleError::ProtocolPaused);

    let clock = Clock::get()?;
    let ts = clock.unix_timestamp;
    let current_slot = clock.slot;

    // Validate lock period
    require!(
        current_slot >= ctx.accounts.user_stake.lock_end_slot,
        OracleError::TokensLocked
    );

    require!(
        ctx.accounts.user_stake.staked_amount >= amount,
        OracleError::InsufficientStake
    );

    // Capture old weighted stake BEFORE any modifications
    let old_weight = ctx.accounts.user_stake.get_effective_stake();

    // Extract values needed for CPI before mutable borrows
    let mint_key = ctx.accounts.mint.key();
    let pool_bump = ctx.accounts.stake_pool.bump;
    let decimals = ctx.accounts.mint.decimals;

    // Update pool rewards before state change
    {
        let pool = &mut ctx.accounts.stake_pool;
        update_pool_rewards(pool, ts)?;
    }

    // Harvest pending rewards
    {
        let pool = &ctx.accounts.stake_pool;
        let user_stake = &mut ctx.accounts.user_stake;
        let pending = calculate_pending_rewards(user_stake, pool)?;
        if pending > 0 {
            user_stake.pending_rewards = user_stake
                .pending_rewards
                .checked_add(pending)
                .ok_or(OracleError::MathOverflow)?;
        }
    }

    // Transfer tokens back to user via PDA signer
    let pool_seeds: &[&[u8]] = &[STAKE_POOL_SEED, mint_key.as_ref(), &[pool_bump]];
    let signer = &[pool_seeds];

    // Transfer tokens back to user (forward remaining accounts for Token-2022 hooks/extensions).
    let token_program = ctx.accounts.token_program.to_account_info();
    let from = ctx.accounts.stake_vault.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let to = ctx.accounts.user_token_account.to_account_info();
    let authority = ctx.accounts.stake_pool.to_account_info();
    crate::transfer_checked_with_remaining(
        &token_program,
        &from,
        &mint,
        &to,
        &authority,
        amount,
        decimals,
        signer,
        ctx.remaining_accounts,
    )?;

    // Update state after CPI
    let acc_reward_per_share = ctx.accounts.stake_pool.acc_reward_per_share;

    let user_stake = &mut ctx.accounts.user_stake;
    user_stake.staked_amount = user_stake
        .staked_amount
        .checked_sub(amount)
        .ok_or(OracleError::MathOverflow)?;

    // Calculate new boost and weighted stake (lock may have expired)
    let new_boost = calculate_boost_bps(user_stake.lock_end_slot, current_slot);
    let new_weight = (user_stake.staked_amount as u128)
        .checked_mul(new_boost as u128)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(BOOST_PRECISION as u128)
        .ok_or(OracleError::MathOverflow)? as u64;

    // Save user's new weighted stake
    user_stake.weighted_stake = new_weight;
    user_stake.last_action_time = ts;
    user_stake.reward_debt = calculate_reward_debt(new_weight, acc_reward_per_share)?;

    let remaining_stake = user_stake.staked_amount;

    let pool = &mut ctx.accounts.stake_pool;

    // Update pool weighted total: remove old weight, add new weight
    let effective_total = pool.get_effective_total();
    pool.total_weighted_stake = effective_total
        .checked_sub(old_weight)
        .ok_or(OracleError::MathOverflow)?
        .checked_add(new_weight)
        .ok_or(OracleError::MathOverflow)?;

    // Update raw total_staked for backwards compat
    pool.total_staked = pool
        .total_staked
        .checked_sub(amount)
        .ok_or(OracleError::MathOverflow)?;

    emit!(Unstaked {
        user: ctx.accounts.user.key(),
        mint: ctx.accounts.mint.key(),
        amount,
        remaining_stake,
        timestamp: ts,
    });

    msg!(
        "Unstaked {} CCM, remaining={}",
        amount,
        user_stake.staked_amount
    );
    Ok(())
}

// =============================================================================
// DELEGATE STAKE
// =============================================================================

#[derive(Accounts)]
pub struct DelegateStake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// CCM Token-2022 mint
    pub mint: InterfaceAccount<'info, Mint>,

    /// Protocol state (for pause check)
    #[account(
        seeds = [PROTOCOL_SEED, mint.key().as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Stake pool PDA (for reward updates)
    #[account(
        mut,
        seeds = [STAKE_POOL_SEED, mint.key().as_ref()],
        bump = stake_pool.bump,
    )]
    pub stake_pool: Account<'info, StakePool>,

    /// User stake PDA
    #[account(
        mut,
        seeds = [USER_STAKE_SEED, user.key().as_ref(), mint.key().as_ref()],
        bump = user_stake.bump,
        constraint = user_stake.user == user.key() @ OracleError::Unauthorized,
        constraint = user_stake.staked_amount > 0 @ OracleError::InsufficientStake,
    )]
    pub user_stake: Account<'info, UserStake>,
}

pub fn delegate_stake(ctx: Context<DelegateStake>, subject_id: Option<[u8; 32]>) -> Result<()> {
    // Block delegation while paused
    require!(!ctx.accounts.protocol_state.paused, OracleError::ProtocolPaused);

    let pool = &mut ctx.accounts.stake_pool;
    let user_stake = &mut ctx.accounts.user_stake;
    let ts = Clock::get()?.unix_timestamp;

    // Update pool rewards
    update_pool_rewards(pool, ts)?;

    let old_subject = user_stake.delegated_subject;
    user_stake.delegated_subject = subject_id;
    user_stake.last_action_time = ts;

    emit!(DelegationChanged {
        user: ctx.accounts.user.key(),
        mint: ctx.accounts.mint.key(),
        old_subject,
        new_subject: subject_id,
        staked_amount: user_stake.staked_amount,
        timestamp: ts,
    });

    msg!(
        "Delegation updated: staked={}, has_subject={}",
        user_stake.staked_amount,
        subject_id.is_some()
    );
    Ok(())
}

// =============================================================================
// CLAIM STAKE REWARDS
// =============================================================================

#[derive(Accounts)]
pub struct ClaimStakeRewards<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// CCM Token-2022 mint
    pub mint: InterfaceAccount<'info, Mint>,

    /// Protocol state (for treasury access)
    #[account(
        seeds = [PROTOCOL_SEED, mint.key().as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Stake pool PDA
    #[account(
        mut,
        seeds = [STAKE_POOL_SEED, mint.key().as_ref()],
        bump = stake_pool.bump,
    )]
    pub stake_pool: Account<'info, StakePool>,

    /// User stake PDA
    #[account(
        mut,
        seeds = [USER_STAKE_SEED, user.key().as_ref(), mint.key().as_ref()],
        bump = user_stake.bump,
        constraint = user_stake.user == user.key() @ OracleError::Unauthorized,
    )]
    pub user_stake: Account<'info, UserStake>,

    /// User's CCM token account (destination for rewards)
    #[account(
        mut,
        token::mint = mint,
        token::authority = user,
    )]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    /// Treasury ATA (source of rewards)
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = protocol_state,
        associated_token::token_program = token_program
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_stake_rewards<'info>(
    ctx: Context<'_, '_, '_, 'info, ClaimStakeRewards<'info>>,
) -> Result<()> {
    // Block reward claims while paused
    require!(!ctx.accounts.protocol_state.paused, OracleError::ProtocolPaused);

    let pool = &mut ctx.accounts.stake_pool;
    let user_stake = &mut ctx.accounts.user_stake;
    let ts = Clock::get()?.unix_timestamp;

    // Update pool rewards
    update_pool_rewards(pool, ts)?;

    // Calculate pending rewards
    let pending = calculate_pending_rewards(user_stake, pool)?;
    let total_rewards = user_stake
        .pending_rewards
        .checked_add(pending)
        .ok_or(OracleError::MathOverflow)?;

    require!(total_rewards > 0, OracleError::NoPendingRewards);

    // Transfer rewards from treasury via protocol_state PDA
    let mint_key = ctx.accounts.mint.key();
    let protocol_seeds: &[&[u8]] = &[
        PROTOCOL_SEED,
        mint_key.as_ref(),
        &[ctx.accounts.protocol_state.bump],
    ];
    let signer = &[protocol_seeds];

    // Transfer rewards from treasury (forward remaining accounts for Token-2022 hooks/extensions).
    let token_program = ctx.accounts.token_program.to_account_info();
    let from = ctx.accounts.treasury_ata.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let to = ctx.accounts.user_token_account.to_account_info();
    let authority = ctx.accounts.protocol_state.to_account_info();
    crate::transfer_checked_with_remaining(
        &token_program,
        &from,
        &mint,
        &to,
        &authority,
        total_rewards,
        ctx.accounts.mint.decimals,
        signer,
        ctx.remaining_accounts,
    )?;

    // Reset pending rewards and update debt using weighted stake
    user_stake.pending_rewards = 0;
    user_stake.reward_debt = calculate_reward_debt(user_stake.get_effective_stake(), pool.acc_reward_per_share)?;
    user_stake.last_action_time = ts;

    emit!(RewardsClaimed {
        user: ctx.accounts.user.key(),
        mint: ctx.accounts.mint.key(),
        amount: total_rewards,
        timestamp: ts,
    });

    msg!("Claimed {} CCM rewards", total_rewards);
    Ok(())
}

// =============================================================================
// HELPER FUNCTIONS (MasterChef-style accounting)
// =============================================================================

/// Update accumulated rewards per share based on time elapsed.
/// Uses total_weighted_stake for boost-aware reward distribution.
fn update_pool_rewards(pool: &mut StakePool, current_time: i64) -> Result<()> {
    let effective_total = pool.get_effective_total();
    if effective_total == 0 {
        pool.last_reward_time = current_time;
        return Ok(());
    }

    let time_delta_i64 = current_time.saturating_sub(pool.last_reward_time);
    let time_delta: u128 = u128::try_from(time_delta_i64).unwrap_or(0);

    if time_delta == 0 {
        return Ok(());
    }

    // rewards = time_delta * reward_rate
    let rewards = time_delta
        .checked_mul(pool.reward_rate as u128)
        .ok_or(OracleError::MathOverflow)?;

    // acc_reward_per_share += (rewards * PRECISION) / total_weighted_stake
    let reward_per_share = rewards
        .checked_mul(REWARD_PRECISION)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(effective_total as u128)
        .ok_or(OracleError::MathOverflow)?;

    pool.acc_reward_per_share = pool
        .acc_reward_per_share
        .checked_add(reward_per_share)
        .ok_or(OracleError::MathOverflow)?;

    pool.last_reward_time = current_time;
    Ok(())
}

/// Calculate pending rewards for a user.
/// Uses effective (weighted) stake for boost-aware rewards.
fn calculate_pending_rewards(user_stake: &UserStake, pool: &StakePool) -> Result<u64> {
    let effective_stake = user_stake.get_effective_stake();
    if effective_stake == 0 {
        return Ok(0);
    }

    // accumulated = (weighted_stake * acc_reward_per_share) / PRECISION
    let accumulated = (effective_stake as u128)
        .checked_mul(pool.acc_reward_per_share)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(REWARD_PRECISION)
        .ok_or(OracleError::MathOverflow)?;

    // pending = accumulated - reward_debt
    let pending = accumulated
        .saturating_sub(user_stake.reward_debt)
        .min(u64::MAX as u128) as u64;

    Ok(pending)
}

/// Calculate reward debt for weighted stake amount.
/// Uses weighted_amount (not raw staked_amount) for boost-aware accounting.
fn calculate_reward_debt(weighted_amount: u64, acc_reward_per_share: u128) -> Result<u128> {
    let debt = (weighted_amount as u128)
        .checked_mul(acc_reward_per_share)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(REWARD_PRECISION)
        .ok_or(OracleError::MathOverflow)?;
    Ok(debt)
}

/// Harvest and accumulate pending rewards (internal helper)
fn harvest_pending(user_stake: &mut UserStake, pool: &StakePool) -> Result<()> {
    let pending = calculate_pending_rewards(user_stake, pool)?;
    if pending > 0 {
        user_stake.pending_rewards = user_stake
            .pending_rewards
            .checked_add(pending)
            .ok_or(OracleError::MathOverflow)?;
    }
    Ok(())
}
