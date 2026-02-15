//! Channel staking instructions using Token-2022 soulbound NFT receipts.
//!
//! Uses the NonTransferable extension to create soulbound stake receipts.
//! The receipt proves stake ownership and must be burned to unstake.

use crate::constants::{
    calculate_boost_bps, BOOST_PRECISION, CHANNEL_STAKE_POOL_SEED, CHANNEL_USER_STAKE_SEED,
    MAX_LOCK_SLOTS, MIN_STAKE_AMOUNT, PROTOCOL_SEED, REWARD_PRECISION, STAKE_NFT_MINT_SEED,
    STAKE_VAULT_SEED,
};
use crate::errors::OracleError;
use crate::events::{ChannelStaked, ChannelUnstaked, ChannelEmergencyUnstaked, PoolClosed, PoolRecovered};
use crate::state::{ChannelConfigV2, ChannelStakePool, ProtocolState, UserChannelStake};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::spl_token_2022,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

/// Token-2022 program ID for CPI validation (TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb)
const TOKEN_2022_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    0x06, 0xdd, 0xf6, 0xe1, 0xee, 0x75, 0x8f, 0xde, 0x18, 0x42, 0x5d, 0xbc, 0xe4, 0x6c, 0xcd, 0xda,
    0xb6, 0x1a, 0xfc, 0x4d, 0x83, 0xb9, 0x0d, 0x27, 0xfe, 0xbd, 0xf9, 0x28, 0xd8, 0xa1, 0x8b, 0xfc,
]);

// =============================================================================
// REWARD HELPERS (MasterChef-style accumulator)
// =============================================================================

/// Update pool's accumulated rewards per share.
/// Call this before any stake/unstake/claim action.
pub fn update_pool_rewards(pool: &mut ChannelStakePool, current_slot: u64) -> Result<()> {
    // Skip if no time elapsed or no stakers
    if current_slot <= pool.last_reward_slot || pool.total_weighted == 0 {
        pool.last_reward_slot = current_slot;
        return Ok(());
    }

    // Calculate rewards accrued since last update
    let slots_elapsed = current_slot
        .checked_sub(pool.last_reward_slot)
        .ok_or(OracleError::MathOverflow)?;

    let rewards_accrued = (pool.reward_per_slot as u128)
        .checked_mul(slots_elapsed as u128)
        .ok_or(OracleError::MathOverflow)?;

    // Update accumulator: acc += (rewards * PRECISION) / total_weighted
    let reward_per_share_increase = rewards_accrued
        .checked_mul(REWARD_PRECISION)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(pool.total_weighted as u128)
        .ok_or(OracleError::MathOverflow)?;

    pool.acc_reward_per_share = pool
        .acc_reward_per_share
        .checked_add(reward_per_share_increase)
        .ok_or(OracleError::MathOverflow)?;

    pool.last_reward_slot = current_slot;

    Ok(())
}

/// Calculate user's pending rewards (claimable amount).
pub fn calculate_pending_rewards(
    user_stake: &UserChannelStake,
    pool: &ChannelStakePool,
) -> Result<u64> {
    // User's weighted stake
    let weighted_stake = (user_stake.amount as u128)
        .checked_mul(user_stake.multiplier_bps as u128)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(BOOST_PRECISION as u128)
        .ok_or(OracleError::MathOverflow)?;

    // Accumulated rewards = (weighted_stake * acc_reward_per_share) / PRECISION
    let accumulated = weighted_stake
        .checked_mul(pool.acc_reward_per_share)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(REWARD_PRECISION)
        .ok_or(OracleError::MathOverflow)?;

    // Pending = accumulated - reward_debt + already_pending
    let pending = accumulated
        .checked_sub(user_stake.reward_debt)
        .ok_or(OracleError::MathOverflow)?
        .checked_add(user_stake.pending_rewards as u128)
        .ok_or(OracleError::MathOverflow)?;

    Ok(pending as u64)
}

/// Calculate new reward debt for user after stake change.
pub fn calculate_reward_debt(
    amount: u64,
    multiplier_bps: u64,
    acc_reward_per_share: u128,
) -> Result<u128> {
    let weighted_stake = (amount as u128)
        .checked_mul(multiplier_bps as u128)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(BOOST_PRECISION as u128)
        .ok_or(OracleError::MathOverflow)?;

    Ok(weighted_stake
        .checked_mul(acc_reward_per_share)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(REWARD_PRECISION)
        .ok_or(OracleError::MathOverflow)?)
}

// =============================================================================
// INITIALIZE STAKE POOL
// =============================================================================

#[derive(Accounts)]
pub struct InitializeStakePool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Protocol state (for authority check)
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = payer.key() == protocol_state.admin || payer.key() == protocol_state.publisher @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel config for this pool
    #[account(
        constraint = channel_config.mint == protocol_state.mint @ OracleError::InvalidMint,
    )]
    pub channel_config: Account<'info, ChannelConfigV2>,

    /// Token mint (CCM)
    #[account(
        constraint = mint.key() == protocol_state.mint @ OracleError::InvalidMint,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    /// Stake pool PDA
    #[account(
        init,
        payer = payer,
        space = ChannelStakePool::LEN,
        seeds = [CHANNEL_STAKE_POOL_SEED, channel_config.key().as_ref()],
        bump
    )]
    pub stake_pool: Account<'info, ChannelStakePool>,

    /// Vault to hold staked tokens
    #[account(
        init,
        payer = payer,
        token::mint = mint,
        token::authority = stake_pool,
        seeds = [STAKE_VAULT_SEED, stake_pool.key().as_ref()],
        bump
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        constraint = token_program.key() == TOKEN_2022_PROGRAM_ID @ OracleError::InvalidTokenProgram,
    )]
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_stake_pool(ctx: Context<InitializeStakePool>) -> Result<()> {
    let clock = Clock::get()?;
    let pool = &mut ctx.accounts.stake_pool;

    pool.bump = ctx.bumps.stake_pool;
    pool.channel = ctx.accounts.channel_config.key();
    pool.mint = ctx.accounts.mint.key();
    pool.vault = ctx.accounts.vault.key();
    pool.total_staked = 0;
    pool.total_weighted = 0;
    pool.staker_count = 0;

    // Initialize reward fields
    pool.acc_reward_per_share = 0;
    pool.last_reward_slot = clock.slot;
    pool.reward_per_slot = 0; // Admin sets this later
    pool.is_shutdown = false;

    msg!(
        "Initialized stake pool for channel: {}, vault: {}",
        ctx.accounts.channel_config.key(),
        ctx.accounts.vault.key()
    );

    Ok(())
}

// =============================================================================
// STAKE CHANNEL (Mint Soulbound Receipt)
// =============================================================================

#[derive(Accounts)]
pub struct StakeChannel<'info> {
    /// The user/staker (can be a PDA for vault integrations)
    pub user: Signer<'info>,

    /// Rent payer (separate to allow PDA users)
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Protocol state (for pause check)
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ OracleError::ProtocolPaused,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel config
    #[account(
        constraint = channel_config.mint == protocol_state.mint @ OracleError::InvalidMint,
    )]
    pub channel_config: Box<Account<'info, ChannelConfigV2>>,

    /// Token mint (CCM)
    #[account(
        constraint = mint.key() == protocol_state.mint @ OracleError::InvalidMint,
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// Stake pool
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_POOL_SEED, channel_config.key().as_ref()],
        bump = stake_pool.bump,
    )]
    pub stake_pool: Box<Account<'info, ChannelStakePool>>,

    /// User's stake position
    #[account(
        init,
        payer = payer,
        space = UserChannelStake::LEN,
        seeds = [CHANNEL_USER_STAKE_SEED, channel_config.key().as_ref(), user.key().as_ref()],
        bump
    )]
    pub user_stake: Box<Account<'info, UserChannelStake>>,

    /// Vault holding staked tokens
    #[account(
        mut,
        address = stake_pool.vault,
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// User's token account
    #[account(
        mut,
        constraint = user_token_account.owner == user.key() @ OracleError::Unauthorized,
        constraint = user_token_account.mint == mint.key() @ OracleError::InvalidMint,
    )]
    pub user_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Soulbound NFT mint (initialized with NonTransferable extension via CPI)
    /// CHECK: We manually initialize this account with Token-2022 extensions
    #[account(
        mut,
        seeds = [STAKE_NFT_MINT_SEED, stake_pool.key().as_ref(), user.key().as_ref()],
        bump
    )]
    pub nft_mint: UncheckedAccount<'info>,

    /// User's NFT token account (created via CPI after mint initialization)
    /// CHECK: Initialized via AssociatedToken CPI after mint is created
    #[account(mut)]
    pub nft_ata: UncheckedAccount<'info>,

    #[account(
        constraint = token_program.key() == TOKEN_2022_PROGRAM_ID @ OracleError::InvalidTokenProgram,
    )]
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn stake_channel(ctx: Context<StakeChannel>, amount: u64, lock_duration: u64) -> Result<()> {
    use spl_token_2022::extension::ExtensionType;

    // Block new stakes if pool is shutdown
    require!(!ctx.accounts.stake_pool.is_shutdown, OracleError::PoolIsShutdown);

    require!(amount >= MIN_STAKE_AMOUNT, OracleError::StakeBelowMinimum);
    require!(lock_duration <= MAX_LOCK_SLOTS, OracleError::LockPeriodTooLong);

    let clock = Clock::get()?;
    let current_slot = clock.slot;

    // Calculate boost multiplier based on lock duration
    let multiplier_bps = calculate_boost_bps(lock_duration);

    let lock_end_slot = if lock_duration > 0 {
        current_slot.checked_add(lock_duration).ok_or(OracleError::MathOverflow)?
    } else {
        0
    };

    // Capture values before mutable borrows
    let mint_key = ctx.accounts.mint.key();
    let decimals = ctx.accounts.mint.decimals;
    let channel_key = ctx.accounts.channel_config.key();
    let pool_bump = ctx.accounts.stake_pool.bump;
    let pool_key = ctx.accounts.stake_pool.key();
    let user_key = ctx.accounts.user.key();
    let payer_key = ctx.accounts.payer.key();
    let nft_mint_key = ctx.accounts.nft_mint.key();

    // Pool signer seeds
    let seeds: &[&[u8]] = &[
        CHANNEL_STAKE_POOL_SEED,
        channel_key.as_ref(),
        &[pool_bump],
    ];
    let signer_seeds = &[seeds];

    // Capture vault balance before transfer to measure actual received
    let vault_balance_before = ctx.accounts.vault.amount;

    // 1. Transfer tokens from user to vault
    let transfer_ix = spl_token_2022::instruction::transfer_checked(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.user_token_account.key(),
        &mint_key,
        &ctx.accounts.vault.key(),
        &user_key,
        &[],
        amount,
        decimals,
    )?;

    anchor_lang::solana_program::program::invoke(
        &transfer_ix,
        &[
            ctx.accounts.user_token_account.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.vault.to_account_info(),
            ctx.accounts.user.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ],
    )?;

    // Measure actual tokens received (net of Token-2022 transfer fees)
    ctx.accounts.vault.reload()?;
    let actual_received = ctx.accounts.vault.amount
        .checked_sub(vault_balance_before)
        .ok_or(OracleError::MathOverflow)?;
    require!(actual_received > 0, OracleError::StakeBelowMinimum);

    // 2. Handle NFT mint — may already exist from a previous stake cycle
    let nft_mint_info = ctx.accounts.nft_mint.to_account_info();
    let nft_mint_bump = ctx.bumps.nft_mint;
    let nft_mint_seeds: &[&[u8]] = &[
        STAKE_NFT_MINT_SEED,
        pool_key.as_ref(),
        user_key.as_ref(),
        &[nft_mint_bump],
    ];
    let nft_mint_signer = &[nft_mint_seeds];

    let nft_mint_exists = nft_mint_info.data_len() > 0;

    if nft_mint_exists {
        // Re-stake: NFT mint already exists from previous cycle.
        // Check if pool still has mint authority (post-fix mints retain it).
        use spl_token_2022::extension::StateWithExtensions;
        use anchor_lang::solana_program::program_option::COption;

        let has_mint_authority = {
            let mint_data = nft_mint_info.try_borrow_data()?;
            let mint_state = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;
            mint_state.base.mint_authority == COption::Some(pool_key)
        };

        // Create ATA idempotently (may already exist from previous cycle)
        let create_ata_ix = anchor_spl::associated_token::spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &payer_key,
            &user_key,
            &nft_mint_key,
            &ctx.accounts.token_program.key(),
        );

        anchor_lang::solana_program::program::invoke(
            &create_ata_ix,
            &[
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.nft_ata.to_account_info(),
                ctx.accounts.user.to_account_info(),
                ctx.accounts.nft_mint.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.associated_token_program.to_account_info(),
            ],
        )?;

        if has_mint_authority {
            // Post-fix mint: authority retained — remint 1 NFT
            let mint_to_ix = spl_token_2022::instruction::mint_to(
                &ctx.accounts.token_program.key(),
                &nft_mint_key,
                &ctx.accounts.nft_ata.key(),
                &pool_key,
                &[],
                1,
            )?;

            anchor_lang::solana_program::program::invoke_signed(
                &mint_to_ix,
                &[
                    ctx.accounts.nft_mint.to_account_info(),
                    ctx.accounts.nft_ata.to_account_info(),
                    ctx.accounts.stake_pool.to_account_info(),
                    ctx.accounts.token_program.to_account_info(),
                ],
                signer_seeds,
            )?;
            msg!("Re-minted soulbound NFT (authority retained)");
        } else {
            // Legacy mint: authority revoked — skip NFT minting entirely.
            // The user_stake PDA serves as the authoritative stake receipt.
            msg!("Legacy NFT mint (authority revoked) — skipping NFT receipt");
        }
    } else {
        // Fresh stake: create NFT mint from scratch
        let extension_types = &[ExtensionType::NonTransferable];
        let space = ExtensionType::try_calculate_account_len::<spl_token_2022::state::Mint>(extension_types)
            .map_err(|_| OracleError::MathOverflow)?;
        let rent_lamports = ctx.accounts.rent.minimum_balance(space);

        // Create the mint account (payer funds rent, nft_mint is the new account)
        anchor_lang::solana_program::program::invoke_signed(
            &anchor_lang::solana_program::system_instruction::create_account(
                &payer_key,
                &nft_mint_key,
                rent_lamports,
                space as u64,
                &ctx.accounts.token_program.key(),
            ),
            &[
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.nft_mint.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            nft_mint_signer,
        )?;

        // Initialize NonTransferable extension
        let init_non_transferable_ix = spl_token_2022::instruction::initialize_non_transferable_mint(
            &ctx.accounts.token_program.key(),
            &nft_mint_key,
        )?;

        anchor_lang::solana_program::program::invoke(
            &init_non_transferable_ix,
            &[
                ctx.accounts.nft_mint.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
            ],
        )?;

        // Initialize the mint — pool retains authority to support future re-stakes
        let init_mint_ix = spl_token_2022::instruction::initialize_mint2(
            &ctx.accounts.token_program.key(),
            &nft_mint_key,
            &pool_key,    // mint authority (retained, NOT revoked)
            Some(&pool_key), // freeze authority
            0, // decimals
        )?;

        anchor_lang::solana_program::program::invoke(
            &init_mint_ix,
            &[
                ctx.accounts.nft_mint.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
            ],
        )?;

        // Create ATA idempotently (payer funds, user owns the ATA)
        let create_ata_ix = anchor_spl::associated_token::spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &payer_key,
            &user_key,
            &nft_mint_key,
            &ctx.accounts.token_program.key(),
        );

        anchor_lang::solana_program::program::invoke(
            &create_ata_ix,
            &[
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.nft_ata.to_account_info(),
                ctx.accounts.user.to_account_info(),
                ctx.accounts.nft_mint.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.associated_token_program.to_account_info(),
            ],
        )?;

        // Mint soulbound NFT to user
        let mint_to_ix = spl_token_2022::instruction::mint_to(
            &ctx.accounts.token_program.key(),
            &nft_mint_key,
            &ctx.accounts.nft_ata.key(),
            &pool_key,
            &[],
            1,
        )?;

        anchor_lang::solana_program::program::invoke_signed(
            &mint_to_ix,
            &[
                ctx.accounts.nft_mint.to_account_info(),
                ctx.accounts.nft_ata.to_account_info(),
                ctx.accounts.stake_pool.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
            ],
            signer_seeds,
        )?;

        // NOTE: mint authority intentionally NOT revoked to allow re-minting on re-stakes
    }

    // 8. Update pool rewards BEFORE changing totals
    let pool = &mut ctx.accounts.stake_pool;
    update_pool_rewards(pool, current_slot)?;

    // Capture acc_reward_per_share for user's reward_debt
    let current_acc = pool.acc_reward_per_share;

    // Calculate weighted amount based on actual received (net of transfer fees)
    let weighted_amount = (actual_received as u128)
        .checked_mul(multiplier_bps as u128)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(BOOST_PRECISION as u128)
        .ok_or(OracleError::MathOverflow)? as u64;

    // 9. Update pool totals (using actual received, not requested amount)
    pool.total_staked = pool
        .total_staked
        .checked_add(actual_received)
        .ok_or(OracleError::MathOverflow)?;
    pool.total_weighted = pool
        .total_weighted
        .checked_add(weighted_amount)
        .ok_or(OracleError::MathOverflow)?;
    pool.staker_count = pool
        .staker_count
        .checked_add(1)
        .ok_or(OracleError::MathOverflow)?;

    // 10. Initialize user stake with reward fields
    let user_stake = &mut ctx.accounts.user_stake;
    user_stake.bump = ctx.bumps.user_stake;
    user_stake.user = user_key;
    user_stake.channel = channel_key;
    user_stake.amount = actual_received;
    user_stake.start_slot = current_slot;
    user_stake.lock_end_slot = lock_end_slot;
    user_stake.multiplier_bps = multiplier_bps;
    user_stake.nft_mint = nft_mint_key;

    // Set reward debt so user doesn't claim rewards from before their stake
    user_stake.reward_debt = calculate_reward_debt(actual_received, multiplier_bps, current_acc)?;
    user_stake.pending_rewards = 0;

    // 11. Emit event
    emit!(ChannelStaked {
        user: user_key,
        channel: channel_key,
        amount: actual_received,
        nft_mint: nft_mint_key,
        lock_duration,
        boost_bps: multiplier_bps,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Staked {} tokens (requested {}), multiplier={}bps, lock_end={}, nft={}",
        actual_received,
        amount,
        multiplier_bps,
        lock_end_slot,
        nft_mint_key
    );

    Ok(())
}

// =============================================================================
// UNSTAKE CHANNEL (Burn Receipt)
// =============================================================================

#[derive(Accounts)]
pub struct UnstakeChannel<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// Channel config
    pub channel_config: Box<Account<'info, ChannelConfigV2>>,

    /// Token mint (CCM)
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// Stake pool
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_POOL_SEED, channel_config.key().as_ref()],
        bump = stake_pool.bump,
        constraint = stake_pool.mint == mint.key() @ OracleError::InvalidMint,
    )]
    pub stake_pool: Box<Account<'info, ChannelStakePool>>,

    /// User's stake position
    #[account(
        mut,
        close = user,
        seeds = [CHANNEL_USER_STAKE_SEED, channel_config.key().as_ref(), user.key().as_ref()],
        bump = user_stake.bump,
        constraint = user_stake.user == user.key() @ OracleError::Unauthorized,
    )]
    pub user_stake: Box<Account<'info, UserChannelStake>>,

    /// Vault holding staked tokens
    #[account(
        mut,
        address = stake_pool.vault,
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// User's token account (receives unstaked tokens)
    #[account(
        mut,
        constraint = user_token_account.owner == user.key() @ OracleError::Unauthorized,
        constraint = user_token_account.mint == mint.key() @ OracleError::InvalidMint,
    )]
    pub user_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// NFT mint to burn
    #[account(
        mut,
        address = user_stake.nft_mint,
    )]
    pub nft_mint: Box<InterfaceAccount<'info, Mint>>,

    /// User's NFT token account (may hold 0 if legacy re-stake skipped NFT)
    #[account(
        mut,
        associated_token::mint = nft_mint,
        associated_token::authority = user,
        associated_token::token_program = token_program,
    )]
    pub nft_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = token_program.key() == TOKEN_2022_PROGRAM_ID @ OracleError::InvalidTokenProgram,
    )]
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn unstake_channel(ctx: Context<UnstakeChannel>) -> Result<()> {
    let clock = Clock::get()?;
    let current_slot = clock.slot;

    // 1. Check lock period (waived if pool is shutdown for penalty-free exit)
    if !ctx.accounts.stake_pool.is_shutdown && ctx.accounts.user_stake.lock_end_slot > 0 {
        require!(
            current_slot >= ctx.accounts.user_stake.lock_end_slot,
            OracleError::LockNotExpired
        );
    }

    // 2. Update pool rewards and calculate pending (scoped borrow)
    let (pending, pool_bump) = {
        let pool = &mut ctx.accounts.stake_pool;
        update_pool_rewards(pool, current_slot)?;
        let pending = calculate_pending_rewards(&ctx.accounts.user_stake, pool)?;
        (pending, pool.bump)
    };

    // Block unstake if user has claimable pending rewards
    // Users must call claim_channel_rewards first, or the pool must be shutdown
    if pending > 0 && !ctx.accounts.stake_pool.is_shutdown {
        // Only block if rewards are actually claimable (vault has sufficient excess)
        let vault_balance = ctx.accounts.vault.amount;
        let total_staked = ctx.accounts.stake_pool.total_staked;
        let excess = vault_balance.saturating_sub(total_staked);
        if excess >= pending {
            msg!("User has {} pending rewards - call claim_channel_rewards first", pending);
            return Err(OracleError::PendingRewardsOnUnstake.into());
        }
        // Rewards underfunded - allow unstake with forfeit to prevent deadlock
        msg!(
            "Rewards underfunded ({} available, {} pending) - allowing unstake with forfeit",
            excess,
            pending
        );
    }

    // 3. Capture values before mutable borrows
    let amount = ctx.accounts.user_stake.amount;
    let multiplier_bps = ctx.accounts.user_stake.multiplier_bps;
    let weighted_amount = (amount as u128)
        .checked_mul(multiplier_bps as u128)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(BOOST_PRECISION as u128)
        .ok_or(OracleError::MathOverflow)? as u64;

    let mint_key = ctx.accounts.mint.key();
    let decimals = ctx.accounts.mint.decimals;
    let channel_key = ctx.accounts.channel_config.key();
    let pool_key = ctx.accounts.stake_pool.key();

    // 4. Burn the receipt NFT (if present — legacy re-stakes may have skipped minting)
    if ctx.accounts.nft_ata.amount > 0 {
        let burn_ix = spl_token_2022::instruction::burn(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.nft_ata.key(),
            &ctx.accounts.nft_mint.key(),
            &ctx.accounts.user.key(),
            &[],
            1,
        )?;

        anchor_lang::solana_program::program::invoke(
            &burn_ix,
            &[
                ctx.accounts.nft_ata.to_account_info(),
                ctx.accounts.nft_mint.to_account_info(),
                ctx.accounts.user.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
            ],
        )?;
    }

    // 5. Transfer tokens from vault back to user
    let seeds: &[&[u8]] = &[
        CHANNEL_STAKE_POOL_SEED,
        channel_key.as_ref(),
        &[pool_bump],
    ];
    let signer_seeds = &[seeds];

    let transfer_ix = spl_token_2022::instruction::transfer_checked(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.vault.key(),
        &mint_key,
        &ctx.accounts.user_token_account.key(),
        &pool_key,
        &[],
        amount,
        decimals,
    )?;

    anchor_lang::solana_program::program::invoke_signed(
        &transfer_ix,
        &[
            ctx.accounts.vault.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.user_token_account.to_account_info(),
            ctx.accounts.stake_pool.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ],
        signer_seeds,
    )?;

    // 6. Update pool totals
    {
        let pool = &mut ctx.accounts.stake_pool;
        pool.total_staked = pool
            .total_staked
            .checked_sub(amount)
            .ok_or(OracleError::MathOverflow)?;
        pool.total_weighted = pool
            .total_weighted
            .checked_sub(weighted_amount)
            .ok_or(OracleError::MathOverflow)?;
        pool.staker_count = pool
            .staker_count
            .checked_sub(1)
            .ok_or(OracleError::MathOverflow)?;
    }

    // 7. Emit event
    emit!(ChannelUnstaked {
        user: ctx.accounts.user.key(),
        channel: channel_key,
        amount,
        nft_mint: ctx.accounts.nft_mint.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "Unstaked {} tokens, user={}",
        amount,
        ctx.accounts.user.key()
    );

    Ok(())
}

// =============================================================================
// CLAIM CHANNEL REWARDS
// =============================================================================

#[derive(Accounts)]
pub struct ClaimChannelRewards<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// Channel config
    pub channel_config: Box<Account<'info, ChannelConfigV2>>,

    /// Token mint (CCM)
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// Stake pool (holds rewards)
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_POOL_SEED, channel_config.key().as_ref()],
        bump = stake_pool.bump,
        constraint = stake_pool.mint == mint.key() @ OracleError::InvalidMint,
    )]
    pub stake_pool: Box<Account<'info, ChannelStakePool>>,

    /// User's stake position
    #[account(
        mut,
        seeds = [CHANNEL_USER_STAKE_SEED, channel_config.key().as_ref(), user.key().as_ref()],
        bump = user_stake.bump,
        constraint = user_stake.user == user.key() @ OracleError::Unauthorized,
    )]
    pub user_stake: Box<Account<'info, UserChannelStake>>,

    /// Vault holding staked tokens (also holds rewards for distribution)
    #[account(
        mut,
        address = stake_pool.vault,
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// User's token account (receives rewards)
    #[account(
        mut,
        constraint = user_token_account.owner == user.key() @ OracleError::Unauthorized,
        constraint = user_token_account.mint == mint.key() @ OracleError::InvalidMint,
    )]
    pub user_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = token_program.key() == TOKEN_2022_PROGRAM_ID @ OracleError::InvalidTokenProgram,
    )]
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn claim_channel_rewards(ctx: Context<ClaimChannelRewards>) -> Result<()> {
    use crate::events::ChannelRewardsClaimed;

    let clock = Clock::get()?;
    let current_slot = clock.slot;

    // 1. Update pool rewards
    let pool = &mut ctx.accounts.stake_pool;
    update_pool_rewards(pool, current_slot)?;

    // 2. Calculate pending rewards
    let pending = calculate_pending_rewards(&ctx.accounts.user_stake, pool)?;

    require!(pending > 0, OracleError::NoRewardsToClaim);

    // INVARIANT: claims must not eat into principal (total_staked)
    // excess = vault_balance - total_staked = available rewards
    let vault_balance = ctx.accounts.vault.amount;
    let total_staked = pool.total_staked;
    let excess = vault_balance.saturating_sub(total_staked);
    require!(
        excess >= pending,
        OracleError::ClaimExceedsAvailableRewards
    );

    // Capture values for CPI
    let channel_key = ctx.accounts.channel_config.key();
    let pool_bump = pool.bump;
    let mint_key = ctx.accounts.mint.key();
    let decimals = ctx.accounts.mint.decimals;
    let pool_key = ctx.accounts.stake_pool.key();

    // 3. Transfer rewards from vault to user
    let seeds: &[&[u8]] = &[
        CHANNEL_STAKE_POOL_SEED,
        channel_key.as_ref(),
        &[pool_bump],
    ];
    let signer_seeds = &[seeds];

    let transfer_ix = spl_token_2022::instruction::transfer_checked(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.vault.key(),
        &mint_key,
        &ctx.accounts.user_token_account.key(),
        &pool_key,
        &[],
        pending,
        decimals,
    )?;

    anchor_lang::solana_program::program::invoke_signed(
        &transfer_ix,
        &[
            ctx.accounts.vault.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.user_token_account.to_account_info(),
            ctx.accounts.stake_pool.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ],
        signer_seeds,
    )?;

    // 4. Update user's reward debt (reset to current accumulator value)
    let user_stake = &mut ctx.accounts.user_stake;
    user_stake.reward_debt = calculate_reward_debt(
        user_stake.amount,
        user_stake.multiplier_bps,
        ctx.accounts.stake_pool.acc_reward_per_share,
    )?;
    user_stake.pending_rewards = 0;

    // 5. Emit event
    emit!(ChannelRewardsClaimed {
        user: ctx.accounts.user.key(),
        channel: channel_key,
        amount: pending,
        timestamp: clock.unix_timestamp,
    });

    msg!("Claimed {} reward tokens", pending);

    Ok(())
}

// =============================================================================
// SET REWARD RATE (Admin only)
// =============================================================================

#[derive(Accounts)]
pub struct SetRewardRate<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel config
    pub channel_config: Box<Account<'info, ChannelConfigV2>>,

    /// Stake pool to update (realloc to new size if needed)
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_POOL_SEED, channel_config.key().as_ref()],
        bump,
        realloc = ChannelStakePool::LEN,
        realloc::payer = admin,
        realloc::zero = false,
    )]
    pub stake_pool: Box<Account<'info, ChannelStakePool>>,

    /// Vault holding staked tokens + reward reserves (for funding validation)
    #[account(
        address = stake_pool.vault,
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    pub system_program: Program<'info, System>,
}

pub fn set_reward_rate(ctx: Context<SetRewardRate>, new_rate: u64) -> Result<()> {
    use crate::constants::{BPS_DENOMINATOR, MAX_APR_BPS, MIN_RUNWAY_SLOTS, SLOTS_PER_YEAR};
    use crate::events::RewardRateUpdated;

    let clock = Clock::get()?;
    let pool = &mut ctx.accounts.stake_pool;

    // Update pool rewards before changing rate
    update_pool_rewards(pool, clock.slot)?;

    // Enforce APR cap based on current TVL
    // max_rate = (MAX_APR_BPS * total_weighted) / (BPS_DENOMINATOR * SLOTS_PER_YEAR)
    // Note: Division order matters to avoid overflow
    if pool.total_weighted > 0 {
        let max_rate = (pool.total_weighted as u128)
            .checked_mul(MAX_APR_BPS as u128)
            .ok_or(OracleError::MathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(OracleError::MathOverflow)?
            .checked_div(SLOTS_PER_YEAR as u128)
            .ok_or(OracleError::MathOverflow)? as u64;

        require!(
            new_rate <= max_rate,
            OracleError::RewardRateExceedsMaxApr
        );

        msg!(
            "Rate cap check: new_rate={}, max_rate={} ({}% APR on {} weighted)",
            new_rate,
            max_rate,
            MAX_APR_BPS / 100,
            pool.total_weighted
        );
    }

    // Enforce minimum treasury runway (prevents setting unsustainable rates)
    // Available rewards = vault_balance - total_staked (principal is sacrosanct)
    // Must have at least MIN_RUNWAY_SLOTS worth of rewards at the new rate
    if new_rate > 0 {
        let vault_balance = ctx.accounts.vault.amount;
        let total_staked = pool.total_staked;
        let available_rewards = vault_balance.saturating_sub(total_staked);

        let required_runway = (new_rate as u128)
            .checked_mul(MIN_RUNWAY_SLOTS as u128)
            .ok_or(OracleError::MathOverflow)? as u64;

        require!(
            available_rewards >= required_runway,
            OracleError::InsufficientTreasuryFunding
        );

        msg!(
            "Treasury runway check: available={}, required={} ({} slots at {} per slot)",
            available_rewards,
            required_runway,
            MIN_RUNWAY_SLOTS,
            new_rate
        );
    }

    let old_rate = pool.reward_per_slot;
    pool.reward_per_slot = new_rate;

    emit!(RewardRateUpdated {
        channel: ctx.accounts.channel_config.key(),
        old_rate,
        new_rate,
        admin: ctx.accounts.admin.key(),
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Updated reward rate for channel {}: {} -> {} per slot",
        ctx.accounts.channel_config.key(),
        old_rate,
        new_rate
    );

    Ok(())
}

// =============================================================================
// MIGRATE USER STAKE (One-time migration for existing accounts)
// =============================================================================

#[derive(Accounts)]
pub struct MigrateUserStake<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel config
    pub channel_config: Box<Account<'info, ChannelConfigV2>>,

    /// The user whose stake to migrate
    /// CHECK: Just used for PDA derivation
    pub user: UncheckedAccount<'info>,

    /// User stake to migrate (as UncheckedAccount to avoid deserialization before resize)
    /// CHECK: We validate ownership and seeds manually
    #[account(
        mut,
        seeds = [CHANNEL_USER_STAKE_SEED, channel_config.key().as_ref(), user.key().as_ref()],
        bump,
    )]
    pub user_stake: UncheckedAccount<'info>,

    /// Stake pool for getting current accumulator
    #[account(
        seeds = [CHANNEL_STAKE_POOL_SEED, channel_config.key().as_ref()],
        bump = stake_pool.bump,
    )]
    pub stake_pool: Box<Account<'info, ChannelStakePool>>,

    pub system_program: Program<'info, System>,
}

pub fn migrate_user_stake(ctx: Context<MigrateUserStake>) -> Result<()> {
    let user_stake_info = ctx.accounts.user_stake.to_account_info();
    let current_len = user_stake_info.data_len();
    let target_len = UserChannelStake::LEN;

    // Check if already migrated
    if current_len >= target_len {
        msg!("User stake already at target size ({} >= {})", current_len, target_len);
        return Ok(());
    }

    msg!("Migrating user stake from {} to {} bytes", current_len, target_len);

    // Read existing data before resize
    let data = user_stake_info.try_borrow_data()?;
    // Parse amount and multiplier_bps from old layout
    // 8 disc + 1 bump + 32 user + 32 channel = 73
    require!(data.len() >= 105, OracleError::InvalidInputLength);
    let amount = u64::from_le_bytes(
        data[73..81].try_into().map_err(|_| OracleError::InvalidInputLength)?
    );
    let multiplier_bps = u64::from_le_bytes(
        data[97..105].try_into().map_err(|_| OracleError::InvalidInputLength)?
    );
    drop(data);

    // Calculate additional rent needed
    let rent = Rent::get()?;
    let current_lamports = user_stake_info.lamports();
    let required_lamports = rent.minimum_balance(target_len);
    let lamports_diff = required_lamports.saturating_sub(current_lamports);

    // Transfer additional rent from admin if needed
    if lamports_diff > 0 {
        let transfer_ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.admin.key(),
            &user_stake_info.key(),
            lamports_diff,
        );
        anchor_lang::solana_program::program::invoke(
            &transfer_ix,
            &[
                ctx.accounts.admin.to_account_info(),
                user_stake_info.clone(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;
    }

    // Resize the account
    user_stake_info.resize(target_len)?;

    // Calculate reward_debt based on current accumulator
    // This prevents the user from claiming rewards from before migration
    let pool = &ctx.accounts.stake_pool;
    let weighted_stake = (amount as u128)
        .checked_mul(multiplier_bps as u128)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(BOOST_PRECISION as u128)
        .ok_or(OracleError::MathOverflow)?;

    let reward_debt = weighted_stake
        .checked_mul(pool.acc_reward_per_share)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(REWARD_PRECISION)
        .ok_or(OracleError::MathOverflow)?;

    // Write new fields
    let mut data = user_stake_info.try_borrow_mut_data()?;

    // reward_debt: u128 (16 bytes) at offset 137
    let debt_bytes: [u8; 16] = reward_debt.to_le_bytes();
    data[137..153].copy_from_slice(&debt_bytes);

    // pending_rewards: u64 (8 bytes) at offset 153
    let pending_bytes: [u8; 8] = 0u64.to_le_bytes();
    data[153..161].copy_from_slice(&pending_bytes);

    msg!(
        "Migrated user stake {} to {} bytes, reward_debt={}",
        user_stake_info.key(),
        target_len,
        reward_debt
    );

    Ok(())
}

// =============================================================================
// MIGRATE STAKE POOL (One-time migration for existing accounts)
// =============================================================================

#[derive(Accounts)]
pub struct MigrateStakePool<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel config
    pub channel_config: Box<Account<'info, ChannelConfigV2>>,

    /// Stake pool to migrate (as UncheckedAccount to avoid deserialization before resize)
    /// CHECK: We validate ownership and seeds manually
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_POOL_SEED, channel_config.key().as_ref()],
        bump,
    )]
    pub stake_pool: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn migrate_stake_pool(ctx: Context<MigrateStakePool>) -> Result<()> {
    let stake_pool_info = ctx.accounts.stake_pool.to_account_info();
    let current_len = stake_pool_info.data_len();
    let target_len = ChannelStakePool::LEN;

    // Check if already migrated
    if current_len >= target_len {
        msg!("Stake pool already at target size ({} >= {})", current_len, target_len);
        return Ok(());
    }

    msg!("Migrating stake pool from {} to {} bytes", current_len, target_len);

    // Calculate additional rent needed
    let rent = Rent::get()?;
    let current_lamports = stake_pool_info.lamports();
    let required_lamports = rent.minimum_balance(target_len);
    let lamports_diff = required_lamports.saturating_sub(current_lamports);

    // Transfer additional rent from admin if needed
    if lamports_diff > 0 {
        let transfer_ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.admin.key(),
            &stake_pool_info.key(),
            lamports_diff,
        );
        anchor_lang::solana_program::program::invoke(
            &transfer_ix,
            &[
                ctx.accounts.admin.to_account_info(),
                stake_pool_info.clone(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;
    }

    // Resize the account
    stake_pool_info.resize(target_len)?;

    // Only initialize NEW fields beyond the old length.
    // 129→161: reward fields (acc_reward_per_share, last_reward_slot, reward_per_slot)
    // 161→162: is_shutdown (bool)
    let clock = Clock::get()?;
    let mut data = stake_pool_info.try_borrow_mut_data()?;

    if current_len <= 129 {
        // First migration: 129→162, initialize all reward fields + is_shutdown
        data[129..145].copy_from_slice(&0u128.to_le_bytes());
        data[145..153].copy_from_slice(&clock.slot.to_le_bytes());
        data[153..161].copy_from_slice(&0u64.to_le_bytes());
        data[161] = 0; // is_shutdown = false
    } else if current_len <= 161 {
        // Second migration: 161→162, only initialize is_shutdown
        // DO NOT overwrite existing reward fields
        data[161] = 0; // is_shutdown = false
    }

    msg!(
        "Migrated stake pool {} from {} to {} bytes",
        stake_pool_info.key(),
        current_len,
        target_len
    );

    Ok(())
}

// =============================================================================
// EMERGENCY UNSTAKE (Early Exit with Penalty)
// =============================================================================

#[derive(Accounts)]
pub struct EmergencyUnstakeChannel<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// Channel config
    pub channel_config: Box<Account<'info, ChannelConfigV2>>,

    /// Token mint (CCM) — must be mut because emergency unstake burns penalty tokens,
    /// which decrements mint supply. Without mut, the burn CPI fails with PrivilegeEscalation.
    #[account(mut)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// Stake pool
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_POOL_SEED, channel_config.key().as_ref()],
        bump = stake_pool.bump,
        constraint = stake_pool.mint == mint.key() @ OracleError::InvalidMint,
    )]
    pub stake_pool: Box<Account<'info, ChannelStakePool>>,

    /// User's stake position
    #[account(
        mut,
        close = user,
        seeds = [CHANNEL_USER_STAKE_SEED, channel_config.key().as_ref(), user.key().as_ref()],
        bump = user_stake.bump,
        constraint = user_stake.user == user.key() @ OracleError::Unauthorized,
    )]
    pub user_stake: Box<Account<'info, UserChannelStake>>,

    /// Vault holding staked tokens
    #[account(
        mut,
        address = stake_pool.vault,
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// User's token account (receives returned tokens)
    #[account(
        mut,
        constraint = user_token_account.owner == user.key() @ OracleError::Unauthorized,
        constraint = user_token_account.mint == mint.key() @ OracleError::InvalidMint,
    )]
    pub user_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// NFT mint to burn
    #[account(
        mut,
        address = user_stake.nft_mint,
    )]
    pub nft_mint: Box<InterfaceAccount<'info, Mint>>,

    /// User's NFT token account (may hold 0 if legacy re-stake skipped NFT)
    #[account(
        mut,
        associated_token::mint = nft_mint,
        associated_token::authority = user,
        associated_token::token_program = token_program,
    )]
    pub nft_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = token_program.key() == TOKEN_2022_PROGRAM_ID @ OracleError::InvalidTokenProgram,
    )]
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn emergency_unstake_channel(ctx: Context<EmergencyUnstakeChannel>) -> Result<()> {
    let clock = Clock::get()?;
    let current_slot = clock.slot;

    // Prevent accidental penalties when lock already expired or no lock exists.
    require!(
        ctx.accounts.user_stake.lock_end_slot > current_slot,
        OracleError::LockExpiredUseStandardUnstake
    );

    // Capture values before mutable borrows
    let amount = ctx.accounts.user_stake.amount;
    let multiplier_bps = ctx.accounts.user_stake.multiplier_bps;
    let lock_end_slot = ctx.accounts.user_stake.lock_end_slot;

    let weighted_amount = (amount as u128)
        .checked_mul(multiplier_bps as u128)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(BOOST_PRECISION as u128)
        .ok_or(OracleError::MathOverflow)? as u64;

    let mint_key = ctx.accounts.mint.key();
    let decimals = ctx.accounts.mint.decimals;
    let channel_key = ctx.accounts.channel_config.key();
    let pool_bump = ctx.accounts.stake_pool.bump;
    let pool_key = ctx.accounts.stake_pool.key();

    // Calculate penalty (20% flat rate for early exit)
    let penalty = amount
        .checked_mul(20)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(100)
        .ok_or(OracleError::MathOverflow)?;

    let return_amount = amount
        .checked_sub(penalty)
        .ok_or(OracleError::MathOverflow)?;

    // Calculate remaining lock slots for event
    let remaining_lock_slots = lock_end_slot.saturating_sub(current_slot);

    // Pool signer seeds
    let seeds: &[&[u8]] = &[
        CHANNEL_STAKE_POOL_SEED,
        channel_key.as_ref(),
        &[pool_bump],
    ];
    let signer_seeds = &[seeds];

    // 1. Burn the receipt NFT (if present — legacy re-stakes may have skipped minting)
    if ctx.accounts.nft_ata.amount > 0 {
        let burn_ix = spl_token_2022::instruction::burn(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.nft_ata.key(),
            &ctx.accounts.nft_mint.key(),
            &ctx.accounts.user.key(),
            &[],
            1,
        )?;

        anchor_lang::solana_program::program::invoke(
            &burn_ix,
            &[
                ctx.accounts.nft_ata.to_account_info(),
                ctx.accounts.nft_mint.to_account_info(),
                ctx.accounts.user.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
            ],
        )?;
    }

    // 2. Return tokens (minus penalty) to user
    if return_amount > 0 {
        let transfer_ix = spl_token_2022::instruction::transfer_checked(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.vault.key(),
            &mint_key,
            &ctx.accounts.user_token_account.key(),
            &pool_key,
            &[],
            return_amount,
            decimals,
        )?;

        anchor_lang::solana_program::program::invoke_signed(
            &transfer_ix,
            &[
                ctx.accounts.vault.to_account_info(),
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.user_token_account.to_account_info(),
                ctx.accounts.stake_pool.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
            ],
            signer_seeds,
        )?;
    }

    // 3. Split penalty 50/50: burn half (deflationary), keep half for rewards
    let burn_amount = penalty / 2;
    let reward_amount = penalty - burn_amount; // Avoid rounding errors

    // 3a. Burn half of penalty (deflationary)
    if burn_amount > 0 {
        let burn_penalty_ix = spl_token_2022::instruction::burn(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.vault.key(),
            &mint_key,
            &pool_key,
            &[],
            burn_amount,
        )?;

        anchor_lang::solana_program::program::invoke_signed(
            &burn_penalty_ix,
            &[
                ctx.accounts.vault.to_account_info(),
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.stake_pool.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
            ],
            signer_seeds,
        )?;
    }

    // 3b. The other half (reward_amount) stays in vault for reward distribution
    // Note: total_staked is reduced by full amount, so reward_amount becomes "free" for rewards
    msg!(
        "Penalty split: {} burned, {} added to reward pool",
        burn_amount,
        reward_amount
    );

    // 4. Update pool rewards BEFORE modifying totals (prevents accumulator skew)
    let pool = &mut ctx.accounts.stake_pool;
    update_pool_rewards(pool, current_slot)?;

    pool.total_staked = pool
        .total_staked
        .checked_sub(amount)
        .ok_or(OracleError::MathOverflow)?;
    pool.total_weighted = pool
        .total_weighted
        .checked_sub(weighted_amount)
        .ok_or(OracleError::MathOverflow)?;
    pool.staker_count = pool
        .staker_count
        .checked_sub(1)
        .ok_or(OracleError::MathOverflow)?;

    // 5. Emit event
    emit!(ChannelEmergencyUnstaked {
        user: ctx.accounts.user.key(),
        channel: channel_key,
        staked_amount: amount,
        penalty_amount: penalty,
        returned_amount: return_amount,
        nft_mint: ctx.accounts.nft_mint.key(),
        remaining_lock_slots,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Emergency unstake: {} returned, {} penalty ({} burned, {} to rewards), {} slots early",
        return_amount,
        penalty,
        burn_amount,
        reward_amount,
        remaining_lock_slots
    );

    Ok(())
}

// =============================================================================
// ADMIN SHUTDOWN POOL (Emergency Penalty-Free Exit)
// =============================================================================

#[derive(Accounts)]
pub struct AdminShutdownPool<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel config
    pub channel_config: Box<Account<'info, ChannelConfigV2>>,

    /// Stake pool to shutdown (realloc to new size if needed)
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_POOL_SEED, channel_config.key().as_ref()],
        bump,
        realloc = ChannelStakePool::LEN,
        realloc::payer = admin,
        realloc::zero = false,
    )]
    pub stake_pool: Box<Account<'info, ChannelStakePool>>,

    pub system_program: Program<'info, System>,
}

pub fn admin_shutdown_pool(ctx: Context<AdminShutdownPool>, reason: String) -> Result<()> {
    use crate::events::PoolShutdown;

    let clock = Clock::get()?;
    let pool = &mut ctx.accounts.stake_pool;

    // Finalize any pending rewards before shutdown
    update_pool_rewards(pool, clock.slot)?;

    // Stop reward accrual
    let old_rate = pool.reward_per_slot;
    pool.reward_per_slot = 0;
    pool.is_shutdown = true;

    emit!(PoolShutdown {
        channel: ctx.accounts.channel_config.key(),
        admin: ctx.accounts.admin.key(),
        reason: reason.clone(),
        staker_count: pool.staker_count,
        total_staked: pool.total_staked,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Pool shutdown: channel={}, stakers={}, total_staked={}, reward_rate {} -> 0, reason={}",
        ctx.accounts.channel_config.key(),
        pool.staker_count,
        pool.total_staked,
        old_rate,
        reason
    );

    Ok(())
}

// =============================================================================
// ADMIN RECOVER POOL (Emergency: Unset Shutdown Without State Loss)
// =============================================================================

#[derive(Accounts)]
pub struct AdminRecoverPool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Protocol state (for authority check)
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = payer.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Stake pool to recover
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_POOL_SEED, channel_config.key().as_ref()],
        bump,
    )]
    pub stake_pool: Account<'info, ChannelStakePool>,

    /// Channel config (for seed derivation)
    pub channel_config: Account<'info, ChannelConfigV2>,

    pub system_program: Program<'info, System>,
}

pub fn admin_recover_pool(ctx: Context<AdminRecoverPool>) -> Result<()> {
    let pool = &mut ctx.accounts.stake_pool;

    // Simply unset shutdown flag, preserve all other state
    let was_shutdown = pool.is_shutdown;
    pool.is_shutdown = false;

    emit!(PoolRecovered {
        pool: pool.key(),
        channel: pool.channel,
        total_staked: pool.total_staked,
        staker_count: pool.staker_count,
        was_shutdown,
    });

    msg!(
        "Pool {} recovered from shutdown: total_staked={}, stakers={}",
        pool.channel,
        pool.total_staked,
        pool.staker_count
    );

    Ok(())
}

// =============================================================================
// CLOSE STAKE POOL (Recover surplus reward tokens from emptied pools)
// =============================================================================

/// Close a fully-emptied shutdown pool.
///
/// Steps:
///   1. Withdraw withheld Token-2022 transfer fees from vault (protocol_state signs)
///   2. Transfer remaining spendable tokens to destination (stake_pool signs)
///   3. Close the vault Token-2022 ATA (stake_pool signs)
///   4. Anchor closes the stake pool PDA (via `close = admin`)
///
/// Safety: only callable when pool is shut down, has 0 stakers, 0 staked,
/// and 0 weighted. This does NOT weaken trust guarantees — admin cannot
/// touch active pools or staked principal.
#[derive(Accounts)]
pub struct CloseStakePool<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel config (for PDA derivation of stake pool).
    pub channel_config: Box<Account<'info, ChannelConfigV2>>,

    /// Stake pool to close — must be shutdown with 0 stakers, 0 staked, 0 weighted.
    /// Anchor's `close = admin` returns rent after handler completes.
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_POOL_SEED, channel_config.key().as_ref()],
        bump = stake_pool.bump,
        close = admin,
        constraint = stake_pool.is_shutdown @ OracleError::PoolNotShutdown,
        constraint = stake_pool.staker_count == 0 @ OracleError::StakePoolNotEmpty,
        constraint = stake_pool.total_staked == 0 @ OracleError::StakePoolNotEmpty,
        constraint = stake_pool.total_weighted == 0 @ OracleError::StakePoolNotEmpty,
    )]
    pub stake_pool: Box<Account<'info, ChannelStakePool>>,

    /// Vault holding any remaining reward tokens.
    /// Referenced by pubkey stored on stake_pool (not derived by seeds) for robustness.
    #[account(
        mut,
        address = stake_pool.vault,
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    /// CCM mint (needed for transfer_checked and withheld fee withdrawal).
    #[account(
        mut,
        constraint = mint.key() == stake_pool.mint @ OracleError::InvalidMint,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    /// Destination for remaining reward tokens (treasury ATA, admin ATA, etc.).
    /// Must match the same mint.
    #[account(
        mut,
        constraint = destination.mint == mint.key() @ OracleError::InvalidMint,
    )]
    pub destination: InterfaceAccount<'info, TokenAccount>,

    #[account(
        constraint = token_program.key() == TOKEN_2022_PROGRAM_ID @ OracleError::InvalidTokenProgram,
    )]
    pub token_program: Interface<'info, TokenInterface>,

    pub system_program: Program<'info, System>,
}

pub fn close_stake_pool(ctx: Context<CloseStakePool>) -> Result<()> {
    use anchor_spl::token_2022_extensions::transfer_fee::{
        withdraw_withheld_tokens_from_accounts, WithdrawWithheldTokensFromAccounts,
    };

    let channel_key = ctx.accounts.channel_config.key();
    let pool_bump = ctx.accounts.stake_pool.bump;
    let mint_key = ctx.accounts.mint.key();
    let decimals = ctx.accounts.mint.decimals;

    // Pool PDA signer seeds (vault authority for transfers + close)
    let pool_seeds: &[&[u8]] = &[
        CHANNEL_STAKE_POOL_SEED,
        channel_key.as_ref(),
        &[pool_bump],
    ];
    let pool_signer = &[pool_seeds];

    // Protocol PDA signer seeds (withdraw_withheld_authority for the mint)
    let protocol_seeds: &[&[u8]] = &[
        PROTOCOL_SEED,
        mint_key.as_ref(),
        &[ctx.accounts.protocol_state.bump],
    ];
    let protocol_signer = &[protocol_seeds];

    // Step 1: Withdraw withheld Token-2022 transfer fees from the vault.
    // The protocol_state PDA is the mint's withdraw_withheld_authority.
    // This moves any withheld fees from vault -> destination so the vault
    // can be closed (close_account requires zero withheld + zero balance).
    {
        let sources = vec![ctx.accounts.vault.to_account_info()];
        withdraw_withheld_tokens_from_accounts(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                WithdrawWithheldTokensFromAccounts {
                    token_program_id: ctx.accounts.token_program.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    destination: ctx.accounts.destination.to_account_info(),
                    authority: ctx.accounts.protocol_state.to_account_info(),
                },
                protocol_signer,
            ),
            sources,
        )?;
        msg!("Withheld fees withdrawn from vault");
    }

    // Step 2: Transfer remaining spendable tokens (reward surplus) to destination.
    // Reload vault after withheld fee withdrawal to get current spendable balance.
    ctx.accounts.vault.reload()?;
    let vault_balance = ctx.accounts.vault.amount;

    if vault_balance > 0 {
        let transfer_ix = spl_token_2022::instruction::transfer_checked(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.vault.key(),
            &mint_key,
            &ctx.accounts.destination.key(),
            &ctx.accounts.stake_pool.key(),
            &[],
            vault_balance,
            decimals,
        )?;

        anchor_lang::solana_program::program::invoke_signed(
            &transfer_ix,
            &[
                ctx.accounts.vault.to_account_info(),
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.destination.to_account_info(),
                ctx.accounts.stake_pool.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
            ],
            pool_signer,
        )?;

        msg!("Transferred {} surplus tokens to destination", vault_balance);
    }

    // Step 3: Close the vault ATA (returns SOL rent to admin).
    let close_ix = spl_token_2022::instruction::close_account(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.vault.key(),
        &ctx.accounts.admin.key(),
        &ctx.accounts.stake_pool.key(),
        &[],
    )?;

    anchor_lang::solana_program::program::invoke_signed(
        &close_ix,
        &[
            ctx.accounts.vault.to_account_info(),
            ctx.accounts.admin.to_account_info(),
            ctx.accounts.stake_pool.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ],
        pool_signer,
    )?;

    // Step 4: Emit event (vault_balance = gross amount attempted, subject to 0.5% transfer fee).
    emit!(PoolClosed {
        channel: ctx.accounts.channel_config.key(),
        admin: ctx.accounts.admin.key(),
        tokens_recovered: vault_balance,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "Pool closed: channel={}, tokens_recovered={} (gross, minus 0.5% transfer fee)",
        ctx.accounts.channel_config.key(),
        vault_balance,
    );

    // Step 5: Anchor closes the stake_pool PDA via `close = admin` after handler returns.
    Ok(())
}
