use crate::{
    constants::{
        MAX_LOCK_SLOTS, MIN_STAKE_AMOUNT, PROTOCOL_SEED, REWARD_PRECISION, STAKE_POOL_SEED,
        STAKE_VAULT_SEED, USER_STAKE_SEED,
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
    pool._reserved = [0u8; 64];

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

pub fn stake(ctx: Context<Stake>, amount: u64, lock_slots: u64) -> Result<()> {
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
        user_stake._reserved = [0u8; 32];
    }

    // Harvest pending rewards before adding new stake
    harvest_pending(user_stake, pool)?;

    // Transfer tokens to vault
    anchor_spl::token_interface::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token_interface::TransferChecked {
                from: ctx.accounts.user_token_account.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.stake_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    // Update state
    user_stake.staked_amount = user_stake
        .staked_amount
        .checked_add(amount)
        .ok_or(OracleError::MathOverflow)?;

    // Set lock if provided (extend only, never reduce)
    let new_lock_end = current_slot.saturating_add(lock_slots);
    if new_lock_end > user_stake.lock_end_slot {
        user_stake.lock_end_slot = new_lock_end;
    }

    user_stake.last_action_time = ts;
    user_stake.reward_debt = calculate_reward_debt(user_stake.staked_amount, pool.acc_reward_per_share)?;

    pool.total_staked = pool
        .total_staked
        .checked_add(amount)
        .ok_or(OracleError::MathOverflow)?;

    emit!(Staked {
        user: ctx.accounts.user.key(),
        mint: ctx.accounts.mint.key(),
        amount,
        lock_end_slot: user_stake.lock_end_slot,
        total_staked: user_stake.staked_amount,
        timestamp: ts,
    });

    msg!(
        "Staked {} CCM, total={}, lock_end={}",
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

pub fn unstake(ctx: Context<Unstake>, amount: u64) -> Result<()> {
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

    anchor_spl::token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token_interface::TransferChecked {
                from: ctx.accounts.stake_vault.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.stake_pool.to_account_info(),
            },
            signer,
        ),
        amount,
        decimals,
    )?;

    // Update state after CPI
    let acc_reward_per_share = ctx.accounts.stake_pool.acc_reward_per_share;

    let user_stake = &mut ctx.accounts.user_stake;
    user_stake.staked_amount = user_stake
        .staked_amount
        .checked_sub(amount)
        .ok_or(OracleError::MathOverflow)?;

    user_stake.last_action_time = ts;
    user_stake.reward_debt = calculate_reward_debt(user_stake.staked_amount, acc_reward_per_share)?;

    let remaining_stake = user_stake.staked_amount;

    let pool = &mut ctx.accounts.stake_pool;
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

pub fn claim_stake_rewards(ctx: Context<ClaimStakeRewards>) -> Result<()> {
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

    anchor_spl::token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token_interface::TransferChecked {
                from: ctx.accounts.treasury_ata.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.protocol_state.to_account_info(),
            },
            signer,
        ),
        total_rewards,
        ctx.accounts.mint.decimals,
    )?;

    // Reset pending rewards and update debt
    user_stake.pending_rewards = 0;
    user_stake.reward_debt = calculate_reward_debt(user_stake.staked_amount, pool.acc_reward_per_share)?;
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

/// Update accumulated rewards per share based on time elapsed
fn update_pool_rewards(pool: &mut StakePool, current_time: i64) -> Result<()> {
    if pool.total_staked == 0 {
        pool.last_reward_time = current_time;
        return Ok(());
    }

    let time_delta = current_time
        .saturating_sub(pool.last_reward_time)
        .max(0) as u128;

    if time_delta == 0 {
        return Ok(());
    }

    // rewards = time_delta * reward_rate
    let rewards = time_delta
        .checked_mul(pool.reward_rate as u128)
        .ok_or(OracleError::MathOverflow)?;

    // acc_reward_per_share += (rewards * PRECISION) / total_staked
    let reward_per_share = rewards
        .checked_mul(REWARD_PRECISION)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(pool.total_staked as u128)
        .ok_or(OracleError::MathOverflow)?;

    pool.acc_reward_per_share = pool
        .acc_reward_per_share
        .checked_add(reward_per_share)
        .ok_or(OracleError::MathOverflow)?;

    pool.last_reward_time = current_time;
    Ok(())
}

/// Calculate pending rewards for a user
fn calculate_pending_rewards(user_stake: &UserStake, pool: &StakePool) -> Result<u64> {
    if user_stake.staked_amount == 0 {
        return Ok(0);
    }

    // accumulated = (staked_amount * acc_reward_per_share) / PRECISION
    let accumulated = (user_stake.staked_amount as u128)
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

/// Calculate reward debt for current stake
fn calculate_reward_debt(staked_amount: u64, acc_reward_per_share: u128) -> Result<u128> {
    let debt = (staked_amount as u128)
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
