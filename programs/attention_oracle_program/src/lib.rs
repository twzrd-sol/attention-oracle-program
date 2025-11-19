use anchor_lang::prelude::*;
use sha3::{Digest, Keccak256};

fn keccak_hash(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(bytes);
    let out = hasher.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out[..32]);
    arr
}

fn keccak_hashv(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    for p in parts {
        hasher.update(p);
    }
    let out = hasher.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out[..32]);
    arr
}

declare_id!("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");

#[program]
pub mod attention_oracle_program {
    use super::*;

    pub fn update_root(
        ctx: Context<UpdateRoot>,
        channel: String,
        epoch: u64,
        root: [u8; 32],
        total_amount: u128,
    ) -> Result<()> {
        require!(channel.len() <= 64, ErrorCode::ChannelTooLong);

        let epoch_root = &mut ctx.accounts.epoch_root;
        epoch_root.channel = keccak_hash(channel.as_bytes());
        epoch_root.epoch = epoch;
        epoch_root.root = root;
        epoch_root.total_amount = total_amount as u64;
        epoch_root.bump = *ctx.bumps.get("epoch_root").unwrap_or(&0);
        Ok(())
    }

    #[allow(clippy::explicit_iter_loop)]
    pub fn claim(
        ctx: Context<Claim>,
        channel: String,
        epoch: u64,
        index: u32,
        amount: u64,
        id: String,
        merkle_proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        require!(channel.len() <= 64, ErrorCode::ChannelTooLong);
        require!(id.len() <= 64, ErrorCode::IdTooLong);

        let leaf = keccak_hashv(&[
            ctx.accounts.claimer.key().as_ref(),
            &index.to_le_bytes(),
            &amount.to_le_bytes(),
            id.as_bytes(),
        ]);

        let mut computed_hash = leaf;

        for proof_elem in merkle_proof.iter() {
            let proof_bytes = *proof_elem;
            let pair = if computed_hash <= proof_bytes {
                [computed_hash, proof_bytes]
            } else {
                [proof_bytes, computed_hash]
            };
            computed_hash = keccak_hashv(&[&pair[0], &pair[1]]);
        }

        require_eq!(computed_hash, ctx.accounts.epoch_root.root, ErrorCode::InvalidProof);

        // Double-claim protection via bitmap
        let byte_idx = (index / 8) as usize;
        let bit_idx = (index % 8) as u8;
        require!(byte_idx < ctx.accounts.epoch_root.claimed_bitmap.len(), ErrorCode::IndexOutOfBounds);
        require!(
            (ctx.accounts.epoch_root.claimed_bitmap[byte_idx] & (1 << bit_idx)) == 0,
            ErrorCode::AlreadyClaimed
        );
        // mark claimed
        ctx.accounts.epoch_root.claimed_bitmap[byte_idx] |= 1 << bit_idx;

        // Payout using epoch_root PDA as authority over treasury
        let channel_hash = keccak_hash(channel.as_bytes());
        let epoch_bytes = epoch.to_le_bytes();
        let signer_seeds: &[&[u8]] = &[b"epoch_root", &channel_hash, &epoch_bytes, &[ctx.accounts.epoch_root.bump]];
        let signer = &[signer_seeds];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.treasury.to_account_info(),
                to: ctx.accounts.claimer_ata.to_account_info(),
                authority: ctx.accounts.epoch_root.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
            signer,
        );
        token_interface::transfer_checked(cpi_ctx, amount, ctx.accounts.mint.decimals)?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(channel: String, epoch: u64, root: [u8;32], total_amount: u128)]
pub struct UpdateRoot<'info> {
    #[account(mut)]
    pub oracle_authority: Signer<'info>,

    #[account(
        init_if_needed,
        payer = oracle_authority,
        space = EpochRoot::LEN,
        seeds = [b"epoch_root", &keccak_hash(channel.as_bytes()), &epoch.to_le_bytes()],
        bump,
    )]
    pub epoch_root: Account<'info, EpochRoot>,

    pub system_program: Program<'info, System>,
}

use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};

#[derive(Accounts)]
#[instruction(channel: String, epoch: u64, index: u32, amount: u64, id: String, merkle_proof: Vec<[u8;32]>)]
pub struct Claim<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"epoch_root", &keccak_hash(channel.as_bytes()), &epoch.to_le_bytes()],
        bump = epoch_root.bump,
    )]
    pub epoch_root: Account<'info, EpochRoot>,

    #[account(mut)]
    pub treasury: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub claimer_ata: InterfaceAccount<'info, TokenAccount>,

    pub mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[account]
pub struct EpochRoot {
    pub channel: [u8; 32],
    pub epoch: u64,
    pub root: [u8; 32],
    pub total_amount: u64,
    pub bump: u8,
    pub claimed_bitmap: [u8; 512],
}

impl EpochRoot {
    pub const LEN: usize = 8 /*disc*/ + 32 + 8 + 32 + 8 + 1 + 512;
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid Merkle proof")]
    InvalidProof,
    #[msg("Index out of bounds")]
    IndexOutOfBounds,
    #[msg("Merkle leaf already claimed")]
    AlreadyClaimed,
    #[msg("Channel string too long")]
    ChannelTooLong,
    #[msg("ID string too long")]
    IdTooLong,
}
