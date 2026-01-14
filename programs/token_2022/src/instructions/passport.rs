use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::OracleError;
use crate::events::*;
use crate::state::{PassportRegistry, ProtocolState};

#[derive(Accounts)]
#[instruction(user_hash: [u8; 32])]
pub struct MintPassportOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = protocol_state.admin == admin.key() @ OracleError::Unauthorized
    )]
    pub protocol_state: Account<'info, ProtocolState>,
    #[account(
        init,
        payer = admin,
        space = PassportRegistry::LEN,
        seeds = [PASSPORT_SEED, user_hash.as_ref()],
        bump
    )]
    pub registry: Account<'info, PassportRegistry>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(user_hash: [u8; 32])]
pub struct UpgradePassportOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = protocol_state.admin == admin.key() @ OracleError::Unauthorized
    )]
    pub protocol_state: Account<'info, ProtocolState>,
    #[account(
        mut,
        seeds = [PASSPORT_SEED, user_hash.as_ref()],
        bump = registry.bump,
        constraint = registry.user_hash == user_hash @ OracleError::InvalidUserHash
    )]
    pub registry: Account<'info, PassportRegistry>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(user_hash: [u8; 32])]
pub struct ReissuePassportOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = protocol_state.admin == admin.key() @ OracleError::Unauthorized
    )]
    pub protocol_state: Account<'info, ProtocolState>,
    #[account(
        mut,
        seeds = [PASSPORT_SEED, user_hash.as_ref()],
        bump = registry.bump,
        constraint = registry.user_hash == user_hash @ OracleError::InvalidUserHash
    )]
    pub registry: Account<'info, PassportRegistry>,
}

#[derive(Accounts)]
#[instruction(user_hash: [u8; 32])]
pub struct RevokePassportOpen<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
    #[account(
        mut,
        seeds = [PASSPORT_SEED, user_hash.as_ref()],
        bump = registry.bump,
        constraint = registry.user_hash == user_hash @ OracleError::InvalidUserHash,
        constraint = (authority.key() == registry.owner || authority.key() == protocol_state.admin) @ OracleError::Unauthorized
    )]
    pub registry: Account<'info, PassportRegistry>,
}


pub fn mint_passport_open(
    ctx: Context<MintPassportOpen>,
    user_hash: [u8; 32],
    owner: Pubkey,
    tier: u8,
    score: u64,
) -> Result<()> {
    let registry = &mut ctx.accounts.registry;
    let current_time = Clock::get()?.unix_timestamp;

    require!(tier <= MAX_TIER, OracleError::InvalidTier);

    registry.owner = owner;
    registry.user_hash = user_hash;
    registry.tier = tier;
    registry.score = score;
    registry.epoch_count = 0;
    registry.weighted_presence = 0;
    registry.badges = 0;
    registry.tree = Pubkey::default();
    registry.leaf_hash = None;
    registry.updated_at = current_time;
    registry.bump = ctx.bumps.registry;

    emit!(PassportMinted {
        user_hash,
        owner,
        tier,
        score,
        updated_at: current_time,
    });

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn upgrade_passport_open(
    ctx: Context<UpgradePassportOpen>,
    user_hash: [u8; 32],
    new_tier: u8,
    new_score: u64,
    epoch_count: u32,
    weighted_presence: u64,
    badges: u32,
    leaf_hash: Option<[u8; 32]>,
) -> Result<()> {
    let registry = &mut ctx.accounts.registry;
    let current_time = Clock::get()?.unix_timestamp;

    require!(
        registry.user_hash == user_hash,
        OracleError::InvalidUserHash
    );
    require!(
        new_tier >= registry.tier,
        OracleError::DowngradeNotAllowed
    );
    require!(
        new_score >= registry.score,
        OracleError::DowngradeNotAllowed
    );
    require!(new_tier <= MAX_TIER, OracleError::InvalidTier);

    registry.tier = new_tier;
    registry.score = new_score;
    registry.epoch_count = epoch_count;
    registry.weighted_presence = weighted_presence;
    registry.badges = badges;
    registry.leaf_hash = leaf_hash;
    registry.updated_at = current_time;

    emit!(PassportUpgraded {
        user_hash,
        owner: registry.owner,
        new_tier,
        new_score,
        epoch_count,
        weighted_presence,
        badges,
        leaf_hash,
        updated_at: current_time,
    });

    Ok(())
}

pub fn reissue_passport_open(
    ctx: Context<ReissuePassportOpen>,
    user_hash: [u8; 32],
    new_owner: Pubkey,
) -> Result<()> {
    let registry = &mut ctx.accounts.registry;
    let current_time = Clock::get()?.unix_timestamp;

    require!(
        registry.user_hash == user_hash,
        OracleError::InvalidUserHash
    );

    let old_owner = registry.owner;
    registry.owner = new_owner;
    registry.updated_at = current_time;

    emit!(PassportReissued {
        user_hash,
        old_owner,
        new_owner,
        updated_at: current_time,
    });

    Ok(())
}

pub fn revoke_passport_open(ctx: Context<RevokePassportOpen>, user_hash: [u8; 32]) -> Result<()> {
    let registry = &mut ctx.accounts.registry;
    let current_time = Clock::get()?.unix_timestamp;

    require!(
        registry.user_hash == user_hash,
        OracleError::InvalidUserHash
    );

    registry.tier = 0;
    registry.score = 0;
    registry.leaf_hash = None;
    registry.updated_at = current_time;

    emit!(PassportRevoked {
        user_hash,
        owner: registry.owner,
        updated_at: current_time,
    });

    Ok(())
}
