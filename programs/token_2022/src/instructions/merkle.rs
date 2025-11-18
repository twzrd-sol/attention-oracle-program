use crate::{
    constants::{EPOCH_STATE_SEED, PROTOCOL_SEED},
    errors::MiloError,
    state::{EpochState, ProtocolState},
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(root: [u8;32], epoch: u64, claim_count: u32, streamer_key: Pubkey)]
pub struct SetMerkleRoot<'info> {
    #[account(mut)]
    pub update_authority: Signer<'info>,

    /// Global protocol state (authority + mint/treasury refs)
    #[account(
        mut,
        seeds = [PROTOCOL_SEED],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Epoch state for this (streamer, epoch)
    #[account(
        init_if_needed,
        payer = update_authority,
        space = EpochState::space_for(claim_count as usize),
        seeds = [EPOCH_STATE_SEED, &epoch.to_le_bytes(), streamer_key.as_ref()],
        bump
    )]
    pub epoch_state: Account<'info, EpochState>,

    pub system_program: Program<'info, System>,
}

pub fn set_merkle_root(
    ctx: Context<SetMerkleRoot>,
    root: [u8; 32],
    epoch: u64,
    claim_count: u32,
    streamer_key: Pubkey,
) -> Result<()> {
    let protocol = &ctx.accounts.protocol_state;
    // Authorization: admin or allowlisted publisher
    let signer = ctx.accounts.update_authority.key();
    let is_admin = signer == protocol.admin;
    let is_publisher = protocol.publisher != Pubkey::default() && signer == protocol.publisher;
    require!(is_admin || is_publisher, MiloError::Unauthorized);
    require!(!protocol.paused, MiloError::ProtocolPaused);

    let epoch_state = &mut ctx.accounts.epoch_state;
    let ts = Clock::get()?.unix_timestamp;

    // Prevent re-initialization of an active epoch
    require!(
        epoch_state.timestamp == 0,
        MiloError::EpochAlreadyInitialized
    );

    // Initialize/overwrite epoch fields
    epoch_state.epoch = epoch;
    epoch_state.root = root;
    epoch_state.claim_count = claim_count;
    epoch_state.mint = protocol.mint;
    epoch_state.streamer = streamer_key;
    epoch_state.treasury = protocol.treasury; // PDA authority that owns the treasury ATA
    epoch_state.timestamp = ts;
    epoch_state.total_claimed = 0;
    epoch_state.closed = false;

    // Resize/clear bitmap
    let need = ((claim_count as usize + 7) / 8).max(1);
    epoch_state.claimed_bitmap = vec![0u8; need];

    Ok(())
}

// Open variant: protocol_state keyed by mint; epoch_state seeds include mint to avoid collisions
#[derive(Accounts)]
#[instruction(root: [u8;32], epoch: u64, claim_count: u32, streamer_key: Pubkey)]
pub struct SetMerkleRootOpen<'info> {
    #[account(mut)]
    pub update_authority: Signer<'info>,

    /// Global protocol state keyed by mint
    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Epoch state for (streamer, epoch, mint)
    #[account(
        init_if_needed,
        payer = update_authority,
        space = EpochState::space_for(claim_count as usize),
        seeds = [EPOCH_STATE_SEED, &epoch.to_le_bytes(), streamer_key.as_ref(), protocol_state.mint.as_ref()],
        bump
    )]
    pub epoch_state: Account<'info, EpochState>,

    pub system_program: Program<'info, System>,
}

pub fn set_merkle_root_open(
    ctx: Context<SetMerkleRootOpen>,
    root: [u8; 32],
    epoch: u64,
    claim_count: u32,
    streamer_key: Pubkey,
) -> Result<()> {
    let protocol = &ctx.accounts.protocol_state;
    // Authorization: admin or allowlisted publisher
    let signer = ctx.accounts.update_authority.key();
    let is_admin = signer == protocol.admin;
    let is_publisher = protocol.publisher != Pubkey::default() && signer == protocol.publisher;
    require!(is_admin || is_publisher, MiloError::Unauthorized);
    require!(!protocol.paused, MiloError::ProtocolPaused);

    let epoch_state = &mut ctx.accounts.epoch_state;
    let ts = Clock::get()?.unix_timestamp;

    // Prevent re-initialization of an active epoch
    require!(
        epoch_state.timestamp == 0,
        MiloError::EpochAlreadyInitialized
    );

    epoch_state.epoch = epoch;
    epoch_state.root = root;
    epoch_state.claim_count = claim_count;
    epoch_state.mint = protocol.mint;
    epoch_state.streamer = streamer_key;
    epoch_state.treasury = protocol.treasury;
    epoch_state.timestamp = ts;
    epoch_state.total_claimed = 0;
    epoch_state.closed = false;

    let need = ((claim_count as usize + 7) / 8).max(1);
    epoch_state.claimed_bitmap = vec![0u8; need];

    Ok(())
}
