use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::constants::{
    CLAIM_STATE_GLOBAL_SEED, CUMULATIVE_ROOT_HISTORY, GLOBAL_ROOT_SEED, PROTOCOL_SEED,
};
use crate::errors::OracleError;
use crate::events::{GlobalRewardsClaimed, GlobalRootPublished};
use crate::merkle_proof::{compute_global_leaf, verify_proof};
use crate::state::{ClaimStateGlobal, GlobalRootConfig, ProtocolState, RootEntry};

const GLOBAL_ROOT_VERSION: u8 = 1;
const CLAIM_STATE_GLOBAL_VERSION: u8 = 1;
const MAX_PROOF_LEN: usize = 32;

// =============================================================================
// GLOBAL ROOT CONFIG
// =============================================================================

#[derive(Accounts)]
pub struct InitializeGlobalRoot<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        init,
        payer = payer,
        space = GlobalRootConfig::LEN,
        seeds = [GLOBAL_ROOT_SEED, protocol_state.mint.as_ref()],
        bump,
    )]
    pub global_root_config: Account<'info, GlobalRootConfig>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_global_root(ctx: Context<InitializeGlobalRoot>) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;

    let signer = ctx.accounts.payer.key();
    let is_admin = signer == protocol_state.admin;
    let is_publisher =
        protocol_state.publisher != Pubkey::default() && signer == protocol_state.publisher;
    require!(is_admin || is_publisher, OracleError::Unauthorized);

    let global_cfg = &mut ctx.accounts.global_root_config;
    global_cfg.version = GLOBAL_ROOT_VERSION;
    global_cfg.bump = ctx.bumps.global_root_config;
    global_cfg.mint = protocol_state.mint;
    global_cfg.latest_root_seq = 0;
    global_cfg.roots = [RootEntry::default(); CUMULATIVE_ROOT_HISTORY];

    Ok(())
}

// =============================================================================
// PUBLISH GLOBAL ROOT
// =============================================================================

#[derive(Accounts)]
pub struct PublishGlobalRoot<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        mut,
        seeds = [GLOBAL_ROOT_SEED, protocol_state.mint.as_ref()],
        bump = global_root_config.bump,
    )]
    pub global_root_config: Account<'info, GlobalRootConfig>,
}

pub fn publish_global_root(
    ctx: Context<PublishGlobalRoot>,
    root_seq: u64,
    root: [u8; 32],
    dataset_hash: [u8; 32],
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;

    let signer = ctx.accounts.payer.key();
    let is_admin = signer == protocol_state.admin;
    let is_publisher =
        protocol_state.publisher != Pubkey::default() && signer == protocol_state.publisher;
    require!(is_admin || is_publisher, OracleError::Unauthorized);
    require!(!protocol_state.paused || is_admin, OracleError::ProtocolPaused);

    let cfg = &mut ctx.accounts.global_root_config;
    require!(cfg.version == GLOBAL_ROOT_VERSION, OracleError::InvalidChannelState);
    require!(cfg.mint == protocol_state.mint, OracleError::InvalidMint);

    require!(root_seq == cfg.latest_root_seq + 1, OracleError::InvalidRootSeq);

    let idx = (root_seq as usize) % CUMULATIVE_ROOT_HISTORY;
    let slot = Clock::get()?.slot;
    cfg.roots[idx] = RootEntry {
        seq: root_seq,
        root,
        dataset_hash,
        published_slot: slot,
    };
    cfg.latest_root_seq = root_seq;

    emit!(GlobalRootPublished {
        mint: protocol_state.mint,
        root_seq,
        root,
        dataset_hash,
        publisher: signer,
        slot,
    });

    Ok(())
}

// =============================================================================
// CLAIM GLOBAL (SELF-SIGN)
// =============================================================================

#[derive(Accounts)]
pub struct ClaimGlobal<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        seeds = [GLOBAL_ROOT_SEED, protocol_state.mint.as_ref()],
        bump = global_root_config.bump,
    )]
    pub global_root_config: Box<Account<'info, GlobalRootConfig>>,

    #[account(
        init_if_needed,
        payer = claimer,
        space = ClaimStateGlobal::LEN,
        seeds = [CLAIM_STATE_GLOBAL_SEED, protocol_state.mint.as_ref(), claimer.key().as_ref()],
        bump,
    )]
    pub claim_state: Box<Account<'info, ClaimStateGlobal>>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = protocol_state,
        associated_token::token_program = token_program
    )]
    pub treasury_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = claimer,
        associated_token::mint = mint,
        associated_token::authority = claimer,
        associated_token::token_program = token_program
    )]
    pub claimer_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = token_program.key() == anchor_spl::token_2022::ID @ OracleError::InvalidTokenProgram
    )]
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_global<'info>(
    ctx: Context<'_, '_, '_, 'info, ClaimGlobal<'info>>,
    root_seq: u64,
    cumulative_total: u64,
    proof: Vec<[u8; 32]>,
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;
    let global_cfg = &ctx.accounts.global_root_config;

    require!(!protocol_state.paused, OracleError::ProtocolPaused);
    require_keys_eq!(ctx.accounts.mint.key(), protocol_state.mint, OracleError::InvalidMint);
    require!(proof.len() <= MAX_PROOF_LEN, OracleError::InvalidProofLength);

    require!(global_cfg.version == GLOBAL_ROOT_VERSION, OracleError::InvalidChannelState);
    require!(global_cfg.mint == protocol_state.mint, OracleError::InvalidMint);

    // Look up root from circular buffer
    let idx = (root_seq as usize) % CUMULATIVE_ROOT_HISTORY;
    let entry = global_cfg.roots[idx];
    require!(entry.seq == root_seq, OracleError::RootTooOldOrMissing);

    // Verify merkle proof
    let leaf = compute_global_leaf(
        &protocol_state.mint,
        root_seq,
        &ctx.accounts.claimer.key(),
        cumulative_total,
    );
    require!(verify_proof(&proof, leaf, entry.root), OracleError::InvalidProof);

    // Initialize or validate claim state
    let claim_state = &mut ctx.accounts.claim_state;
    if claim_state.version == 0 {
        claim_state.version = CLAIM_STATE_GLOBAL_VERSION;
        claim_state.bump = ctx.bumps.claim_state;
        claim_state.mint = protocol_state.mint;
        claim_state.wallet = ctx.accounts.claimer.key();
        claim_state.claimed_total = 0;
        claim_state.last_claim_seq = 0;
    } else {
        require!(claim_state.mint == protocol_state.mint, OracleError::InvalidClaimState);
        require!(
            claim_state.wallet == ctx.accounts.claimer.key(),
            OracleError::InvalidClaimState
        );
    }

    // Idempotent: no-op if already claimed up to this total
    if cumulative_total <= claim_state.claimed_total {
        return Ok(());
    }

    let delta = cumulative_total
        .checked_sub(claim_state.claimed_total)
        .ok_or(OracleError::MathOverflow)?;

    // Transfer delta to claimer (no on-chain creator fee — handled off-chain by publisher)
    let seeds: &[&[u8]] = &[
        PROTOCOL_SEED,
        protocol_state.mint.as_ref(),
        &[protocol_state.bump],
    ];
    let signer = &[seeds];

    crate::transfer_checked_with_remaining(
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.treasury_ata.to_account_info(),
        &ctx.accounts.mint.to_account_info(),
        &ctx.accounts.claimer_ata.to_account_info(),
        &ctx.accounts.protocol_state.to_account_info(),
        delta,
        ctx.accounts.mint.decimals,
        signer,
        ctx.remaining_accounts,
    )?;

    claim_state.claimed_total = cumulative_total;
    claim_state.last_claim_seq = root_seq;

    emit!(GlobalRewardsClaimed {
        claimer: ctx.accounts.claimer.key(),
        amount: delta,
        cumulative_total,
        root_seq,
    });

    Ok(())
}

// =============================================================================
// CLAIM GLOBAL (SPONSORED / GASLESS)
// =============================================================================

#[derive(Accounts)]
pub struct ClaimGlobalSponsored<'info> {
    /// Payer (relayer) pays rent + gas; claimer is the beneficiary.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: Authorized by merkle proof (wallet is leaf component).
    pub claimer: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        seeds = [GLOBAL_ROOT_SEED, protocol_state.mint.as_ref()],
        bump = global_root_config.bump,
    )]
    pub global_root_config: Box<Account<'info, GlobalRootConfig>>,

    #[account(
        init_if_needed,
        payer = payer,
        space = ClaimStateGlobal::LEN,
        seeds = [CLAIM_STATE_GLOBAL_SEED, protocol_state.mint.as_ref(), claimer.key().as_ref()],
        bump,
    )]
    pub claim_state: Box<Account<'info, ClaimStateGlobal>>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = protocol_state,
        associated_token::token_program = token_program
    )]
    pub treasury_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = claimer,
        associated_token::token_program = token_program
    )]
    pub claimer_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = token_program.key() == anchor_spl::token_2022::ID @ OracleError::InvalidTokenProgram
    )]
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_global_sponsored<'info>(
    ctx: Context<'_, '_, '_, 'info, ClaimGlobalSponsored<'info>>,
    root_seq: u64,
    cumulative_total: u64,
    proof: Vec<[u8; 32]>,
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;
    let global_cfg = &ctx.accounts.global_root_config;

    require!(!protocol_state.paused, OracleError::ProtocolPaused);
    require_keys_eq!(ctx.accounts.mint.key(), protocol_state.mint, OracleError::InvalidMint);
    require!(proof.len() <= MAX_PROOF_LEN, OracleError::InvalidProofLength);

    require!(global_cfg.version == GLOBAL_ROOT_VERSION, OracleError::InvalidChannelState);
    require!(global_cfg.mint == protocol_state.mint, OracleError::InvalidMint);

    let idx = (root_seq as usize) % CUMULATIVE_ROOT_HISTORY;
    let entry = global_cfg.roots[idx];
    require!(entry.seq == root_seq, OracleError::RootTooOldOrMissing);

    let leaf = compute_global_leaf(
        &protocol_state.mint,
        root_seq,
        &ctx.accounts.claimer.key(),
        cumulative_total,
    );
    require!(verify_proof(&proof, leaf, entry.root), OracleError::InvalidProof);

    let claim_state = &mut ctx.accounts.claim_state;
    if claim_state.version == 0 {
        claim_state.version = CLAIM_STATE_GLOBAL_VERSION;
        claim_state.bump = ctx.bumps.claim_state;
        claim_state.mint = protocol_state.mint;
        claim_state.wallet = ctx.accounts.claimer.key();
        claim_state.claimed_total = 0;
        claim_state.last_claim_seq = 0;
    } else {
        require!(claim_state.mint == protocol_state.mint, OracleError::InvalidClaimState);
        require!(
            claim_state.wallet == ctx.accounts.claimer.key(),
            OracleError::InvalidClaimState
        );
    }

    if cumulative_total <= claim_state.claimed_total {
        return Ok(());
    }

    let delta = cumulative_total
        .checked_sub(claim_state.claimed_total)
        .ok_or(OracleError::MathOverflow)?;

    let seeds: &[&[u8]] = &[
        PROTOCOL_SEED,
        protocol_state.mint.as_ref(),
        &[protocol_state.bump],
    ];
    let signer = &[seeds];

    crate::transfer_checked_with_remaining(
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.treasury_ata.to_account_info(),
        &ctx.accounts.mint.to_account_info(),
        &ctx.accounts.claimer_ata.to_account_info(),
        &ctx.accounts.protocol_state.to_account_info(),
        delta,
        ctx.accounts.mint.decimals,
        signer,
        ctx.remaining_accounts,
    )?;

    claim_state.claimed_total = cumulative_total;
    claim_state.last_claim_seq = root_seq;

    emit!(GlobalRewardsClaimed {
        claimer: ctx.accounts.claimer.key(),
        amount: delta,
        cumulative_total,
        root_seq,
    });

    Ok(())
}
