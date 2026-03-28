#[cfg(feature = "channel_staking")]
use crate::{
    constants::{CHANNEL_CONFIG_V2_SEED, CUMULATIVE_ROOT_HISTORY},
    state::{ChannelConfigV2, RootEntry},
};
use crate::{
    errors::OracleError,
    events::{ProtocolPaused, PublisherUpdated},
    state::ProtocolState,
};
use anchor_lang::prelude::*;

/// Update the allowlisted publisher (keyed by mint)
#[derive(Accounts)]
pub struct UpdatePublisherOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_publisher_open(
    ctx: Context<UpdatePublisherOpen>,
    new_publisher: Pubkey,
) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    let old_publisher = state.publisher;
    state.publisher = new_publisher;

    emit!(PublisherUpdated {
        admin: ctx.accounts.admin.key(),
        old_publisher,
        new_publisher,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Emergency pause/unpause (keyed by mint)
#[derive(Accounts)]
pub struct SetPausedOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_paused_open(ctx: Context<SetPausedOpen>, paused: bool) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.paused = paused;

    emit!(ProtocolPaused {
        admin: ctx.accounts.admin.key(),
        paused,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

// =============================================================================
// TREASURY WITHDRAW - REMOVED
// =============================================================================
// Admin withdrawal capability was removed after Initial Liquidity Offering.
// Treasury is now locked to claim-based distribution only.
// See: https://solscan.io/tx/L53wKdRPTYKCwR1DJJQjFr34SYsCzjqcyNgXP7BbZAV7Yasz7bDwqP2no6ozm7tLVMawUcADGhZPXRNe4wQajeh
// =============================================================================

// =============================================================================
// SET TREASURY (Fee Destination Owner)
// =============================================================================

#[event]
pub struct TreasuryUpdated {
    pub admin: Pubkey,
    pub old_treasury: Pubkey,
    pub new_treasury: Pubkey,
    pub mint: Pubkey,
    pub timestamp: i64,
}

/// Update the treasury wallet (fee destination owner).
/// The treasury field stores the OWNER of the fee destination token account.
/// harvest_fees will send withheld fees to ATA(treasury, mint).
#[derive(Accounts)]
pub struct SetTreasury<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_treasury(ctx: Context<SetTreasury>, new_treasury: Pubkey) -> Result<()> {
    require!(
        new_treasury != Pubkey::default(),
        OracleError::InvalidPubkey
    );
    let state = &mut ctx.accounts.protocol_state;
    let old_treasury = state.treasury;
    state.treasury = new_treasury;

    emit!(TreasuryUpdated {
        admin: ctx.accounts.admin.key(),
        old_treasury,
        new_treasury,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!("Treasury updated: {} -> {}", old_treasury, new_treasury);

    Ok(())
}

// =============================================================================
// CREATE CHANNEL CONFIG V2 — Initialize a ChannelConfigV2 PDA (Phase 2)
// =============================================================================

#[cfg(feature = "channel_staking")]
#[derive(Accounts)]
#[instruction(subject: Pubkey)]
pub struct CreateChannelConfigV2<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        init,
        payer = admin,
        space = ChannelConfigV2::LEN,
        seeds = [CHANNEL_CONFIG_V2_SEED, protocol_state.mint.as_ref(), subject.as_ref()],
        bump,
    )]
    pub channel_config: Account<'info, ChannelConfigV2>,

    pub system_program: Program<'info, System>,
}

#[cfg(feature = "channel_staking")]
pub fn create_channel_config_v2(
    ctx: Context<CreateChannelConfigV2>,
    subject: Pubkey,
    authority: Pubkey,
    creator_wallet: Pubkey,
    creator_fee_bps: u16,
) -> Result<()> {
    let config = &mut ctx.accounts.channel_config;
    config.version = 1;
    config.bump = ctx.bumps.channel_config;
    config.mint = ctx.accounts.protocol_state.mint;
    config.subject = subject;
    config.authority = authority;
    config.latest_root_seq = 0;
    config.cutover_epoch = 0;
    config.creator_wallet = creator_wallet;
    config.creator_fee_bps = creator_fee_bps;
    config._padding = [0u8; 6];
    config.roots = [RootEntry::default(); CUMULATIVE_ROOT_HISTORY];

    msg!(
        "ChannelConfigV2 created: subject={}, authority={}, mint={}",
        subject,
        authority,
        config.mint
    );

    Ok(())
}
