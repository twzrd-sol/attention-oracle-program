use crate::{
    constants::{CHANNEL_META_SEED, CHANNEL_STATE_SEED, MAX_CREATOR_FEE_BPS, PROTOCOL_SEED},
    errors::OracleError,
    state::{ChannelMeta, ChannelState, ProtocolState},
};
use anchor_lang::prelude::*;

// =============================================================================
// EVENTS
// =============================================================================

#[event]
pub struct ChannelMetaInitialized {
    pub channel_state: Pubkey,
    pub creator_wallet: Pubkey,
    pub fee_share_bps: u16,
    pub timestamp: i64,
}

#[event]
pub struct CreatorWalletUpdated {
    pub channel_state: Pubkey,
    pub old_wallet: Pubkey,
    pub new_wallet: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct CreatorFeeShareUpdated {
    pub channel_state: Pubkey,
    pub old_fee_share_bps: u16,
    pub new_fee_share_bps: u16,
    pub timestamp: i64,
}

// =============================================================================
// INITIALIZE CHANNEL META
// =============================================================================

/// Initialize channel metadata for creator revenue sharing.
/// Can be called by admin or publisher for a given channel.
#[derive(Accounts)]
#[instruction(channel: String)]
pub struct InitializeChannelMeta<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Protocol state (verifies authority)
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = is_authorized(&protocol_state, &authority.key()) @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Existing channel state PDA
    #[account(
        seeds = [
            CHANNEL_STATE_SEED,
            protocol_state.mint.as_ref(),
            &subject_id_from_channel(&channel),
        ],
        bump,
    )]
    pub channel_state: AccountLoader<'info, ChannelState>,

    /// Channel metadata PDA (keyed by channel_state)
    #[account(
        init,
        payer = authority,
        space = ChannelMeta::LEN,
        seeds = [CHANNEL_META_SEED, channel_state.key().as_ref()],
        bump,
    )]
    pub channel_meta: Account<'info, ChannelMeta>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_channel_meta(
    ctx: Context<InitializeChannelMeta>,
    _channel: String,
    creator_wallet: Pubkey,
    fee_share_bps: u16,
) -> Result<()> {
    require!(fee_share_bps <= MAX_CREATOR_FEE_BPS, OracleError::CreatorFeeTooHigh);
    require!(creator_wallet != Pubkey::default(), OracleError::InvalidPubkey);

    let ts = Clock::get()?.unix_timestamp;
    let meta = &mut ctx.accounts.channel_meta;

    meta.version = 1;
    meta.bump = ctx.bumps.channel_meta;
    meta.channel_state = ctx.accounts.channel_state.key();
    meta.creator_wallet = creator_wallet;
    meta.fee_share_bps = fee_share_bps;
    meta.total_delegated = 0;
    meta._reserved = [0u8; 64];

    emit!(ChannelMetaInitialized {
        channel_state: meta.channel_state,
        creator_wallet,
        fee_share_bps,
        timestamp: ts,
    });

    msg!(
        "Channel meta initialized: channel_state={}, creator={}, fee_bps={}",
        meta.channel_state,
        creator_wallet,
        fee_share_bps
    );
    Ok(())
}

// =============================================================================
// SET CREATOR WALLET
// =============================================================================

/// Update the creator wallet for a channel.
/// Only admin or publisher can update.
#[derive(Accounts)]
pub struct SetCreatorWallet<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Protocol state (verifies authority)
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = is_authorized(&protocol_state, &authority.key()) @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel metadata PDA
    #[account(
        mut,
        seeds = [CHANNEL_META_SEED, channel_meta.channel_state.as_ref()],
        bump = channel_meta.bump,
    )]
    pub channel_meta: Account<'info, ChannelMeta>,
}

pub fn set_creator_wallet(ctx: Context<SetCreatorWallet>, new_wallet: Pubkey) -> Result<()> {
    require!(new_wallet != Pubkey::default(), OracleError::InvalidPubkey);

    let ts = Clock::get()?.unix_timestamp;
    let meta = &mut ctx.accounts.channel_meta;
    let old_wallet = meta.creator_wallet;

    meta.creator_wallet = new_wallet;

    emit!(CreatorWalletUpdated {
        channel_state: meta.channel_state,
        old_wallet,
        new_wallet,
        timestamp: ts,
    });

    msg!("Creator wallet updated: {} -> {}", old_wallet, new_wallet);
    Ok(())
}

// =============================================================================
// SET CREATOR FEE SHARE
// =============================================================================

/// Update the creator fee share for a channel.
/// Only admin can update.
#[derive(Accounts)]
pub struct SetCreatorFeeShare<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// Protocol state (verifies admin only, not publisher)
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel metadata PDA
    #[account(
        mut,
        seeds = [CHANNEL_META_SEED, channel_meta.channel_state.as_ref()],
        bump = channel_meta.bump,
    )]
    pub channel_meta: Account<'info, ChannelMeta>,
}

pub fn set_creator_fee_share(ctx: Context<SetCreatorFeeShare>, new_fee_share_bps: u16) -> Result<()> {
    require!(new_fee_share_bps <= MAX_CREATOR_FEE_BPS, OracleError::CreatorFeeTooHigh);

    let ts = Clock::get()?.unix_timestamp;
    let meta = &mut ctx.accounts.channel_meta;
    let old_fee_share_bps = meta.fee_share_bps;

    meta.fee_share_bps = new_fee_share_bps;

    emit!(CreatorFeeShareUpdated {
        channel_state: meta.channel_state,
        old_fee_share_bps,
        new_fee_share_bps,
        timestamp: ts,
    });

    msg!("Creator fee share updated: {} -> {} bps", old_fee_share_bps, new_fee_share_bps);
    Ok(())
}

// =============================================================================
// UPDATE TOTAL DELEGATED (Keeper/Aggregator)
// =============================================================================

/// Update the total delegated stake for a channel.
/// Publisher-only operation (called by aggregator after epoch cutoff).
#[derive(Accounts)]
pub struct UpdateTotalDelegated<'info> {
    #[account(mut)]
    pub publisher: Signer<'info>,

    /// Protocol state (verifies publisher)
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = is_publisher(&protocol_state, &publisher.key()) @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Channel metadata PDA
    #[account(
        mut,
        seeds = [CHANNEL_META_SEED, channel_meta.channel_state.as_ref()],
        bump = channel_meta.bump,
    )]
    pub channel_meta: Account<'info, ChannelMeta>,
}

pub fn update_total_delegated(ctx: Context<UpdateTotalDelegated>, total_delegated: u64) -> Result<()> {
    let meta = &mut ctx.accounts.channel_meta;
    meta.total_delegated = total_delegated;

    msg!(
        "Total delegated updated: channel_state={}, total={}",
        meta.channel_state,
        total_delegated
    );
    Ok(())
}

// =============================================================================
// HELPERS
// =============================================================================

/// Check if the signer is admin or publisher
fn is_authorized(state: &ProtocolState, signer: &Pubkey) -> bool {
    *signer == state.admin
        || (state.publisher != Pubkey::default() && *signer == state.publisher)
}

/// Check if the signer is the publisher
fn is_publisher(state: &ProtocolState, signer: &Pubkey) -> bool {
    state.publisher != Pubkey::default() && *signer == state.publisher
}

/// Derive subject_id from channel name (keccak256 hash)
fn subject_id_from_channel(channel: &str) -> [u8; 32] {
    use sha3::{Digest, Keccak256};
    let input = format!("channel:{}", channel.to_lowercase());
    let mut hasher = Keccak256::new();
    hasher.update(input.as_bytes());
    hasher.finalize().into()
}
