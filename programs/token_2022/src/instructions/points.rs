use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{self, Mint, MintTo, TokenAccount, TokenInterface},
};

use crate::{
    constants::PROTOCOL_SEED,
    errors::MiloError,
    state::{EpochState, ProtocolState},
};

#[derive(Accounts)]
pub struct ClaimPointsOpen<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    /// Global protocol state keyed by mint (PDA authority for minting points)
    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ MiloError::ProtocolPaused,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Epoch state holding Merkle root and claim bitmap
    #[account(mut)]
    pub epoch_state: Account<'info, EpochState>,

    /// Points mint (Token-2022 NonTransferable recommended)
    #[account(mut)]
    pub points_mint: InterfaceAccount<'info, Mint>,

    /// Claimer ATA for points
    #[account(
        init_if_needed,
        payer = claimer,
        associated_token::mint = points_mint,
        associated_token::authority = claimer,
        associated_token::token_program = token_program
    )]
    pub claimer_points_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_points_open(
    ctx: Context<ClaimPointsOpen>,
    index: u32,
    amount: u64,
    id: String,
    proof: Vec<[u8; 32]>,
) -> Result<()> {
    let protocol = &ctx.accounts.protocol_state;
    let epoch = &mut ctx.accounts.epoch_state;

    // Guards
    require!(!epoch.closed, MiloError::EpochClosed);

    // Optional: ensure points are for the same protocol instance by convention
    // (we allow any points mint as long as authority signs via PDA)

    // Check bitmap not already claimed
    let byte_i = (index / 8) as usize;
    let bit = 1u8 << (index % 8);
    require!(byte_i < epoch.claimed_bitmap.len(), MiloError::InvalidIndex);
    require!(
        epoch.claimed_bitmap[byte_i] & bit == 0,
        MiloError::AlreadyClaimed
    );

    // Verify proof
    let leaf = super::claim::compute_leaf(&ctx.accounts.claimer.key(), index, amount, &id);
    require!(
        super::claim::verify_proof(&proof, leaf, epoch.root),
        MiloError::InvalidProof
    );

    // Mint points to claimer using PDA authority
    let seeds: &[&[u8]] = &[PROTOCOL_SEED, protocol.mint.as_ref(), &[protocol.bump]];
    let signer = &[seeds];

    token_interface::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.points_mint.to_account_info(),
                to: ctx.accounts.claimer_points_ata.to_account_info(),
                authority: ctx.accounts.protocol_state.to_account_info(),
            },
            signer,
        ),
        amount,
    )?;

    // Mark claimed and bump totals (epoch total_claimed tracks CCM; here we only mark claimed)
    epoch.claimed_bitmap[byte_i] |= bit;

    Ok(())
}

#[derive(Accounts)]
pub struct RequirePoints<'info> {
    /// Wallet being checked (no signer needed)
    /// CHECK: only used as ATA authority check via constraints below
    pub owner: UncheckedAccount<'info>,

    /// Points mint
    pub points_mint: InterfaceAccount<'info, Mint>,

    /// Owner's points ATA
    #[account(
        associated_token::mint = points_mint,
        associated_token::authority = owner,
        associated_token::token_program = token_program
    )]
    pub points_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

/// Require that `owner` has at least `min` points
pub fn require_points_ge(ctx: Context<RequirePoints>, min: u64) -> Result<()> {
    let balance = ctx.accounts.points_ata.amount;
    require!(balance >= min, MiloError::InsufficientPoints);
    Ok(())
}
