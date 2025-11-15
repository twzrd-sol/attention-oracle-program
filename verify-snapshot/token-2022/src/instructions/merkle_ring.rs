use crate::{
    constants::{CHANNEL_MAX_CLAIMS, CHANNEL_STATE_SEED, MAX_ID_BYTES, PROTOCOL_SEED},
    errors::ProtocolError,
    instructions::claim::{compute_leaf, verify_proof},
    state::{ChannelSlot, ChannelState, ProtocolState},
};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked},
};

const MAX_PROOF_NODES: usize = 20;

/// Initialize channel ring buffer (one-time setup per channel)
#[derive(Accounts)]
#[instruction(streamer_key: Pubkey)]
pub struct InitializeChannel<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel state PDA - will be created if doesn't exist
    /// Seeds: [CHANNEL_STATE_SEED, mint, streamer_key] - NO EPOCH
    #[account(
        init_if_needed,
        payer = payer,
        space = ChannelState::LEN,
        seeds = [CHANNEL_STATE_SEED, protocol_state.mint.as_ref(), streamer_key.as_ref()],
        bump
    )]
    pub channel_state: AccountLoader<'info, ChannelState>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_channel(ctx: Context<InitializeChannel>, streamer_key: Pubkey) -> Result<()> {
    let channel_state = &mut ctx.accounts.channel_state.load_init()?;

    channel_state.version = 1;
    channel_state.bump = ctx.bumps.channel_state;
    channel_state.mint = ctx.accounts.protocol_state.mint;
    channel_state.streamer = streamer_key;
    channel_state.latest_epoch = 0;

    // Slots are already zeroed by init

    msg!("Channel initialized for streamer: {}", streamer_key);
    Ok(())
}

/// Set merkle root using ring buffer (replaces old per-epoch accounts)
#[derive(Accounts)]
#[instruction(root: [u8; 32], epoch: u64, claim_count: u16, streamer_key: Pubkey)]
pub struct SetMerkleRootRing<'info> {
    #[account(mut)]
    pub update_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel state - must exist (call initialize_channel first)
    #[account(
        mut,
        seeds = [CHANNEL_STATE_SEED, protocol_state.mint.as_ref(), streamer_key.as_ref()],
        bump
    )]
    pub channel_state: AccountLoader<'info, ChannelState>,

    pub system_program: Program<'info, System>,
}

pub fn set_merkle_root_ring(
    ctx: Context<SetMerkleRootRing>,
    root: [u8; 32],
    epoch: u64,
    claim_count: u16,
    streamer_key: Pubkey,
) -> Result<()> {
    let protocol = &ctx.accounts.protocol_state;

    authorize_publisher(protocol, &ctx.accounts.update_authority.key())?;
    require!(!protocol.paused, ProtocolError::ProtocolPaused);
    require!(epoch > 0, ProtocolError::InvalidEpoch);
    require!(
        claim_count as usize <= CHANNEL_MAX_CLAIMS,
        ProtocolError::InvalidInputLength
    );

    let channel_state = &mut ctx.accounts.channel_state.load_mut()?;

    // Verify channel was initialized
    require!(
        channel_state.version > 0,
        ProtocolError::ChannelNotInitialized
    );
    require!(
        channel_state.streamer == streamer_key,
        ProtocolError::InvalidStreamer
    );

    // Update ring buffer slot (modulo 10)
    let slot = channel_state.slot_mut(epoch);
    require!(
        slot.epoch == 0 || epoch > slot.epoch,
        ProtocolError::EpochNotIncreasing
    );
    slot.reset(epoch, root);
    slot.claim_count = claim_count;

    // Update latest epoch
    if epoch > channel_state.latest_epoch {
        channel_state.latest_epoch = epoch;
    }

    msg!(
        "Merkle root set for epoch {} in slot {}",
        epoch,
        ChannelState::slot_index(epoch)
    );

    Ok(())
}

/// Claim tokens using ring buffer state
#[derive(Accounts)]
#[instruction(epoch: u64, index: u32, amount: u64, proof: Vec<[u8; 32]>, id: String, streamer_key: Pubkey)]
pub struct ClaimWithRing<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ ProtocolError::ProtocolPaused,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        mut,
        seeds = [CHANNEL_STATE_SEED, protocol_state.mint.as_ref(), streamer_key.as_ref()],
        bump
    )]
    pub channel_state: AccountLoader<'info, ChannelState>,

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

pub fn claim_with_ring(
    ctx: Context<ClaimWithRing>,
    epoch: u64,
    index: u32,
    amount: u64,
    proof: Vec<[u8; 32]>,
    id: String,
    streamer_key: Pubkey,
) -> Result<()> {
    let channel_state = &mut ctx.accounts.channel_state.load_mut()?;

    // Verify channel initialized
    require!(
        channel_state.version > 0,
        ProtocolError::ChannelNotInitialized
    );
    require!(
        channel_state.streamer == streamer_key,
        ProtocolError::InvalidStreamer
    );

    let slot = channel_state.slot_mut(epoch);
    require!(slot.epoch == epoch, ProtocolError::InvalidEpoch);
    require!(id.len() <= MAX_ID_BYTES, ProtocolError::InvalidInputLength);
    ChannelSlot::validate_index(index as usize)?;
    require!(index < slot.claim_count as u32, ProtocolError::InvalidIndex);
    require!(
        proof.len() <= MAX_PROOF_NODES,
        ProtocolError::InvalidProofLength
    );
    require!(
        !slot.test_bit(index as usize),
        ProtocolError::AlreadyClaimed
    );

    let leaf = compute_leaf(&ctx.accounts.claimer.key(), index, amount, &id);
    require!(
        verify_proof(&proof, leaf, slot.root),
        ProtocolError::InvalidProof
    );

    require!(
        ctx.accounts.treasury_ata.amount >= amount,
        ProtocolError::InvalidAmount
    );

    let signer_seeds: &[&[u8]] = &[
        PROTOCOL_SEED,
        ctx.accounts.protocol_state.mint.as_ref(),
        &[ctx.accounts.protocol_state.bump],
    ];

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.treasury_ata.to_account_info(),
                to: ctx.accounts.claimer_ata.to_account_info(),
                authority: ctx.accounts.protocol_state.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
            &[signer_seeds],
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    slot.set_bit(index as usize);

    Ok(())
}

fn authorize_publisher(protocol: &ProtocolState, signer: &Pubkey) -> Result<()> {
    let is_admin = *signer == protocol.admin;
    let is_publisher = protocol.publisher != Pubkey::default() && *signer == protocol.publisher;
    require!(is_admin || is_publisher, ProtocolError::Unauthorized);
    Ok(())
}
