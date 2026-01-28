//! Channel staking instructions using Token-2022 soulbound NFT receipts.
//!
//! Uses the NonTransferable extension to create soulbound stake receipts.
//! The receipt proves stake ownership and must be burned to unstake.

use crate::constants::{
    calculate_boost_bps, BOOST_PRECISION, CHANNEL_STAKE_POOL_SEED, CHANNEL_USER_STAKE_SEED,
    MAX_LOCK_SLOTS, MIN_STAKE_AMOUNT, PROTOCOL_SEED, STAKE_NFT_MINT_SEED, STAKE_VAULT_SEED,
};
use crate::errors::OracleError;
use crate::events::{ChannelStaked, ChannelUnstaked, ChannelEmergencyUnstaked};
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
    let pool = &mut ctx.accounts.stake_pool;
    pool.bump = ctx.bumps.stake_pool;
    pool.channel = ctx.accounts.channel_config.key();
    pool.mint = ctx.accounts.mint.key();
    pool.vault = ctx.accounts.vault.key();
    pool.total_staked = 0;
    pool.total_weighted = 0;
    pool.staker_count = 0;

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

    require!(amount >= MIN_STAKE_AMOUNT, OracleError::StakeBelowMinimum);
    require!(lock_duration <= MAX_LOCK_SLOTS, OracleError::LockPeriodTooLong);

    let clock = Clock::get()?;
    let current_slot = clock.slot;

    // Calculate boost multiplier based on lock duration
    let multiplier_bps = calculate_boost_bps(lock_duration);
    let weighted_amount = (amount as u128)
        .checked_mul(multiplier_bps as u128)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(BOOST_PRECISION as u128)
        .ok_or(OracleError::MathOverflow)? as u64;

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

    // 2. Create NFT mint account with NonTransferable extension
    let extension_types = &[ExtensionType::NonTransferable];
    let space = ExtensionType::try_calculate_account_len::<spl_token_2022::state::Mint>(extension_types)
        .map_err(|_| OracleError::MathOverflow)?;
    let rent_lamports = ctx.accounts.rent.minimum_balance(space);

    // NFT mint PDA seeds
    let nft_mint_bump = ctx.bumps.nft_mint;
    let nft_mint_seeds: &[&[u8]] = &[
        STAKE_NFT_MINT_SEED,
        pool_key.as_ref(),
        user_key.as_ref(),
        &[nft_mint_bump],
    ];
    let nft_mint_signer = &[nft_mint_seeds];

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

    // 3. Initialize NonTransferable extension
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

    // 4. Initialize the mint
    let init_mint_ix = spl_token_2022::instruction::initialize_mint2(
        &ctx.accounts.token_program.key(),
        &nft_mint_key,
        &pool_key,    // mint authority
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

    // 5. Create associated token account for NFT (payer funds, user owns the ATA)
    let create_ata_ix = anchor_spl::associated_token::spl_associated_token_account::instruction::create_associated_token_account(
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

    // 6. Mint soulbound NFT to user
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

    // 7. Revoke mint authority (fixed supply of 1)
    let revoke_ix = spl_token_2022::instruction::set_authority(
        &ctx.accounts.token_program.key(),
        &nft_mint_key,
        None,
        spl_token_2022::instruction::AuthorityType::MintTokens,
        &pool_key,
        &[],
    )?;

    anchor_lang::solana_program::program::invoke_signed(
        &revoke_ix,
        &[
            ctx.accounts.nft_mint.to_account_info(),
            ctx.accounts.stake_pool.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ],
        signer_seeds,
    )?;

    // 8. Initialize user stake
    let user_stake = &mut ctx.accounts.user_stake;
    user_stake.bump = ctx.bumps.user_stake;
    user_stake.user = user_key;
    user_stake.channel = channel_key;
    user_stake.amount = amount;
    user_stake.start_slot = current_slot;
    user_stake.lock_end_slot = lock_end_slot;
    user_stake.multiplier_bps = multiplier_bps;
    user_stake.nft_mint = nft_mint_key;

    // 9. Update pool totals
    let pool = &mut ctx.accounts.stake_pool;
    pool.total_staked = pool
        .total_staked
        .checked_add(amount)
        .ok_or(OracleError::MathOverflow)?;
    pool.total_weighted = pool
        .total_weighted
        .checked_add(weighted_amount)
        .ok_or(OracleError::MathOverflow)?;
    pool.staker_count = pool
        .staker_count
        .checked_add(1)
        .ok_or(OracleError::MathOverflow)?;

    // 10. Emit event
    emit!(ChannelStaked {
        user: user_key,
        channel: channel_key,
        amount,
        nft_mint: nft_mint_key,
        lock_duration,
        boost_bps: multiplier_bps,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Staked {} tokens, multiplier={}bps, lock_end={}, nft={}",
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

    /// User's NFT token account
    #[account(
        mut,
        associated_token::mint = nft_mint,
        associated_token::authority = user,
        associated_token::token_program = token_program,
        constraint = nft_ata.amount == 1 @ OracleError::NftHolderMismatch,
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

    // 1. Check lock period
    if ctx.accounts.user_stake.lock_end_slot > 0 {
        require!(
            current_slot >= ctx.accounts.user_stake.lock_end_slot,
            OracleError::LockNotExpired
        );
    }

    // Capture values before mutable borrows
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
    let pool_bump = ctx.accounts.stake_pool.bump;
    let pool_key = ctx.accounts.stake_pool.key();

    // 2. Burn the receipt NFT
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

    // 3. Transfer tokens from vault back to user
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

    // 4. Update pool totals
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

    // 5. Emit event
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
// EMERGENCY UNSTAKE (Early Exit with Penalty)
// =============================================================================

#[derive(Accounts)]
pub struct EmergencyUnstakeChannel<'info> {
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

    /// User's NFT token account
    #[account(
        mut,
        associated_token::mint = nft_mint,
        associated_token::authority = user,
        associated_token::token_program = token_program,
        constraint = nft_ata.amount == 1 @ OracleError::NftHolderMismatch,
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
    let remaining_lock_slots = if lock_end_slot > current_slot {
        lock_end_slot - current_slot
    } else {
        0
    };

    // Pool signer seeds
    let seeds: &[&[u8]] = &[
        CHANNEL_STAKE_POOL_SEED,
        channel_key.as_ref(),
        &[pool_bump],
    ];
    let signer_seeds = &[seeds];

    // 1. Burn the receipt NFT
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

    // 3. Burn penalty (deflationary - reduces total CCM supply)
    if penalty > 0 {
        let burn_penalty_ix = spl_token_2022::instruction::burn(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.vault.key(),
            &mint_key,
            &pool_key,
            &[],
            penalty,
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

    // 4. Update pool totals (use checked_sub for safety)
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
        "Emergency unstake: {} returned, {} penalty burned, {} slots early",
        return_amount,
        penalty,
        remaining_lock_slots
    );

    Ok(())
}
