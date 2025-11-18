use crate::{
    constants::{CHANNEL_STATE_SEED, PROTOCOL_SEED},
    errors::OracleError,
    state::{ChannelState, ProtocolState},
};
use anchor_lang::prelude::*;

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

    // Authorization check
    let signer = ctx.accounts.update_authority.key();
    let is_admin = signer == protocol.admin;
    let is_publisher = protocol.publisher != Pubkey::default() && signer == protocol.publisher;
    require!(is_admin || is_publisher, OracleError::Unauthorized);
    require!(!protocol.paused, OracleError::ProtocolPaused);

    let channel_state = &mut ctx.accounts.channel_state.load_mut()?;

    // Verify channel was initialized
    require!(
        channel_state.version > 0,
        OracleError::ChannelNotInitialized
    );
    require!(
        channel_state.streamer == streamer_key,
        OracleError::InvalidStreamer
    );

    // Update ring buffer slot (modulo 10)
    let slot = channel_state.slot_mut(epoch);
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
#[instruction(epoch: u64, index: u32, amount: u64, proof: Vec<[u8; 32]>, streamer_key: Pubkey)]
pub struct ClaimWithRing<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ OracleError::ProtocolPaused,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        mut,
        seeds = [CHANNEL_STATE_SEED, protocol_state.mint.as_ref(), streamer_key.as_ref()],
        bump
    )]
    pub channel_state: AccountLoader<'info, ChannelState>,

    // Token accounts would go here for actual claim
    // Simplified for demonstration
    pub system_program: Program<'info, System>,
}

pub fn claim_with_ring(
    ctx: Context<ClaimWithRing>,
    epoch: u64,
    index: u32,
    amount: u64,
    proof: Vec<[u8; 32]>,
    streamer_key: Pubkey,
) -> Result<()> {
    let channel_state = &mut ctx.accounts.channel_state.load_mut()?;

    // Verify channel initialized
    require!(
        channel_state.version > 0,
        OracleError::ChannelNotInitialized
    );
    require!(
        channel_state.streamer == streamer_key,
        OracleError::InvalidStreamer
    );

    // Get the slot for this epoch
    let slot = channel_state.slot_mut(epoch);

    // Verify epoch matches
    require!(slot.epoch == epoch, OracleError::InvalidEpoch);

    // Check if already claimed
    require!(!slot.test_bit(index as usize), OracleError::AlreadyClaimed);

    // Verify merkle proof (would use actual verification here)
    // For now just a placeholder
    require!(proof.len() > 0, OracleError::InvalidProof);

    // Mark as claimed
    slot.set_bit(index as usize);

    // Transfer tokens would happen here
    msg!(
        "Claimed {} tokens for epoch {} index {}",
        amount,
        epoch,
        index
    );

    Ok(())
}

/// Close old epoch state accounts to recover rent
#[derive(Accounts)]
pub struct CloseOldEpochState<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// The old epoch_state account to close
    /// CHECK: We're closing this account, so we just need it to exist
    #[account(mut)]
    pub epoch_state: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

pub fn close_old_epoch_state(ctx: Context<CloseOldEpochState>) -> Result<()> {
    // Transfer lamports to admin
    let epoch_state = &mut ctx.accounts.epoch_state;
    let admin = &mut ctx.accounts.admin;

    let lamports = epoch_state.lamports();
    **epoch_state.try_borrow_mut_lamports()? -= lamports;
    **admin.try_borrow_mut_lamports()? += lamports;

    // Invalidate account
    epoch_state.assign(&System::id());
    epoch_state.resize(0)?;

    msg!("Closed old epoch state, recovered {} lamports", lamports);

    Ok(())
}
