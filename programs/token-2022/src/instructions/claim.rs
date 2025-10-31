use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked},
};
use solana_program::keccak;

use crate::{
    constants::PROTOCOL_SEED,
    errors::ProtocolError,
    instructions::cnft_verify::CnftReceiptProof,
    state::{EpochState, ProtocolState},
};

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    /// Global protocol state (PDA authority over treasury ATA)
    #[account(
        mut,
        seeds = [PROTOCOL_SEED],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ ProtocolError::ProtocolPaused,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// Epoch state (root + bitmap). Mutable for marking claims.
    #[account(mut)]
    pub epoch_state: Account<'info, EpochState>,

    /// CCM mint (Token‑2022)
    pub mint: InterfaceAccount<'info, Mint>,

    /// Treasury ATA (owned by protocol_state PDA)
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = protocol_state,
        associated_token::token_program = token_program
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,

    /// Claimer ATA (create if needed)
    #[account(
        init_if_needed,
        payer = claimer,
        associated_token::mint = mint,
        associated_token::authority = claimer,
        associated_token::token_program = token_program
    )]
    pub claimer_ata: InterfaceAccount<'info, TokenAccount>,

    /// Optional: ORACLE cNFT receipt (if provided, verifies L1 participation)
    /// CHECK: Optional account, validated in instruction if present
    pub twzrd_receipt: Option<UncheckedAccount<'info>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim(
    ctx: Context<Claim>,
    _streamer_index: u8,
    index: u32,
    amount: u64,
    id: String,
    proof: Vec<[u8; 32]>,
) -> Result<()> {
    let epoch = &mut ctx.accounts.epoch_state;

    // Basic guards
    require!(!epoch.closed, ProtocolError::EpochClosed);
    // Index must be strictly within published claim_count
    require!(index < epoch.claim_count as u32, ProtocolError::InvalidIndex);
    require!(
        epoch.mint == ctx.accounts.mint.key(),
        ProtocolError::InvalidMint
    );

    // Validate epoch_state PDA seeds to prevent spoofing
    let expected = Pubkey::find_program_address(
        &[
            crate::constants::EPOCH_STATE_SEED,
            &epoch.epoch.to_le_bytes(),
            epoch.streamer.as_ref(),
        ],
        ctx.program_id,
    )
    .0;
    require_keys_eq!(expected, ctx.accounts.epoch_state.key(), ProtocolError::InvalidEpochState);

    // Check bitmap not already claimed
    let byte_i = (index / 8) as usize;
    let bit = 1u8 << (index % 8);
    require!(byte_i < epoch.claimed_bitmap.len(), ProtocolError::InvalidIndex);
    require!(
        epoch.claimed_bitmap[byte_i] & bit == 0,
        ProtocolError::AlreadyClaimed
    );

    // Verify proof against on-chain root
    let leaf = compute_leaf(&ctx.accounts.claimer.key(), index, amount, &id);
    require!(
        verify_proof(&proof, leaf, epoch.root),
        ProtocolError::InvalidProof
    );

    // Transfer CCM from treasury PDA to claimer (use transfer_checked for Token-2022)
    let seeds: &[&[u8]] = &[PROTOCOL_SEED, &[ctx.accounts.protocol_state.bump]];
    let signer = &[seeds];

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.treasury_ata.to_account_info(),
                to: ctx.accounts.claimer_ata.to_account_info(),
                authority: ctx.accounts.protocol_state.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
            signer,
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    // Mark claimed and bump totals
    epoch.claimed_bitmap[byte_i] |= bit;
    epoch.total_claimed = epoch.total_claimed.saturating_add(amount);

    Ok(())
}

// Open variant: protocol_state keyed by mint
#[derive(Accounts)]
pub struct ClaimOpen<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    #[account(
        mut,
        seeds = [crate::constants::PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ ProtocolError::ProtocolPaused,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(mut)]
    pub epoch_state: Account<'info, EpochState>,

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

    /// Optional: ORACLE epoch state (for cNFT verification)
    /// CHECK: Optional account, validated in instruction if receipt required
    pub twzrd_epoch_state: Option<UncheckedAccount<'info>>,

    /// Optional: Bubblegum tree (for cNFT verification)
    /// CHECK: Optional account, validated in instruction if receipt required
    pub merkle_tree: Option<UncheckedAccount<'info>>,

    /// Optional: SPL Account Compression program (for cNFT verification)
    /// CHECK: Optional account, validated in instruction if receipt required
    pub compression_program: Option<UncheckedAccount<'info>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_open(
    ctx: Context<ClaimOpen>,
    _streamer_index: u8,
    index: u32,
    amount: u64,
    id: String,
    proof: Vec<[u8; 32]>,
    // Optional: cNFT receipt proof (for L1 verification)
    channel: Option<String>,
    twzrd_epoch: Option<u64>,
    receipt_proof: Option<CnftReceiptProof>,
) -> Result<()> {
    let epoch = &mut ctx.accounts.epoch_state;
    require!(!epoch.closed, ProtocolError::EpochClosed);
    require!(index < epoch.claim_count as u32, ProtocolError::InvalidIndex);
    require!(
        epoch.mint == ctx.accounts.mint.key(),
        ProtocolError::InvalidMint
    );

    // Step 1: If receipt required, verify ORACLE L1 participation
    if ctx.accounts.protocol_state.require_receipt {
        require!(
            channel.is_some() && twzrd_epoch.is_some() && receipt_proof.is_some(),
            ProtocolError::ReceiptRequired
        );

        let receipt = receipt_proof.as_ref().unwrap();
        let chan_ref = channel.as_ref().unwrap();
        let epoch_val = twzrd_epoch.unwrap();

        crate::instructions::cnft_verify::verify_cnft_receipt(
            receipt,
            ctx.accounts.claimer.key,
            chan_ref,
            epoch_val,
        )?;

        msg!(
            "L1 receipt verified: channel={}, epoch={}",
            chan_ref,
            epoch_val
        );
    }

    // Validate epoch_state PDA seeds to prevent spoofing (mint-keyed open variant)
    let expected = Pubkey::find_program_address(
        &[
            crate::constants::EPOCH_STATE_SEED,
            &epoch.epoch.to_le_bytes(),
            epoch.streamer.as_ref(),
            ctx.accounts.protocol_state.mint.as_ref(),
        ],
        ctx.program_id,
    )
    .0;
    require_keys_eq!(expected, ctx.accounts.epoch_state.key(), ProtocolError::InvalidEpochState);

    // Step 2: Verify merkle proof for CCM claim
    let byte_i = (index / 8) as usize;
    let bit = 1u8 << (index % 8);
    require!(byte_i < epoch.claimed_bitmap.len(), ProtocolError::InvalidIndex);
    require!(
        epoch.claimed_bitmap[byte_i] & bit == 0,
        ProtocolError::AlreadyClaimed
    );

    let leaf = compute_leaf(&ctx.accounts.claimer.key(), index, amount, &id);
    require!(
        verify_proof(&proof, leaf, epoch.root),
        ProtocolError::InvalidProof
    );

    // Step 3: Transfer CCM tokens
    let seeds: &[&[u8]] = &[
        crate::constants::PROTOCOL_SEED,
        ctx.accounts.protocol_state.mint.as_ref(),
        &[ctx.accounts.protocol_state.bump],
    ];
    let signer = &[seeds];

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.treasury_ata.to_account_info(),
                to: ctx.accounts.claimer_ata.to_account_info(),
                authority: ctx.accounts.protocol_state.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
            signer,
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    // Step 4: Mark claimed
    epoch.claimed_bitmap[byte_i] |= bit;
    epoch.total_claimed = epoch.total_claimed.saturating_add(amount);
    Ok(())
}

pub fn compute_leaf(claimer: &Pubkey, index: u32, amount: u64, id: &str) -> [u8; 32] {
    // Note: Off-chain must mirror this exact hashing scheme
    let mut idx = index.to_le_bytes();
    let mut amt = amount.to_le_bytes();
    let id_bytes = id.as_bytes();
    keccak::hashv(&[claimer.as_ref(), &idx, &amt, id_bytes]).to_bytes()
}

pub fn verify_proof(proof: &[[u8; 32]], mut hash: [u8; 32], root: [u8; 32]) -> bool {
    for sibling in proof.iter() {
        let (a, b) = if hash <= *sibling {
            (hash, *sibling)
        } else {
            (*sibling, hash)
        };
        hash = keccak::hashv(&[&a, &b]).to_bytes();
    }
    hash == root
}
