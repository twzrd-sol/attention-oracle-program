//! NFT stake position instructions for the Attention Oracle Protocol.
//!
//! Allows users to mint transferable NFTs representing their stake positions.
//! The NFT holder can unstake and receives claim rights.

use crate::constants::{
    CHANNEL_STAKE_POOL_SEED, CHANNEL_STAKE_VAULT_SEED, PROTOCOL_SEED, STAKE_POSITION_NFT_SEED,
    USER_CHANNEL_STAKE_SEED,
};
use crate::errors::OracleError;
use crate::state::{ChannelConfigV2, ChannelStakePool, ProtocolState, StakePositionNFT, UserChannelStake};
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_2022::spl_token_2022;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use sha3::{Digest, Keccak256};

/// Token-2022 program ID for CPI validation
const TOKEN_2022_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    0x06, 0xdd, 0xf6, 0xe1, 0xd7, 0x65, 0xa1, 0x93, 0xd9, 0xcb, 0xe1, 0x46, 0xce, 0xeb, 0x79, 0xac,
    0x1c, 0xb4, 0x85, 0xed, 0x5f, 0x5b, 0x37, 0x91, 0x3a, 0x8c, 0xf5, 0x85, 0x7e, 0xff, 0x00, 0xa9,
]);

// =============================================================================
// MINT STAKE POSITION NFT
// =============================================================================

#[derive(Accounts)]
#[instruction(channel: String)]
pub struct MintStakePositionNft<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// Protocol state
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel config (must match protocol mint) - boxed to reduce stack usage
    #[account(
        constraint = channel_config.mint == protocol_state.mint @ OracleError::InvalidMint,
    )]
    pub channel_config: Box<Account<'info, ChannelConfigV2>>,

    /// Token mint (CCM)
    #[account(
        constraint = mint.key() == protocol_state.mint @ OracleError::InvalidMint,
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// Stake pool - boxed to reduce stack usage
    #[account(
        seeds = [CHANNEL_STAKE_POOL_SEED, mint.key().as_ref(), channel_config.subject.as_ref()],
        bump = stake_pool.bump,
    )]
    pub stake_pool: Box<Account<'info, ChannelStakePool>>,

    /// User's stake position - must exist and not already have NFT - boxed to reduce stack usage
    #[account(
        mut,
        seeds = [USER_CHANNEL_STAKE_SEED, user.key().as_ref(), mint.key().as_ref(), channel_config.subject.as_ref()],
        bump = user_stake.bump,
        constraint = user_stake.user == user.key() @ OracleError::Unauthorized,
        constraint = user_stake.staked_amount > 0 @ OracleError::InsufficientStake,
        constraint = user_stake.nft_mint.is_none() @ OracleError::NftAlreadyMinted,
    )]
    pub user_stake: Box<Account<'info, UserChannelStake>>,

    /// New NFT mint - created by this instruction
    #[account(
        init,
        payer = user,
        mint::decimals = 0,
        mint::authority = stake_pool,
        mint::freeze_authority = stake_pool,
        seeds = [STAKE_POSITION_NFT_SEED, user_stake.key().as_ref()],
        bump,
    )]
    pub nft_mint: Box<InterfaceAccount<'info, Mint>>,

    /// NFT metadata account - boxed to reduce stack usage
    #[account(
        init,
        payer = user,
        space = StakePositionNFT::LEN,
        seeds = [b"stake_nft_meta", nft_mint.key().as_ref()],
        bump,
    )]
    pub nft_metadata: Box<Account<'info, StakePositionNFT>>,

    /// User's NFT token account
    #[account(
        init,
        payer = user,
        associated_token::mint = nft_mint,
        associated_token::authority = user,
    )]
    pub user_nft_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Token-2022 program (must be the official Token-2022 program)
    #[account(
        constraint = token_program.key() == TOKEN_2022_PROGRAM_ID @ OracleError::InvalidTokenProgram,
    )]
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn mint_stake_position_nft(
    ctx: Context<MintStakePositionNft>,
    channel: String,
) -> Result<()> {
    let clock = Clock::get()?;
    let user_stake = &mut ctx.accounts.user_stake;

    // Capture values before mutable operations
    let mint_key = ctx.accounts.mint.key();
    let subject = ctx.accounts.channel_config.subject;
    let pool_bump = ctx.accounts.stake_pool.bump;
    let nft_mint_key = ctx.accounts.nft_mint.key();

    // Hash channel name for metadata
    let channel_hash = {
        let mut hasher = Keccak256::new();
        hasher.update(channel.as_bytes());
        let result = hasher.finalize();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&result);
        arr
    };

    // Mint 1 NFT to user
    let seeds: &[&[u8]] = &[
        CHANNEL_STAKE_POOL_SEED,
        mint_key.as_ref(),
        subject.as_ref(),
        &[pool_bump],
    ];
    let signer_seeds = &[seeds];

    let mint_ix = spl_token_2022::instruction::mint_to(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.nft_mint.key(),
        &ctx.accounts.user_nft_ata.key(),
        &ctx.accounts.stake_pool.key(),
        &[],
        1,
    )?;

    anchor_lang::solana_program::program::invoke_signed(
        &mint_ix,
        &[
            ctx.accounts.nft_mint.to_account_info(),
            ctx.accounts.user_nft_ata.to_account_info(),
            ctx.accounts.stake_pool.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ],
        signer_seeds,
    )?;

    // Revoke mint authority to make fixed supply
    let revoke_ix = spl_token_2022::instruction::set_authority(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.nft_mint.key(),
        None, // Remove authority
        spl_token_2022::instruction::AuthorityType::MintTokens,
        &ctx.accounts.stake_pool.key(),
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

    // Update user stake with NFT reference
    user_stake.nft_mint = Some(nft_mint_key);
    user_stake.last_action = clock.unix_timestamp;

    // Initialize NFT metadata
    let nft_meta = &mut ctx.accounts.nft_metadata;
    nft_meta.version = 1;
    nft_meta.bump = ctx.bumps.nft_metadata;
    nft_meta.nft_mint = nft_mint_key;
    nft_meta.channel_pool = ctx.accounts.stake_pool.key();
    nft_meta.subject = subject;
    nft_meta.token_mint = mint_key;
    nft_meta.staked_amount = user_stake.staked_amount;
    nft_meta.lock_end_slot = user_stake.lock_end_slot;
    nft_meta.minted_slot = clock.slot;
    nft_meta.boost_at_mint = user_stake.weighted_stake
        .checked_mul(10000)
        .unwrap_or(0)
        .checked_div(user_stake.staked_amount.max(1))
        .unwrap_or(10000);
    nft_meta.channel_name_hash = channel_hash;

    msg!(
        "Minted stake position NFT: mint={}, staked={}, lock_end={}",
        nft_mint_key,
        user_stake.staked_amount,
        user_stake.lock_end_slot
    );

    Ok(())
}

// =============================================================================
// UNSTAKE WITH NFT (NFT holder can unstake)
// =============================================================================

#[derive(Accounts)]
#[instruction(channel: String)]
pub struct UnstakeWithNft<'info> {
    /// NFT holder (may be different from original staker)
    #[account(mut)]
    pub holder: Signer<'info>,

    /// Protocol state
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel config (must match protocol mint) - boxed to reduce stack usage
    #[account(
        constraint = channel_config.mint == protocol_state.mint @ OracleError::InvalidMint,
    )]
    pub channel_config: Box<Account<'info, ChannelConfigV2>>,

    /// Token mint (CCM)
    #[account(
        constraint = mint.key() == protocol_state.mint @ OracleError::InvalidMint,
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// Stake pool - boxed to reduce stack usage
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_POOL_SEED, mint.key().as_ref(), channel_config.subject.as_ref()],
        bump = stake_pool.bump,
    )]
    pub stake_pool: Box<Account<'info, ChannelStakePool>>,

    /// User's stake position (original staker's PDA) - boxed to reduce stack usage
    #[account(
        mut,
        seeds = [USER_CHANNEL_STAKE_SEED, user_stake.user.as_ref(), mint.key().as_ref(), channel_config.subject.as_ref()],
        bump = user_stake.bump,
        constraint = user_stake.nft_mint == Some(nft_mint.key()) @ OracleError::NftNotMinted,
    )]
    pub user_stake: Box<Account<'info, UserChannelStake>>,

    /// NFT mint
    #[account(
        mut,
        seeds = [STAKE_POSITION_NFT_SEED, user_stake.key().as_ref()],
        bump,
    )]
    pub nft_mint: Box<InterfaceAccount<'info, Mint>>,

    /// NFT metadata - boxed to reduce stack usage
    #[account(
        mut,
        close = holder,
        seeds = [b"stake_nft_meta", nft_mint.key().as_ref()],
        bump = nft_metadata.bump,
    )]
    pub nft_metadata: Box<Account<'info, StakePositionNFT>>,

    /// Holder's NFT token account (must have balance = 1)
    #[account(
        mut,
        associated_token::mint = nft_mint,
        associated_token::authority = holder,
        constraint = holder_nft_ata.amount == 1 @ OracleError::NftHolderMismatch,
    )]
    pub holder_nft_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Holder's token account (receives unstaked tokens)
    #[account(
        mut,
        constraint = holder_token_account.owner == holder.key() @ OracleError::Unauthorized,
        constraint = holder_token_account.mint == mint.key() @ OracleError::InvalidMint,
    )]
    pub holder_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Stake vault
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_VAULT_SEED, mint.key().as_ref(), channel_config.subject.as_ref()],
        bump,
    )]
    pub stake_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Token-2022 program (must be the official Token-2022 program)
    #[account(
        constraint = token_program.key() == TOKEN_2022_PROGRAM_ID @ OracleError::InvalidTokenProgram,
    )]
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn unstake_with_nft(
    ctx: Context<UnstakeWithNft>,
    _channel: String,
    amount: u64,
) -> Result<()> {
    let clock = Clock::get()?;
    let current_slot = clock.slot;

    // Capture values before mutable borrows
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

    // Transfer tokens from vault to holder
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
        &ctx.accounts.holder_token_account.key(),
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
            ctx.accounts.holder_token_account.to_account_info(),
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

    // If fully unstaked, burn the NFT
    if user_stake.staked_amount == 0 {
        // Burn NFT from holder's account
        let burn_ix = spl_token_2022::instruction::burn(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.holder_nft_ata.key(),
            &ctx.accounts.nft_mint.key(),
            &ctx.accounts.holder.key(),
            &[],
            1,
        )?;

        anchor_lang::solana_program::program::invoke(
            &burn_ix,
            &[
                ctx.accounts.holder_nft_ata.to_account_info(),
                ctx.accounts.nft_mint.to_account_info(),
                ctx.accounts.holder.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
            ],
        )?;

        user_stake.nft_mint = None;
        user_stake.weighted_stake = 0;
        pool.staker_count = pool.staker_count.saturating_sub(1);
    } else {
        // Partial unstake - recalculate weighted stake
        let boost_bps = crate::constants::calculate_boost_bps(user_stake.lock_end_slot, current_slot);
        user_stake.weighted_stake = user_stake
            .staked_amount
            .checked_mul(boost_bps)
            .ok_or(OracleError::MathOverflow)?
            .checked_div(crate::constants::BOOST_PRECISION)
            .ok_or(OracleError::MathOverflow)?;
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

    pool.last_update = clock.unix_timestamp;

    msg!(
        "Unstaked {} tokens via NFT, remaining={}, holder={}",
        unstake_amount,
        user_stake.staked_amount,
        ctx.accounts.holder.key()
    );

    Ok(())
}
