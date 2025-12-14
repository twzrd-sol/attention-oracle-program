use crate::{constants::{ADMIN_AUTHORITY, PROTOCOL_SEED}, errors::OracleError, state::ProtocolState};
use anchor_lang::prelude::*;
use anchor_spl::token_2022::{self, MintTo, Token2022};
use anchor_spl::token_interface::{Mint, TokenAccount};

/// Update the allowlisted publisher (singleton protocol_state)
#[derive(Accounts)]
pub struct UpdatePublisher<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_publisher(ctx: Context<UpdatePublisher>, new_publisher: Pubkey) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.publisher = new_publisher;
    Ok(())
}

/// Update the allowlisted publisher (open variant keyed by mint)
#[derive(Accounts)]
pub struct UpdatePublisherOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
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
    state.publisher = new_publisher;
    Ok(())
}

/// Set receipt requirement policy (singleton protocol_state)
#[derive(Accounts)]
pub struct SetPolicy<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_policy(ctx: Context<SetPolicy>, require_receipt: bool) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.require_receipt = require_receipt;
    Ok(())
}

/// Set receipt requirement policy (open variant keyed by mint)
#[derive(Accounts)]
pub struct SetPolicyOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_policy_open(ctx: Context<SetPolicyOpen>, require_receipt: bool) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.require_receipt = require_receipt;
    Ok(())
}

/// Emergency pause/unpause (singleton protocol_state)
#[derive(Accounts)]
pub struct SetPaused<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_paused(ctx: Context<SetPaused>, paused: bool) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.paused = paused;
    Ok(())
}

/// Emergency pause/unpause (open variant keyed by mint)
#[derive(Accounts)]
pub struct SetPausedOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_paused_open(ctx: Context<SetPausedOpen>, paused: bool) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.paused = paused;
    Ok(())
}

/// Transfer admin authority (open variant keyed by mint)
/// Used for migrating to hardware wallet or new admin key
#[derive(Accounts)]
pub struct UpdateAdminOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_admin_open(ctx: Context<UpdateAdminOpen>, new_admin: Pubkey) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.admin = new_admin;
    Ok(())
}

/// Transfer admin authority (singleton variant)
#[derive(Accounts)]
pub struct UpdateAdmin<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_admin(ctx: Context<UpdateAdmin>, new_admin: Pubkey) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.admin = new_admin;
    Ok(())
}

// =============================================================================
// BOOTSTRAP MINT (V2)
// =============================================================================

/// Admin mint for v2 bootstrap (before protocol_state initialized)
/// Uses hardcoded ADMIN_AUTHORITY and derives PDA bump on-the-fly.
/// Mint authority must be the protocol PDA: ["protocol", mint]
#[derive(Accounts)]
pub struct AdminMintV2<'info> {
    #[account(
        mut,
        constraint = admin.key() == ADMIN_AUTHORITY @ OracleError::Unauthorized,
    )]
    pub admin: Signer<'info>,

    /// The v2 mint (mint_authority must be protocol PDA)
    #[account(mut)]
    pub mint: InterfaceAccount<'info, Mint>,

    /// Treasury token account to receive minted tokens
    #[account(
        mut,
        token::mint = mint,
        token::token_program = token_program,
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: PDA signer for mint authority. Seeds validated in handler.
    #[account(
        seeds = [PROTOCOL_SEED, mint.key().as_ref()],
        bump,
    )]
    pub mint_authority: AccountInfo<'info>,

    pub token_program: Program<'info, Token2022>,
}

pub fn admin_mint_v2(ctx: Context<AdminMintV2>, amount: u64) -> Result<()> {
    let mint_key = ctx.accounts.mint.key();
    let seeds: &[&[u8]] = &[
        PROTOCOL_SEED,
        mint_key.as_ref(),
        &[ctx.bumps.mint_authority],
    ];

    token_2022::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.treasury_ata.to_account_info(),
                authority: ctx.accounts.mint_authority.to_account_info(),
            },
            &[seeds],
        ),
        amount,
    )?;

    msg!("Admin minted {} tokens to treasury", amount);
    Ok(())
}
