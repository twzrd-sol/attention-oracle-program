use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::constants::{
    CHANNEL_CONFIG_V2_SEED, CLAIM_STATE_V2_SEED, CUMULATIVE_ROOT_HISTORY,
    PROTOCOL_SEED, REWARD_PRECISION, STAKE_POOL_SEED, STAKE_VAULT_SEED, USER_STAKE_SEED,
};
use crate::errors::OracleError;
use crate::events::{CumulativeRewardsClaimed, InvisibleStaked};
use crate::merkle_proof::{compute_cumulative_leaf, verify_proof};
use crate::state::{ChannelConfigV2, ClaimStateV2, ProtocolState, RootEntry, StakePool, UserStake};

use super::channel::derive_subject_id;

const CHANNEL_CONFIG_V2_VERSION: u8 = 1;

/// Helper to get subject_id as [u8; 32] for use in Anchor seeds.
/// Avoids lifetime issues with Pubkey::as_ref() in macro expansion.
fn subject_id_bytes(channel: &str) -> [u8; 32] {
    derive_subject_id(channel).to_bytes()
}
const CLAIM_STATE_V2_VERSION: u8 = 1;
const MAX_PROOF_LEN: usize = 32;

// =============================================================================
// INITIALIZE CHANNEL CONFIG (V2)
// =============================================================================

/// Initialize a new cumulative channel config account.
/// Seeds: ["channel_cfg_v2", mint, subject_id]
#[derive(Accounts)]
#[instruction(channel: String)]
pub struct InitializeChannelCumulative<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        init_if_needed,
        payer = payer,
        space = ChannelConfigV2::LEN,
        seeds = [CHANNEL_CONFIG_V2_SEED, protocol_state.mint.as_ref(), &subject_id_bytes(&channel)],
        bump,
    )]
    pub channel_config: Account<'info, ChannelConfigV2>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_channel_cumulative(
    ctx: Context<InitializeChannelCumulative>,
    channel: String,
    cutover_epoch: u64,
    creator_wallet: Pubkey,
    creator_fee_bps: u16,
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;

    // Authorization: admin or allowlisted publisher
    let signer = ctx.accounts.payer.key();
    let is_admin = signer == protocol_state.admin;
    let is_publisher =
        protocol_state.publisher != Pubkey::default() && signer == protocol_state.publisher;
    require!(is_admin || is_publisher, OracleError::Unauthorized);

    // Validate creator fee (max 50%)
    require!(creator_fee_bps <= 5000, OracleError::CreatorFeeTooHigh);

    let subject_id = derive_subject_id(&channel);

    let cfg = &mut ctx.accounts.channel_config;
    if cfg.version == 0 {
        cfg.version = CHANNEL_CONFIG_V2_VERSION;
        cfg.bump = ctx.bumps.channel_config;
        cfg.mint = protocol_state.mint;
        cfg.subject = subject_id;
        cfg.authority = signer;
        cfg.latest_root_seq = 0;
        cfg.cutover_epoch = cutover_epoch;
        cfg.creator_wallet = creator_wallet;
        cfg.creator_fee_bps = creator_fee_bps;
        cfg._padding = [0u8; 6];
        cfg.roots = [RootEntry::default(); CUMULATIVE_ROOT_HISTORY];
    } else {
        require!(cfg.version == CHANNEL_CONFIG_V2_VERSION, OracleError::InvalidChannelState);
        require!(cfg.mint == protocol_state.mint, OracleError::InvalidMint);
        require!(cfg.subject == subject_id, OracleError::InvalidChannelState);
    }

    Ok(())
}

// =============================================================================
// PUBLISH CUMULATIVE ROOT (V2)
// =============================================================================

#[derive(Accounts)]
#[instruction(channel: String)]
pub struct PublishCumulativeRoot<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        mut,
        seeds = [CHANNEL_CONFIG_V2_SEED, protocol_state.mint.as_ref(), &subject_id_bytes(&channel)],
        bump = channel_config.bump,
    )]
    pub channel_config: Account<'info, ChannelConfigV2>,
}

pub fn publish_cumulative_root(
    ctx: Context<PublishCumulativeRoot>,
    channel: String,
    root_seq: u64,
    root: [u8; 32],
    dataset_hash: [u8; 32],
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;

    // Authorization: admin or allowlisted publisher
    let signer = ctx.accounts.payer.key();
    let is_admin = signer == protocol_state.admin;
    let is_publisher =
        protocol_state.publisher != Pubkey::default() && signer == protocol_state.publisher;
    require!(is_admin || is_publisher, OracleError::Unauthorized);

    let subject_id = derive_subject_id(&channel);
    let cfg = &mut ctx.accounts.channel_config;

    require!(cfg.version == CHANNEL_CONFIG_V2_VERSION, OracleError::InvalidChannelState);
    require!(cfg.mint == protocol_state.mint, OracleError::InvalidMint);
    require!(cfg.subject == subject_id, OracleError::InvalidChannelState);

    // Enforce strictly increasing sequence
    require!(root_seq == cfg.latest_root_seq + 1, OracleError::InvalidRootSeq);

    let idx = (root_seq as usize) % CUMULATIVE_ROOT_HISTORY;
    cfg.roots[idx] = RootEntry {
        seq: root_seq,
        root,
        dataset_hash,
        published_slot: Clock::get()?.slot,
    };
    cfg.latest_root_seq = root_seq;

    Ok(())
}

// =============================================================================
// CLAIM CUMULATIVE (V2)
// =============================================================================

#[derive(Accounts)]
#[instruction(channel: String)]
pub struct ClaimCumulative<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        seeds = [CHANNEL_CONFIG_V2_SEED, protocol_state.mint.as_ref(), &subject_id_bytes(&channel)],
        bump = channel_config.bump,
    )]
    pub channel_config: Box<Account<'info, ChannelConfigV2>>,

    #[account(
        init_if_needed,
        payer = claimer,
        space = ClaimStateV2::LEN,
        seeds = [CLAIM_STATE_V2_SEED, channel_config.key().as_ref(), claimer.key().as_ref()],
        bump,
    )]
    pub claim_state: Account<'info, ClaimStateV2>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = protocol_state,
        associated_token::token_program = token_program
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = claimer,
        associated_token::mint = mint,
        associated_token::authority = claimer,
        associated_token::token_program = token_program
    )]
    pub claimer_ata: InterfaceAccount<'info, TokenAccount>,

    /// Creator's token account for receiving fee split.
    /// Optional: If creator_fee_bps > 0, this must be provided.
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = channel_config.creator_wallet,
        associated_token::token_program = token_program
    )]
    pub creator_ata: Option<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_cumulative<'info>(
    ctx: Context<'_, '_, '_, 'info, ClaimCumulative<'info>>,
    channel: String,
    root_seq: u64,
    cumulative_total: u64,
    proof: Vec<[u8; 32]>,
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;
    let cfg = &ctx.accounts.channel_config;

    require_keys_eq!(
        ctx.accounts.mint.key(),
        protocol_state.mint,
        OracleError::InvalidMint
    );
    require!(proof.len() <= MAX_PROOF_LEN, OracleError::InvalidProofLength);

    let subject_id = derive_subject_id(&channel);

    require!(cfg.version == CHANNEL_CONFIG_V2_VERSION, OracleError::InvalidChannelState);
    require!(cfg.mint == protocol_state.mint, OracleError::InvalidMint);
    require!(cfg.subject == subject_id, OracleError::InvalidChannelState);

    let idx = (root_seq as usize) % CUMULATIVE_ROOT_HISTORY;
    let entry = cfg.roots[idx];
    require!(entry.seq == root_seq, OracleError::RootTooOldOrMissing);

    let leaf = compute_cumulative_leaf(
        &cfg.key(),
        &protocol_state.mint,
        root_seq,
        &ctx.accounts.claimer.key(),
        cumulative_total,
    );
    require!(verify_proof(&proof, leaf, entry.root), OracleError::InvalidProof);

    let claim_state = &mut ctx.accounts.claim_state;
    if claim_state.version == 0 {
        claim_state.version = CLAIM_STATE_V2_VERSION;
        claim_state.bump = ctx.bumps.claim_state;
        claim_state.channel = cfg.key();
        claim_state.wallet = ctx.accounts.claimer.key();
        claim_state.claimed_total = 0;
        claim_state.last_claim_seq = 0;
    } else {
        require!(claim_state.channel == cfg.key(), OracleError::InvalidClaimState);
        require!(claim_state.wallet == ctx.accounts.claimer.key(), OracleError::InvalidClaimState);
    }

    if cumulative_total <= claim_state.claimed_total {
        return Ok(());
    }

    let delta = cumulative_total
        .checked_sub(claim_state.claimed_total)
        .ok_or(OracleError::MathOverflow)?;

    // Calculate creator/user split (use u128 to prevent overflow on large claims)
    let creator_fee_bps = cfg.creator_fee_bps;
    let (user_amount, creator_amount) = if creator_fee_bps > 0 {
        let creator_cut = (delta as u128)
            .checked_mul(creator_fee_bps as u128)
            .ok_or(OracleError::MathOverflow)?
            .checked_div(10000)
            .ok_or(OracleError::MathOverflow)? as u64; // Safe cast: result < delta
        let user_cut = delta.checked_sub(creator_cut).ok_or(OracleError::MathOverflow)?;
        (user_cut, creator_cut)
    } else {
        (delta, 0u64)
    };

    let seeds: &[&[u8]] = &[
        PROTOCOL_SEED,
        protocol_state.mint.as_ref(),
        &[protocol_state.bump],
    ];
    let signer = &[seeds];

    let token_program = ctx.accounts.token_program.to_account_info();
    let from = ctx.accounts.treasury_ata.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let authority = ctx.accounts.protocol_state.to_account_info();

    // Transfer to USER
    if user_amount > 0 {
        let to = ctx.accounts.claimer_ata.to_account_info();
        crate::transfer_checked_with_remaining(
            &token_program,
            &from,
            &mint,
            &to,
            &authority,
            user_amount,
            ctx.accounts.mint.decimals,
            signer,
            ctx.remaining_accounts,
        )?;
    }

    // Transfer to CREATOR
    if creator_amount > 0 {
        if let Some(creator_ata) = &ctx.accounts.creator_ata {
            let to = creator_ata.to_account_info();
            crate::transfer_checked_with_remaining(
                &token_program,
                &from,
                &mint,
                &to,
                &authority,
                creator_amount,
                ctx.accounts.mint.decimals,
                signer,
                ctx.remaining_accounts,
            )?;
        } else {
            return Err(OracleError::MissingCreatorAta.into());
        }
    }

    claim_state.claimed_total = cumulative_total;
    claim_state.last_claim_seq = root_seq;

    // Emit event with split details
    emit!(CumulativeRewardsClaimed {
        channel: cfg.key(),
        claimer: ctx.accounts.claimer.key(),
        user_amount,
        creator_amount,
        cumulative_total,
        root_seq,
    });

    Ok(())
}

// =============================================================================
// SPONSORED CLAIM (V2) - Liquid claim to user wallet
// =============================================================================

#[derive(Accounts)]
#[instruction(channel: String)]
pub struct ClaimCumulativeSponsored<'info> {
    /// Transaction fee payer (relay bot)
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Reward recipient - NOT a signer.
    /// CHECK: Authorization via merkle proof.
    pub claimer: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        seeds = [CHANNEL_CONFIG_V2_SEED, protocol_state.mint.as_ref(), &subject_id_bytes(&channel)],
        bump = channel_config.bump,
    )]
    pub channel_config: Box<Account<'info, ChannelConfigV2>>,

    #[account(
        init_if_needed,
        payer = payer,
        space = ClaimStateV2::LEN,
        seeds = [CLAIM_STATE_V2_SEED, channel_config.key().as_ref(), claimer.key().as_ref()],
        bump,
    )]
    pub claim_state: Account<'info, ClaimStateV2>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = protocol_state,
        associated_token::token_program = token_program
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = claimer,
        associated_token::token_program = token_program
    )]
    pub claimer_ata: InterfaceAccount<'info, TokenAccount>,

    /// Creator's token account for receiving fee split.
    /// Optional: If creator_fee_bps > 0, this must be provided.
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = channel_config.creator_wallet,
        associated_token::token_program = token_program
    )]
    pub creator_ata: Option<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_cumulative_sponsored<'info>(
    ctx: Context<'_, '_, '_, 'info, ClaimCumulativeSponsored<'info>>,
    channel: String,
    root_seq: u64,
    cumulative_total: u64,
    proof: Vec<[u8; 32]>,
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;
    let cfg = &ctx.accounts.channel_config;

    require_keys_eq!(
        ctx.accounts.mint.key(),
        protocol_state.mint,
        OracleError::InvalidMint
    );
    require!(proof.len() <= MAX_PROOF_LEN, OracleError::InvalidProofLength);

    let subject_id = derive_subject_id(&channel);

    require!(cfg.version == CHANNEL_CONFIG_V2_VERSION, OracleError::InvalidChannelState);
    require!(cfg.mint == protocol_state.mint, OracleError::InvalidMint);
    require!(cfg.subject == subject_id, OracleError::InvalidChannelState);

    let idx = (root_seq as usize) % CUMULATIVE_ROOT_HISTORY;
    let entry = cfg.roots[idx];
    require!(entry.seq == root_seq, OracleError::RootTooOldOrMissing);

    let leaf = compute_cumulative_leaf(
        &cfg.key(),
        &protocol_state.mint,
        root_seq,
        &ctx.accounts.claimer.key(),
        cumulative_total,
    );
    require!(verify_proof(&proof, leaf, entry.root), OracleError::InvalidProof);

    let claim_state = &mut ctx.accounts.claim_state;
    if claim_state.version == 0 {
        claim_state.version = CLAIM_STATE_V2_VERSION;
        claim_state.bump = ctx.bumps.claim_state;
        claim_state.channel = cfg.key();
        claim_state.wallet = ctx.accounts.claimer.key();
        claim_state.claimed_total = 0;
        claim_state.last_claim_seq = 0;
    } else {
        require!(claim_state.channel == cfg.key(), OracleError::InvalidClaimState);
        require!(claim_state.wallet == ctx.accounts.claimer.key(), OracleError::InvalidClaimState);
    }

    if cumulative_total <= claim_state.claimed_total {
        return Ok(());
    }

    let delta = cumulative_total
        .checked_sub(claim_state.claimed_total)
        .ok_or(OracleError::MathOverflow)?;

    // Calculate creator/user split (use u128 to prevent overflow on large claims)
    let creator_fee_bps = cfg.creator_fee_bps;
    let (user_amount, creator_amount) = if creator_fee_bps > 0 {
        let creator_cut = (delta as u128)
            .checked_mul(creator_fee_bps as u128)
            .ok_or(OracleError::MathOverflow)?
            .checked_div(10000)
            .ok_or(OracleError::MathOverflow)? as u64; // Safe cast: result < delta
        let user_cut = delta.checked_sub(creator_cut).ok_or(OracleError::MathOverflow)?;
        (user_cut, creator_cut)
    } else {
        (delta, 0u64)
    };

    let seeds: &[&[u8]] = &[
        PROTOCOL_SEED,
        protocol_state.mint.as_ref(),
        &[protocol_state.bump],
    ];
    let signer = &[seeds];

    let token_program = ctx.accounts.token_program.to_account_info();
    let from = ctx.accounts.treasury_ata.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let authority = ctx.accounts.protocol_state.to_account_info();

    // Transfer to USER
    if user_amount > 0 {
        let to = ctx.accounts.claimer_ata.to_account_info();
        crate::transfer_checked_with_remaining(
            &token_program,
            &from,
            &mint,
            &to,
            &authority,
            user_amount,
            ctx.accounts.mint.decimals,
            signer,
            ctx.remaining_accounts,
        )?;
    }

    // Transfer to CREATOR
    if creator_amount > 0 {
        if let Some(creator_ata) = &ctx.accounts.creator_ata {
            let to = creator_ata.to_account_info();
            crate::transfer_checked_with_remaining(
                &token_program,
                &from,
                &mint,
                &to,
                &authority,
                creator_amount,
                ctx.accounts.mint.decimals,
                signer,
                ctx.remaining_accounts,
            )?;
        } else {
            return Err(OracleError::MissingCreatorAta.into());
        }
    }

    claim_state.claimed_total = cumulative_total;
    claim_state.last_claim_seq = root_seq;

    // Emit event with split details
    emit!(CumulativeRewardsClaimed {
        channel: cfg.key(),
        claimer: ctx.accounts.claimer.key(),
        user_amount,
        creator_amount,
        cumulative_total,
        root_seq,
    });

    Ok(())
}

// =============================================================================
// CLAIM AND STAKE (INVISIBLE STAKING) - Liquidity Defense
// =============================================================================

/// Invisible staking: Claims rewards directly into the stake vault.
/// No liquid tokens hit the user's wallet - they go straight to staking.
/// This is the "Liquidity Defense" mechanism to protect pool TVL.
///
/// NOTE: Uses Box<> for larger accounts to stay under Solana's 4KB stack limit.
#[derive(Accounts)]
#[instruction(channel: String)]
pub struct ClaimAndStakeSponsored<'info> {
    /// Transaction fee payer (relay bot)
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Reward recipient - NOT a signer.
    /// CHECK: Authorization via merkle proof.
    pub claimer: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Box<Account<'info, ProtocolState>>,

    #[account(
        seeds = [CHANNEL_CONFIG_V2_SEED, protocol_state.mint.as_ref(), &subject_id_bytes(&channel)],
        bump = channel_config.bump,
    )]
    pub channel_config: Box<Account<'info, ChannelConfigV2>>,

    #[account(
        init_if_needed,
        payer = payer,
        space = ClaimStateV2::LEN,
        seeds = [CLAIM_STATE_V2_SEED, channel_config.key().as_ref(), claimer.key().as_ref()],
        bump,
    )]
    pub claim_state: Box<Account<'info, ClaimStateV2>>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = protocol_state,
        associated_token::token_program = token_program
    )]
    pub treasury_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Creator's token account for receiving fee split (liquid).
    /// Optional: If creator_fee_bps > 0, this must be provided.
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = channel_config.creator_wallet,
        associated_token::token_program = token_program
    )]
    pub creator_ata: Option<Box<InterfaceAccount<'info, TokenAccount>>>,

    // === STAKING ACCOUNTS ===

    /// Stake pool PDA (must be initialized)
    #[account(
        mut,
        seeds = [STAKE_POOL_SEED, mint.key().as_ref()],
        bump = stake_pool.bump,
        constraint = stake_pool.mint == mint.key() @ OracleError::InvalidMint,
    )]
    pub stake_pool: Box<Account<'info, StakePool>>,

    /// User stake PDA (created if not exists)
    #[account(
        init_if_needed,
        payer = payer,
        space = UserStake::LEN,
        seeds = [USER_STAKE_SEED, claimer.key().as_ref(), mint.key().as_ref()],
        bump,
    )]
    pub user_stake: Box<Account<'info, UserStake>>,

    /// Stake vault (destination for staked tokens)
    #[account(
        mut,
        token::mint = mint,
        token::authority = stake_pool,
        seeds = [STAKE_VAULT_SEED, mint.key().as_ref()],
        bump,
    )]
    pub stake_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_and_stake_sponsored<'info>(
    ctx: Context<'_, '_, '_, 'info, ClaimAndStakeSponsored<'info>>,
    channel: String,
    root_seq: u64,
    cumulative_total: u64,
    proof: Vec<[u8; 32]>,
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;
    let cfg = &ctx.accounts.channel_config;

    require_keys_eq!(
        ctx.accounts.mint.key(),
        protocol_state.mint,
        OracleError::InvalidMint
    );
    require!(proof.len() <= MAX_PROOF_LEN, OracleError::InvalidProofLength);

    let subject_id = derive_subject_id(&channel);

    require!(cfg.version == CHANNEL_CONFIG_V2_VERSION, OracleError::InvalidChannelState);
    require!(cfg.mint == protocol_state.mint, OracleError::InvalidMint);
    require!(cfg.subject == subject_id, OracleError::InvalidChannelState);

    // Validate root exists
    let idx = (root_seq as usize) % CUMULATIVE_ROOT_HISTORY;
    let entry = cfg.roots[idx];
    require!(entry.seq == root_seq, OracleError::RootTooOldOrMissing);

    // Verify merkle proof
    let leaf = compute_cumulative_leaf(
        &cfg.key(),
        &protocol_state.mint,
        root_seq,
        &ctx.accounts.claimer.key(),
        cumulative_total,
    );
    require!(verify_proof(&proof, leaf, entry.root), OracleError::InvalidProof);

    // Initialize or validate claim state
    let claim_state = &mut ctx.accounts.claim_state;
    if claim_state.version == 0 {
        claim_state.version = CLAIM_STATE_V2_VERSION;
        claim_state.bump = ctx.bumps.claim_state;
        claim_state.channel = cfg.key();
        claim_state.wallet = ctx.accounts.claimer.key();
        claim_state.claimed_total = 0;
        claim_state.last_claim_seq = 0;
    } else {
        require!(claim_state.channel == cfg.key(), OracleError::InvalidClaimState);
        require!(claim_state.wallet == ctx.accounts.claimer.key(), OracleError::InvalidClaimState);
    }

    // Nothing to claim
    if cumulative_total <= claim_state.claimed_total {
        return Ok(());
    }

    let delta = cumulative_total
        .checked_sub(claim_state.claimed_total)
        .ok_or(OracleError::MathOverflow)?;

    // Calculate creator/stake split (use u128 to prevent overflow on large claims)
    let creator_fee_bps = cfg.creator_fee_bps;
    let (stake_amount, creator_amount) = if creator_fee_bps > 0 {
        let creator_cut = (delta as u128)
            .checked_mul(creator_fee_bps as u128)
            .ok_or(OracleError::MathOverflow)?
            .checked_div(10000)
            .ok_or(OracleError::MathOverflow)? as u64; // Safe cast: result < delta
        let stake_cut = delta.checked_sub(creator_cut).ok_or(OracleError::MathOverflow)?;
        (stake_cut, creator_cut)
    } else {
        (delta, 0u64)
    };

    let seeds: &[&[u8]] = &[
        PROTOCOL_SEED,
        protocol_state.mint.as_ref(),
        &[protocol_state.bump],
    ];
    let signer = &[seeds];

    let token_program = ctx.accounts.token_program.to_account_info();
    let from = ctx.accounts.treasury_ata.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let authority = ctx.accounts.protocol_state.to_account_info();

    // Transfer to CREATOR (liquid - they can sell)
    if creator_amount > 0 {
        if let Some(creator_ata) = &ctx.accounts.creator_ata {
            let to = creator_ata.to_account_info();
            crate::transfer_checked_with_remaining(
                &token_program,
                &from,
                &mint,
                &to,
                &authority,
                creator_amount,
                ctx.accounts.mint.decimals,
                signer,
                ctx.remaining_accounts,
            )?;
        } else {
            return Err(OracleError::MissingCreatorAta.into());
        }
    }

    // Transfer to STAKE VAULT (locked - no sell pressure)
    if stake_amount > 0 {
        let to = ctx.accounts.stake_vault.to_account_info();
        crate::transfer_checked_with_remaining(
            &token_program,
            &from,
            &mint,
            &to,
            &authority,
            stake_amount,
            ctx.accounts.mint.decimals,
            signer,
            ctx.remaining_accounts,
        )?;
    }

    // Update staking state
    let ts = Clock::get()?.unix_timestamp;
    let pool = &mut ctx.accounts.stake_pool;
    let user_stake = &mut ctx.accounts.user_stake;

    // Update pool rewards before modifying stake
    update_pool_rewards_inline(pool, ts)?;

    // Initialize user_stake if new
    if user_stake.version == 0 {
        user_stake.version = 1;
        user_stake.bump = ctx.bumps.user_stake;
        user_stake.user = ctx.accounts.claimer.key();
        user_stake.mint = ctx.accounts.mint.key();
        user_stake.staked_amount = 0;
        user_stake.delegated_subject = None;
        user_stake.lock_end_slot = 0;
        user_stake.reward_debt = 0;
        user_stake.pending_rewards = 0;
        user_stake.last_action_time = ts;
        user_stake._reserved = [0u8; 32];
    }

    // Harvest any pending rewards before adding new stake
    let pending = calculate_pending_inline(user_stake, pool)?;
    if pending > 0 {
        user_stake.pending_rewards = user_stake
            .pending_rewards
            .checked_add(pending)
            .ok_or(OracleError::MathOverflow)?;
    }

    // Add claimed amount to staked balance
    user_stake.staked_amount = user_stake
        .staked_amount
        .checked_add(stake_amount)
        .ok_or(OracleError::MathOverflow)?;
    user_stake.last_action_time = ts;

    // Recalculate reward debt
    user_stake.reward_debt = calculate_reward_debt_inline(
        user_stake.staked_amount,
        pool.acc_reward_per_share,
    )?;

    // Update pool total
    pool.total_staked = pool
        .total_staked
        .checked_add(stake_amount)
        .ok_or(OracleError::MathOverflow)?;

    // Update claim state
    claim_state.claimed_total = cumulative_total;
    claim_state.last_claim_seq = root_seq;

    emit!(InvisibleStaked {
        channel: cfg.key(),
        claimer: ctx.accounts.claimer.key(),
        staked_amount: stake_amount,
        creator_amount,
        cumulative_total,
        root_seq,
        total_staked: user_stake.staked_amount,
    });

    msg!(
        "Invisible stake: {} CCM staked for {}, {} CCM to creator",
        stake_amount,
        ctx.accounts.claimer.key(),
        creator_amount
    );

    Ok(())
}

// =============================================================================
// INLINE HELPERS (MasterChef-style, duplicated to avoid circular deps)
// =============================================================================

fn update_pool_rewards_inline(pool: &mut StakePool, current_time: i64) -> Result<()> {
    if pool.total_staked == 0 {
        pool.last_reward_time = current_time;
        return Ok(());
    }

    let time_delta_i64 = current_time.saturating_sub(pool.last_reward_time);
    let time_delta: u128 = u128::try_from(time_delta_i64).unwrap_or(0);

    if time_delta == 0 {
        return Ok(());
    }

    let rewards = time_delta
        .checked_mul(pool.reward_rate as u128)
        .ok_or(OracleError::MathOverflow)?;

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

fn calculate_pending_inline(user_stake: &UserStake, pool: &StakePool) -> Result<u64> {
    if user_stake.staked_amount == 0 {
        return Ok(0);
    }

    let accumulated = (user_stake.staked_amount as u128)
        .checked_mul(pool.acc_reward_per_share)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(REWARD_PRECISION)
        .ok_or(OracleError::MathOverflow)?;

    let pending = accumulated
        .saturating_sub(user_stake.reward_debt)
        .min(u64::MAX as u128) as u64;

    Ok(pending)
}

fn calculate_reward_debt_inline(staked_amount: u64, acc_reward_per_share: u128) -> Result<u128> {
    let debt = (staked_amount as u128)
        .checked_mul(acc_reward_per_share)
        .ok_or(OracleError::MathOverflow)?
        .checked_div(REWARD_PRECISION)
        .ok_or(OracleError::MathOverflow)?;
    Ok(debt)
}

// =============================================================================
// MIGRATION: Add creator_wallet fields to existing ChannelConfigV2 accounts
// =============================================================================

/// Old layout size (before creator_wallet fields)
const OLD_CHANNEL_CONFIG_V2_LEN: usize = 8 + 1 + 1 + 32 + 32 + 32 + 8 + 8 + (80 * CUMULATIVE_ROOT_HISTORY);
/// Offset where roots array starts in OLD layout
const OLD_ROOTS_OFFSET: usize = 8 + 1 + 1 + 32 + 32 + 32 + 8 + 8; // = 122
/// Offset where roots array starts in NEW layout
const NEW_ROOTS_OFFSET: usize = OLD_ROOTS_OFFSET + 32 + 2 + 6; // = 162 (+ creator_wallet + fee_bps + padding)
/// Size of roots array
const ROOTS_SIZE: usize = 80 * CUMULATIVE_ROOT_HISTORY; // = 320

/// Migrate existing ChannelConfigV2 accounts to add creator_wallet fields.
/// This is a one-time migration for accounts created before the schema change.
#[derive(Accounts)]
#[instruction(channel: String)]
pub struct MigrateChannelConfigV2<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// The channel config to migrate - use UncheckedAccount to bypass deserialization
    /// CHECK: We verify PDA seeds manually and handle raw data
    #[account(
        mut,
        seeds = [CHANNEL_CONFIG_V2_SEED, protocol_state.mint.as_ref(), &subject_id_bytes(&channel)],
        bump,
    )]
    pub channel_config: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn migrate_channel_config_v2(
    ctx: Context<MigrateChannelConfigV2>,
    _channel: String,
    creator_wallet: Pubkey,
    creator_fee_bps: u16,
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;
    let payer = &ctx.accounts.payer;

    // Authorization: admin or allowlisted publisher
    let signer = payer.key();
    let is_admin = signer == protocol_state.admin;
    let is_publisher =
        protocol_state.publisher != Pubkey::default() && signer == protocol_state.publisher;
    require!(is_admin || is_publisher, OracleError::Unauthorized);

    // Validate creator fee (max 50%)
    require!(creator_fee_bps <= 5000, OracleError::CreatorFeeTooHigh);

    let account_info = ctx.accounts.channel_config.to_account_info();
    let current_len = account_info.data_len();

    // Check if already migrated (new size)
    if current_len == ChannelConfigV2::LEN {
        msg!("Account already migrated (size = {})", current_len);
        return Ok(());
    }

    // Verify it's the old size
    require!(
        current_len == OLD_CHANNEL_CONFIG_V2_LEN,
        OracleError::InvalidChannelState
    );

    // Read old roots data before realloc
    let old_roots_data: [u8; ROOTS_SIZE] = {
        let data = account_info.try_borrow_data()?;
        let mut roots = [0u8; ROOTS_SIZE];
        roots.copy_from_slice(&data[OLD_ROOTS_OFFSET..OLD_ROOTS_OFFSET + ROOTS_SIZE]);
        roots
    };

    // Read old fields we need to preserve
    let (version, bump, mint_bytes, subject_bytes, authority_bytes, latest_root_seq, cutover_epoch) = {
        let data = account_info.try_borrow_data()?;
        let version = data[8];
        let bump = data[9];
        let mut mint = [0u8; 32];
        mint.copy_from_slice(&data[10..42]);
        let mut subject = [0u8; 32];
        subject.copy_from_slice(&data[42..74]);
        let mut authority = [0u8; 32];
        authority.copy_from_slice(&data[74..106]);
        let latest_root_seq = u64::from_le_bytes(data[106..114].try_into().unwrap());
        let cutover_epoch = u64::from_le_bytes(data[114..122].try_into().unwrap());
        (version, bump, mint, subject, authority, latest_root_seq, cutover_epoch)
    };

    // Realloc to new size
    let new_len = ChannelConfigV2::LEN;
    let rent = Rent::get()?;
    let new_minimum_balance = rent.minimum_balance(new_len);
    let current_balance = account_info.lamports();
    let lamports_diff = new_minimum_balance.saturating_sub(current_balance);

    if lamports_diff > 0 {
        // Transfer additional rent from payer
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: payer.to_account_info(),
                    to: account_info.clone(),
                },
            ),
            lamports_diff,
        )?;
    }

    // Resize account
    #[allow(deprecated)]
    account_info.realloc(new_len, false)?;

    // Write new layout
    {
        let mut data = account_info.try_borrow_mut_data()?;

        // Discriminator stays the same (bytes 0-7)
        // Version (byte 8)
        data[8] = version;
        // Bump (byte 9)
        data[9] = bump;
        // Mint (bytes 10-41)
        data[10..42].copy_from_slice(&mint_bytes);
        // Subject (bytes 42-73)
        data[42..74].copy_from_slice(&subject_bytes);
        // Authority (bytes 74-105)
        data[74..106].copy_from_slice(&authority_bytes);
        // latest_root_seq (bytes 106-113)
        data[106..114].copy_from_slice(&latest_root_seq.to_le_bytes());
        // cutover_epoch (bytes 114-121)
        data[114..122].copy_from_slice(&cutover_epoch.to_le_bytes());

        // NEW FIELDS:
        // creator_wallet (bytes 122-153)
        data[122..154].copy_from_slice(&creator_wallet.to_bytes());
        // creator_fee_bps (bytes 154-155)
        data[154..156].copy_from_slice(&creator_fee_bps.to_le_bytes());
        // _padding (bytes 156-161)
        data[156..162].copy_from_slice(&[0u8; 6]);

        // roots (bytes 162-481)
        data[NEW_ROOTS_OFFSET..NEW_ROOTS_OFFSET + ROOTS_SIZE].copy_from_slice(&old_roots_data);
    }

    msg!(
        "Migrated ChannelConfigV2: old_len={}, new_len={}, creator_wallet={}, fee_bps={}",
        current_len,
        new_len,
        creator_wallet,
        creator_fee_bps
    );

    Ok(())
}

// =============================================================================
// UPDATE CHANNEL CREATOR FEE (V2)
// =============================================================================

/// Update creator fee on already-migrated ChannelConfigV2.
/// Admin-only operation.
#[derive(Accounts)]
#[instruction(channel: String)]
pub struct UpdateChannelCreatorFee<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        mut,
        seeds = [CHANNEL_CONFIG_V2_SEED, protocol_state.mint.as_ref(), &subject_id_bytes(&channel)],
        bump = channel_config.bump,
    )]
    pub channel_config: Account<'info, ChannelConfigV2>,
}

pub fn update_channel_creator_fee(
    ctx: Context<UpdateChannelCreatorFee>,
    _channel: String,
    new_creator_fee_bps: u16,
) -> Result<()> {
    require!(new_creator_fee_bps <= 5000, OracleError::CreatorFeeTooHigh);

    let cfg = &mut ctx.accounts.channel_config;
    let old_fee = cfg.creator_fee_bps;
    cfg.creator_fee_bps = new_creator_fee_bps;

    msg!(
        "Updated creator fee: {} -> {} bps",
        old_fee,
        new_creator_fee_bps
    );

    Ok(())
}
