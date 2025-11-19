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

declare_id!("Attn1111111111111111111111111111111111111111");

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
        epoch_root.root = root;
        epoch_root.total_amount = total_amount;
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

        // TODO: mark claimed + payout (vault transfer or mint)
        // Recommended claimed PDA:
        // seeds = [b"claimed", &keccak::hash(channel.as_bytes()).to_bytes(), &epoch.to_le_bytes(), id.as_bytes()], bump

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
        space = 8 + 32 + 16 + 1,
        seeds = [b"epoch_root", &keccak_hash(channel.as_bytes()), &epoch.to_le_bytes()],
        bump,
    )]
    pub epoch_root: Account<'info, EpochRoot>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(channel: String, epoch: u64, index: u32, amount: u64, id: String, merkle_proof: Vec<[u8;32]>)]
pub struct Claim<'info> {
    #[account(
        seeds = [b"epoch_root", &keccak_hash(channel.as_bytes()), &epoch.to_le_bytes()],
        bump,
    )]
    pub epoch_root: Account<'info, EpochRoot>,

    pub claimer: Signer<'info>,
    // + token accounts / mint authority PDA here for payout
}

#[account]
pub struct EpochRoot {
    pub root: [u8; 32],
    pub total_amount: u128,
    pub bump: u8,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid Merkle proof")]
    InvalidProof,
    #[msg("Channel string too long")]
    ChannelTooLong,
    #[msg("ID string too long")]
    IdTooLong,
}
