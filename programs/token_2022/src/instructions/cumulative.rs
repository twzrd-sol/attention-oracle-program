use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::constants::{
    CHANNEL_CONFIG_V2_SEED, CLAIM_STATE_V2_SEED, CUMULATIVE_ROOT_HISTORY, PROTOCOL_SEED,
};
use crate::errors::OracleError;
use crate::merkle_proof::{compute_cumulative_leaf, verify_proof};
use crate::state::{ChannelConfigV2, ClaimStateV2, ProtocolState, RootEntry};

// Re-use derive_subject_id from channel module
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
    if cfg.version == 0 {
        cfg.version = CHANNEL_CONFIG_V2_VERSION;
        cfg.bump = ctx.bumps.channel_config;
        cfg.mint = protocol_state.mint;
        cfg.subject = subject_id;
        cfg.authority = signer;
        cfg.latest_root_seq = 0;
        cfg.cutover_epoch = cutover_epoch;
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
    pub channel_config: Account<'info, ChannelConfigV2>,

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

    require_keys_eq!(
        ctx.accounts.mint.key(),
        protocol_state.mint,
        OracleError::InvalidMint
    );
    require!(proof.len() <= MAX_PROOF_LEN, OracleError::InvalidProofLength);

    let subject_id = derive_subject_id(&channel);
    let cfg = &ctx.accounts.channel_config;

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

    let seeds: &[&[u8]] = &[
        PROTOCOL_SEED,
        protocol_state.mint.as_ref(),
        &[protocol_state.bump],
    ];
    let signer = &[seeds];

    let token_program = ctx.accounts.token_program.to_account_info();
    let from = ctx.accounts.treasury_ata.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let to = ctx.accounts.claimer_ata.to_account_info();
    let authority = ctx.accounts.protocol_state.to_account_info();

    crate::transfer_checked_with_remaining(
        &token_program,
        &from,
        &mint,
        &to,
        &authority,
        delta,
        ctx.accounts.mint.decimals,
        signer,
        ctx.remaining_accounts,
    )?;

    claim_state.claimed_total = cumulative_total;
    claim_state.last_claim_seq = root_seq;

    Ok(())
}

// =============================================================================
// SPONSORED CLAIM (V2)
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
    pub channel_config: Account<'info, ChannelConfigV2>,

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

    require_keys_eq!(
        ctx.accounts.mint.key(),
        protocol_state.mint,
        OracleError::InvalidMint
    );
    require!(proof.len() <= MAX_PROOF_LEN, OracleError::InvalidProofLength);

    let subject_id = derive_subject_id(&channel);
    let cfg = &ctx.accounts.channel_config;

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

    let seeds: &[&[u8]] = &[
        PROTOCOL_SEED,
        protocol_state.mint.as_ref(),
        &[protocol_state.bump],
    ];
    let signer = &[seeds];

    let token_program = ctx.accounts.token_program.to_account_info();
    let from = ctx.accounts.treasury_ata.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let to = ctx.accounts.claimer_ata.to_account_info();
    let authority = ctx.accounts.protocol_state.to_account_info();

    crate::transfer_checked_with_remaining(
        &token_program,
        &from,
        &mint,
        &to,
        &authority,
        delta,
        ctx.accounts.mint.decimals,
        signer,
        ctx.remaining_accounts,
    )?;

    claim_state.claimed_total = cumulative_total;
    claim_state.last_claim_seq = root_seq;

    Ok(())
}
