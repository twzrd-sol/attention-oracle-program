//! Channel staking instructions for the Attention Oracle Protocol.
//!
//! Allows users to stake tokens on channels, earning boosted rewards
//! based on lock duration. Stake positions can optionally be represented
//! as transferable NFTs.

use crate::constants::{
    calculate_boost_bps, BOOST_PRECISION, CHANNEL_STAKE_POOL_SEED, CHANNEL_STAKE_VAULT_SEED,
    MAX_LOCK_SLOTS, MIN_STAKE_AMOUNT, PROTOCOL_SEED, USER_CHANNEL_STAKE_SEED,
};
use crate::errors::OracleError;
use crate::state::{ChannelConfigV2, ChannelStakePool, ProtocolState, UserChannelStake};
use anchor_lang::prelude::*;
use anchor_spl::token_2022::spl_token_2022;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

// =============================================================================
// INITIALIZE CHANNEL STAKE POOL
// =============================================================================

#[derive(Accounts)]
#[instruction(channel: String)]
pub struct InitializeChannelStakePool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Protocol state (for authority check)
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = payer.key() == protocol_state.admin || payer.key() == protocol_state.publisher @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel config must exist
    pub channel_config: Account<'info, ChannelConfigV2>,

    /// Token mint
    #[account(
        constraint = mint.key() == protocol_state.mint @ OracleError::InvalidMint,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    /// Stake pool PDA
    #[account(
        init,
        payer = payer,
        space = ChannelStakePool::LEN,
        seeds = [CHANNEL_STAKE_POOL_SEED, mint.key().as_ref(), channel_config.subject.as_ref()],
        bump,
    )]
    pub stake_pool: Account<'info, ChannelStakePool>,

    /// Vault to hold staked tokens
    #[account(
        init,
        payer = payer,
        token::mint = mint,
        token::authority = stake_pool,
        seeds = [CHANNEL_STAKE_VAULT_SEED, mint.key().as_ref(), channel_config.subject.as_ref()],
        bump,
    )]
    pub stake_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_channel_stake_pool(
    ctx: Context<InitializeChannelStakePool>,
    _channel: String,
) -> Result<()> {
    let pool = &mut ctx.accounts.stake_pool;
    let clock = Clock::get()?;

    pool.version = 1;
    pool.bump = ctx.bumps.stake_pool;
    pool.mint = ctx.accounts.mint.key();
    pool.subject = ctx.accounts.channel_config.subject;
    pool.authority = ctx.accounts.protocol_state.admin;
    pool.total_staked = 0;
    pool.total_weighted_stake = 0;
    pool.staker_count = 0;
    pool.min_stake_amount = MIN_STAKE_AMOUNT;
    pool.max_lock_slots = MAX_LOCK_SLOTS;
    pool.created_at = clock.unix_timestamp;
    pool.last_update = clock.unix_timestamp;

    msg!(
        "Channel stake pool initialized: subject={}, min_stake={}",
        ctx.accounts.channel_config.subject,
        MIN_STAKE_AMOUNT
    );

    Ok(())
}

// =============================================================================
// STAKE CHANNEL
// =============================================================================

#[derive(Accounts)]
#[instruction(channel: String)]
pub struct StakeChannel<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// Protocol state
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ OracleError::ProtocolPaused,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel config
    pub channel_config: Account<'info, ChannelConfigV2>,

    /// Token mint
    #[account(
        constraint = mint.key() == protocol_state.mint @ OracleError::InvalidMint,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    /// Stake pool
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_POOL_SEED, mint.key().as_ref(), channel_config.subject.as_ref()],
        bump = stake_pool.bump,
    )]
    pub stake_pool: Account<'info, ChannelStakePool>,

    /// User's stake position (init if needed)
    #[account(
        init_if_needed,
        payer = user,
        space = UserChannelStake::LEN,
        seeds = [USER_CHANNEL_STAKE_SEED, user.key().as_ref(), mint.key().as_ref(), channel_config.subject.as_ref()],
        bump,
    )]
    pub user_stake: Account<'info, UserChannelStake>,

    /// User's token account
    #[account(
        mut,
        constraint = user_token_account.owner == user.key() @ OracleError::Unauthorized,
        constraint = user_token_account.mint == mint.key() @ OracleError::InvalidMint,
    )]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    /// Stake vault
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_VAULT_SEED, mint.key().as_ref(), channel_config.subject.as_ref()],
        bump,
    )]
    pub stake_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn stake_channel(
    ctx: Context<StakeChannel>,
    _channel: String,
    amount: u64,
    lock_slots: u64,
) -> Result<()> {
    let clock = Clock::get()?;
    let current_slot = clock.slot;

    // Capture keys before mutable borrows
    let pool_key = ctx.accounts.stake_pool.key();
    let user_key = ctx.accounts.user.key();
    let mint_key = ctx.accounts.mint.key();
    let subject = ctx.accounts.channel_config.subject;
    let min_stake = ctx.accounts.stake_pool.min_stake_amount;
    let max_lock = ctx.accounts.stake_pool.max_lock_slots;

    let pool = &mut ctx.accounts.stake_pool;
    let user_stake = &mut ctx.accounts.user_stake;

    // Validate amount
    require!(amount >= min_stake, OracleError::StakeBelowMinimum);

    // Validate lock period
    require!(lock_slots <= max_lock, OracleError::LockPeriodTooLong);

    // Calculate new lock end (only extend, never reduce)
    let new_lock_end = current_slot.saturating_add(lock_slots);
    let effective_lock_end = if user_stake.staked_amount > 0 {
        // Existing position - can only extend lock
        require!(
            new_lock_end >= user_stake.lock_end_slot,
            OracleError::LockReductionNotAllowed
        );
        new_lock_end.max(user_stake.lock_end_slot)
    } else {
        // New position
        new_lock_end
    };

    // Capture old weighted stake for pool adjustment
    let old_weighted = user_stake.weighted_stake;

    // Transfer tokens to vault
    let transfer_ix = spl_token_2022::instruction::transfer_checked(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.user_token_account.key(),
        &mint_key,
        &ctx.accounts.stake_vault.key(),
        &user_key,
        &[],
        amount,
        ctx.accounts.mint.decimals,
    )?;

    anchor_lang::solana_program::program::invoke(
        &transfer_ix,
        &[
            ctx.accounts.user_token_account.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.stake_vault.to_account_info(),
            ctx.accounts.user.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ],
    )?;

    // Update user stake
    let is_new_staker = user_stake.staked_amount == 0;

    if is_new_staker {
        user_stake.version = 1;
        user_stake.bump = ctx.bumps.user_stake;
        user_stake.user = user_key;
        user_stake.mint = mint_key;
        user_stake.subject = subject;
        user_stake.pool = pool_key;
        user_stake.staked_at = clock.unix_timestamp;
        user_stake.nft_mint = None;
    }

    user_stake.staked_amount = user_stake
        .staked_amount
        .checked_add(amount)
        .ok_or(OracleError::MathOverflow)?;
    user_stake.lock_end_slot = effective_lock_end;
    user_stake.last_action = clock.unix_timestamp;

    // Calculate new weighted stake with boost
    let boost_bps = calculate_boost_bps(effective_lock_end, current_slot);
    user_stake.weighted_stake = user_stake
        .staked_amount
        .checked_mul(boost_bps)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(BOOST_PRECISION)
        .ok_or(OracleError::MathOverflow)?;

    // Update pool totals
    pool.total_staked = pool
        .total_staked
        .checked_add(amount)
        .ok_or(OracleError::MathOverflow)?;

    pool.total_weighted_stake = pool
        .total_weighted_stake
        .checked_sub(old_weighted)
        .ok_or(OracleError::MathOverflow)?
        .checked_add(user_stake.weighted_stake)
        .ok_or(OracleError::MathOverflow)?;

    if is_new_staker {
        pool.staker_count = pool.staker_count.saturating_add(1);
    }

    pool.last_update = clock.unix_timestamp;

    msg!(
        "Staked {} tokens on channel, lock_end={}, boost={}bps, weighted={}",
        amount,
        effective_lock_end,
        boost_bps,
        user_stake.weighted_stake
    );

    Ok(())
}

// =============================================================================
// UNSTAKE CHANNEL
// =============================================================================

#[derive(Accounts)]
#[instruction(channel: String)]
pub struct UnstakeChannel<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// Protocol state
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel config
    pub channel_config: Account<'info, ChannelConfigV2>,

    /// Token mint
    #[account(
        constraint = mint.key() == protocol_state.mint @ OracleError::InvalidMint,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    /// Stake pool
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_POOL_SEED, mint.key().as_ref(), channel_config.subject.as_ref()],
        bump = stake_pool.bump,
    )]
    pub stake_pool: Account<'info, ChannelStakePool>,

    /// User's stake position
    #[account(
        mut,
        seeds = [USER_CHANNEL_STAKE_SEED, user_stake.user.as_ref(), mint.key().as_ref(), channel_config.subject.as_ref()],
        bump = user_stake.bump,
        constraint = user_stake.user == user.key() || is_nft_holder(&user_stake, &user.key(), &nft_token_account) @ OracleError::Unauthorized,
    )]
    pub user_stake: Account<'info, UserChannelStake>,

    /// User's token account (receives unstaked tokens)
    #[account(
        mut,
        constraint = user_token_account.owner == user.key() @ OracleError::Unauthorized,
        constraint = user_token_account.mint == mint.key() @ OracleError::InvalidMint,
    )]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    /// Stake vault
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_VAULT_SEED, mint.key().as_ref(), channel_config.subject.as_ref()],
        bump,
    )]
    pub stake_vault: InterfaceAccount<'info, TokenAccount>,

    /// Optional: NFT token account if position has NFT
    /// CHECK: Validated in constraint
    pub nft_token_account: Option<AccountInfo<'info>>,

    pub token_program: Interface<'info, TokenInterface>,
}

/// Check if caller holds the NFT for this position
fn is_nft_holder(
    user_stake: &UserChannelStake,
    caller: &Pubkey,
    nft_account: &Option<AccountInfo>,
) -> bool {
    match (&user_stake.nft_mint, nft_account) {
        (Some(nft_mint), Some(nft_acc)) => {
            // Parse token account and verify ownership
            if let Ok(data) = nft_acc.try_borrow_data() {
                if data.len() >= 72 {
                    // Token account layout: mint (32) + owner (32) + amount (8)
                    let account_mint = Pubkey::try_from(&data[0..32]).ok();
                    let account_owner = Pubkey::try_from(&data[32..64]).ok();
                    let amount = u64::from_le_bytes(data[64..72].try_into().unwrap_or([0; 8]));

                    return account_mint == Some(*nft_mint)
                        && account_owner == Some(*caller)
                        && amount >= 1;
                }
            }
            false
        }
        (None, _) => false, // No NFT on position
        (Some(_), None) => false, // NFT exists but not provided
    }
}

pub fn unstake_channel(
    ctx: Context<UnstakeChannel>,
    _channel: String,
    amount: u64,
) -> Result<()> {
    let clock = Clock::get()?;
    let current_slot = clock.slot;

    // Capture keys, values, and account infos before mutable borrows
    let mint_key = ctx.accounts.mint.key();
    let subject = ctx.accounts.channel_config.subject;
    let pool_key = ctx.accounts.stake_pool.key();
    let pool_bump = ctx.accounts.stake_pool.bump;
    let decimals = ctx.accounts.mint.decimals;
    let stake_pool_info = ctx.accounts.stake_pool.to_account_info();

    let pool = &mut ctx.accounts.stake_pool;
    let user_stake = &mut ctx.accounts.user_stake;

    // Check lock period
    require!(
        current_slot >= user_stake.lock_end_slot,
        OracleError::LockNotExpired
    );

    // Validate amount
    let unstake_amount = if amount == 0 {
        user_stake.staked_amount // Unstake all
    } else {
        amount
    };

    require!(
        unstake_amount <= user_stake.staked_amount,
        OracleError::InsufficientStake
    );

    // Capture old weighted stake
    let old_weighted = user_stake.weighted_stake;

    // Transfer tokens from vault to user
    let seeds: &[&[u8]] = &[
        CHANNEL_STAKE_POOL_SEED,
        mint_key.as_ref(),
        subject.as_ref(),
        &[pool_bump],
    ];
    let signer_seeds = &[seeds];

    let transfer_ix = spl_token_2022::instruction::transfer_checked(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.stake_vault.key(),
        &mint_key,
        &ctx.accounts.user_token_account.key(),
        &pool_key,
        &[],
        unstake_amount,
        decimals,
    )?;

    anchor_lang::solana_program::program::invoke_signed(
        &transfer_ix,
        &[
            ctx.accounts.stake_vault.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.user_token_account.to_account_info(),
            stake_pool_info,
            ctx.accounts.token_program.to_account_info(),
        ],
        signer_seeds,
    )?;

    // Update user stake
    user_stake.staked_amount = user_stake
        .staked_amount
        .checked_sub(unstake_amount)
        .ok_or(OracleError::MathOverflow)?;
    user_stake.last_action = clock.unix_timestamp;

    // Recalculate weighted stake
    if user_stake.staked_amount > 0 {
        let boost_bps = calculate_boost_bps(user_stake.lock_end_slot, current_slot);
        user_stake.weighted_stake = user_stake
            .staked_amount
            .checked_mul(boost_bps)
            .ok_or(OracleError::MathOverflow)?
            .checked_div(BOOST_PRECISION)
            .ok_or(OracleError::MathOverflow)?;
    } else {
        user_stake.weighted_stake = 0;
    }

    // Update pool totals
    pool.total_staked = pool
        .total_staked
        .checked_sub(unstake_amount)
        .ok_or(OracleError::MathOverflow)?;

    pool.total_weighted_stake = pool
        .total_weighted_stake
        .checked_sub(old_weighted)
        .ok_or(OracleError::MathOverflow)?
        .checked_add(user_stake.weighted_stake)
        .ok_or(OracleError::MathOverflow)?;

    if user_stake.staked_amount == 0 {
        pool.staker_count = pool.staker_count.saturating_sub(1);
    }

    pool.last_update = clock.unix_timestamp;

    msg!(
        "Unstaked {} tokens from channel, remaining={}",
        unstake_amount,
        user_stake.staked_amount
    );

    Ok(())
}

// =============================================================================
// EXTEND LOCK
// =============================================================================

#[derive(Accounts)]
#[instruction(channel: String)]
pub struct ExtendLock<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// Protocol state
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel config
    pub channel_config: Account<'info, ChannelConfigV2>,

    /// Token mint
    #[account(
        constraint = mint.key() == protocol_state.mint @ OracleError::InvalidMint,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    /// Stake pool
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_POOL_SEED, mint.key().as_ref(), channel_config.subject.as_ref()],
        bump = stake_pool.bump,
    )]
    pub stake_pool: Account<'info, ChannelStakePool>,

    /// User's stake position
    #[account(
        mut,
        seeds = [USER_CHANNEL_STAKE_SEED, user.key().as_ref(), mint.key().as_ref(), channel_config.subject.as_ref()],
        bump = user_stake.bump,
        constraint = user_stake.user == user.key() @ OracleError::Unauthorized,
        constraint = user_stake.staked_amount > 0 @ OracleError::InsufficientStake,
    )]
    pub user_stake: Account<'info, UserChannelStake>,
}

pub fn extend_lock(
    ctx: Context<ExtendLock>,
    _channel: String,
    additional_slots: u64,
) -> Result<()> {
    let clock = Clock::get()?;
    let current_slot = clock.slot;
    let pool = &mut ctx.accounts.stake_pool;
    let user_stake = &mut ctx.accounts.user_stake;

    // Calculate new lock end
    let new_lock_end = user_stake
        .lock_end_slot
        .max(current_slot)
        .checked_add(additional_slots)
        .ok_or(OracleError::MathOverflow)?;

    require!(
        new_lock_end <= current_slot.saturating_add(pool.max_lock_slots),
        OracleError::LockPeriodTooLong
    );

    // Capture old weighted stake
    let old_weighted = user_stake.weighted_stake;

    // Update lock
    user_stake.lock_end_slot = new_lock_end;
    user_stake.last_action = clock.unix_timestamp;

    // Recalculate weighted stake with new boost
    let boost_bps = calculate_boost_bps(new_lock_end, current_slot);
    user_stake.weighted_stake = user_stake
        .staked_amount
        .checked_mul(boost_bps)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(BOOST_PRECISION)
        .ok_or(OracleError::MathOverflow)?;

    // Update pool weighted total
    pool.total_weighted_stake = pool
        .total_weighted_stake
        .checked_sub(old_weighted)
        .ok_or(OracleError::MathOverflow)?
        .checked_add(user_stake.weighted_stake)
        .ok_or(OracleError::MathOverflow)?;

    pool.last_update = clock.unix_timestamp;

    msg!(
        "Extended lock to slot {}, new boost={}bps, weighted={}",
        new_lock_end,
        boost_bps,
        user_stake.weighted_stake
    );

    Ok(())
}
