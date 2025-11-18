use anchor_lang::solana_program::{program::invoke_signed, rent::Rent, system_instruction, system_program};
use anchor_lang::{accounts::account::Account, prelude::*};
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked},
};

use crate::constants::{
    CHANNEL_BITMAP_BYTES, CHANNEL_MAX_CLAIMS, CHANNEL_RING_SLOTS, CHANNEL_STATE_SEED, PROTOCOL_SEED,
};
use crate::errors::MiloError;
use crate::instructions::claim::{compute_leaf, verify_proof};
use crate::state::{ChannelSlot, ChannelState, ProtocolState};
use anchor_lang::accounts::account_loader::AccountLoader;
use std::convert::TryInto;

const CHANNEL_STATE_VERSION: u8 = 1;

fn derive_streamer_key(channel: &str) -> Pubkey {
    let mut lower = channel.as_bytes().to_vec();
    // Convert ASCII bytes to lowercase in-place (avoids allocation for Unicode)
    lower.iter_mut().for_each(|b| *b = b.to_ascii_lowercase());
    // Brand- and platform-neutral prefix to derive a stable streamer key
    let hash = keccak_hashv(&[b"channel:", lower.as_slice()]);
    Pubkey::new_from_array(hash)
}

// Minimal Keccak-256 helpers (Solana 2.x split crates; use sha3 directly)
use sha3::{Digest, Keccak256};
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

#[derive(Accounts)]
pub struct SetChannelMerkleRoot<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ MiloError::ProtocolPaused,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// PDA derived from (mint, channel) - zero_copy account
    /// CHECK: PDA is validated via derivation in handler
    #[account(mut)]
    pub channel_state: AccountLoader<'info, ChannelState>,

    pub system_program: Program<'info, System>,
}

pub fn set_channel_merkle_root(
    ctx: Context<SetChannelMerkleRoot>,
    channel: String,
    epoch: u64,
    root: [u8; 32],
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;
    // Authorization: admin or allowlisted publisher
    let signer = ctx.accounts.payer.key();
    let is_admin = signer == protocol_state.admin;
    let is_publisher =
        protocol_state.publisher != Pubkey::default() && signer == protocol_state.publisher;
    require!(is_admin || is_publisher, MiloError::Unauthorized);
    let streamer_key = derive_streamer_key(&channel);
    let seeds = [
        CHANNEL_STATE_SEED,
        protocol_state.mint.as_ref(),
        streamer_key.as_ref(),
    ];
    let (expected_pda, bump) = Pubkey::find_program_address(&seeds, ctx.program_id);
    require_keys_eq!(
        expected_pda,
        ctx.accounts.channel_state.to_account_info().key(),
        MiloError::InvalidChannelState
    );

    // Create account if needed
    let channel_state_info = ctx.accounts.channel_state.to_account_info();
    if channel_state_info.owner != ctx.program_id {
        msg!("Creating channel state account");
        let rent = Rent::get()?;
        let space = 8 + std::mem::size_of::<ChannelState>();
        let lamports = rent.minimum_balance(space);
        invoke_signed(
            &system_instruction::create_account(
                &ctx.accounts.payer.key(),
                &expected_pda,
                lamports,
                space as u64,
                ctx.program_id,
            ),
            &[
                ctx.accounts.payer.to_account_info(),
                channel_state_info.clone(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[&[
                CHANNEL_STATE_SEED,
                protocol_state.mint.as_ref(),
                streamer_key.as_ref(),
                &[bump],
            ]],
        )?;

        // Initialize zero_copy account with discriminator
        let mut data = channel_state_info.try_borrow_mut_data()?;
        use anchor_lang::Discriminator;
        data[0..8].copy_from_slice(&ChannelState::DISCRIMINATOR);
        msg!("Account created and initialized");
    }

    // Load via Anchor's zero_copy loader
    let mut channel_state = ctx.accounts.channel_state.load_mut()?;

    // Initialize fields if new
    if channel_state.version == 0 {
        channel_state.version = CHANNEL_STATE_VERSION;
        channel_state.bump = bump;
        channel_state.mint = protocol_state.mint;
        channel_state.streamer = streamer_key;
        channel_state.latest_epoch = 0;
        // slots already zeroed by account creation
    }

    // Validate
    require!(
        channel_state.version == CHANNEL_STATE_VERSION,
        MiloError::InvalidChannelState
    );
    require!(
        channel_state.mint == protocol_state.mint,
        MiloError::InvalidMint
    );
    require!(
        channel_state.streamer == streamer_key,
        MiloError::InvalidChannelState
    );

    // Update slot (ring buffer logic) with monotonic guard
    let slot_idx = ChannelState::slot_index(epoch);
    msg!("writing slot {}", slot_idx);
    let existing_epoch = channel_state.slots[slot_idx].epoch;
    require!(
        existing_epoch == 0 || epoch > existing_epoch,
        MiloError::EpochNotIncreasing
    );
    channel_state.slots[slot_idx].epoch = epoch;
    channel_state.slots[slot_idx].root = root;
    channel_state.slots[slot_idx].claim_count = 0;
    channel_state.slots[slot_idx].claimed_bitmap = [0u8; CHANNEL_BITMAP_BYTES];
    channel_state.latest_epoch = channel_state.latest_epoch.max(epoch);

    Ok(())
}

#[derive(Accounts)]
pub struct ClaimChannel<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ MiloError::ProtocolPaused,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// CHECK: PDA is validated via derivation in handler
    #[account(mut)]
    pub channel_state: AccountLoader<'info, ChannelState>,

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

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_channel_open(
    ctx: Context<ClaimChannel>,
    channel: String,
    epoch: u64,
    index: u32,
    amount: u64,
    id: String,
    proof: Vec<[u8; 32]>,
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;
    let streamer_key = derive_streamer_key(&channel);
    let seeds = [
        CHANNEL_STATE_SEED,
        protocol_state.mint.as_ref(),
        streamer_key.as_ref(),
    ];
    let (expected_pda, _) = Pubkey::find_program_address(&seeds, ctx.program_id);
    require_keys_eq!(
        expected_pda,
        ctx.accounts.channel_state.to_account_info().key(),
        MiloError::InvalidChannelState
    );

    // Load via Anchor's zero_copy loader
    let mut channel_state = ctx.accounts.channel_state.load_mut()?;

    require!(
        channel_state.version == CHANNEL_STATE_VERSION,
        MiloError::InvalidChannelState
    );
    require!(
        channel_state.mint == protocol_state.mint,
        MiloError::InvalidMint
    );
    require!(
        channel_state.streamer == streamer_key,
        MiloError::InvalidChannelState
    );

    ChannelSlot::validate_index(index as usize)?;
    let slot_idx = ChannelState::slot_index(epoch);
    require!(
        channel_state.slots[slot_idx].epoch == epoch,
        MiloError::SlotMismatch
    );

    let byte_i = (index / 8) as usize;
    let bit_mask = 1u8 << (index % 8);
    require!(byte_i < CHANNEL_BITMAP_BYTES, MiloError::InvalidIndex);
    require!(
        channel_state.slots[slot_idx].claimed_bitmap[byte_i] & bit_mask == 0,
        MiloError::AlreadyClaimed
    );

    let leaf = compute_leaf(&ctx.accounts.claimer.key(), index, amount, &id);
    require!(
        verify_proof(&proof, leaf, channel_state.slots[slot_idx].root),
        MiloError::InvalidProof
    );

    // Mark as claimed
    channel_state.slots[slot_idx].claimed_bitmap[byte_i] |= bit_mask;
    channel_state.slots[slot_idx].claim_count =
        channel_state.slots[slot_idx].claim_count.saturating_add(1);

    // Aggregator already scales weight → 100 CCM (weight × 100 × 10^9)
    // Use amount directly as transfer tokens (no double-scaling)
    let tokens = amount;

    let seeds: &[&[u8]] = &[
        PROTOCOL_SEED,
        protocol_state.mint.as_ref(),
        &[protocol_state.bump],
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
        tokens,
        ctx.accounts.mint.decimals,
    )?;

    Ok(())
}

// ============================================================================
// claim_channel_open_with_receipt: Optional cNFT minting (fee-only, rent-free)
// ============================================================================

#[derive(Accounts)]
pub struct ClaimChannelWithReceipt<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ MiloError::ProtocolPaused,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// CHECK: PDA is validated via derivation in handler
    #[account(mut)]
    pub channel_state: AccountLoader<'info, ChannelState>,

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

    // Bubblegum accounts (optional if mint_receipt=false)
    /// CHECK: Merkle tree account for cNFT minting
    #[account(mut)]
    pub merkle_tree: Option<AccountInfo<'info>>,

    /// CHECK: Tree authority PDA
    pub tree_authority: Option<AccountInfo<'info>>,

    /// CHECK: Leaf owner (claimer receives the cNFT)
    pub leaf_owner: Option<AccountInfo<'info>>,

    /// CHECK: Leaf delegate
    pub leaf_delegate: Option<AccountInfo<'info>>,

    /// CHECK: Bubblegum program
    pub bubblegum_program: Option<AccountInfo<'info>>,

    /// CHECK: Log wrapper
    pub log_wrapper: Option<AccountInfo<'info>>,

    /// CHECK: Compression program
    pub compression_program: Option<AccountInfo<'info>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_channel_open_with_receipt(
    ctx: Context<ClaimChannelWithReceipt>,
    channel: String,
    epoch: u64,
    index: u32,
    amount: u64,
    id: String,
    proof: Vec<[u8; 32]>,
    mint_receipt: bool,
) -> Result<()> {
    let protocol_state = &ctx.accounts.protocol_state;
    let streamer_key = derive_streamer_key(&channel);
    let seeds = [
        CHANNEL_STATE_SEED,
        protocol_state.mint.as_ref(),
        streamer_key.as_ref(),
    ];
    let (expected_pda, _) = Pubkey::find_program_address(&seeds, ctx.program_id);
    require_keys_eq!(
        expected_pda,
        ctx.accounts.channel_state.to_account_info().key(),
        MiloError::InvalidChannelState
    );

    // Load via Anchor's zero_copy loader
    let mut channel_state = ctx.accounts.channel_state.load_mut()?;

    require!(
        channel_state.version == CHANNEL_STATE_VERSION,
        MiloError::InvalidChannelState
    );
    require!(
        channel_state.mint == protocol_state.mint,
        MiloError::InvalidMint
    );
    require!(
        channel_state.streamer == streamer_key,
        MiloError::InvalidChannelState
    );

    ChannelSlot::validate_index(index as usize)?;
    let slot_idx = ChannelState::slot_index(epoch);
    require!(
        channel_state.slots[slot_idx].epoch == epoch,
        MiloError::SlotMismatch
    );

    let byte_i = (index / 8) as usize;
    let bit_mask = 1u8 << (index % 8);
    require!(byte_i < CHANNEL_BITMAP_BYTES, MiloError::InvalidIndex);
    require!(
        channel_state.slots[slot_idx].claimed_bitmap[byte_i] & bit_mask == 0,
        MiloError::AlreadyClaimed
    );

    let leaf = compute_leaf(&ctx.accounts.claimer.key(), index, amount, &id);
    require!(
        verify_proof(&proof, leaf, channel_state.slots[slot_idx].root),
        MiloError::InvalidProof
    );

    // Mark as claimed
    channel_state.slots[slot_idx].claimed_bitmap[byte_i] |= bit_mask;
    channel_state.slots[slot_idx].claim_count =
        channel_state.slots[slot_idx].claim_count.saturating_add(1);

    // Aggregator already scales weight → 100 CCM (weight × 100 × 10^9)
    // Use amount directly as transfer tokens (no double-scaling)
    let tokens = amount;

    // Transfer CCM tokens
    let seeds: &[&[u8]] = &[
        PROTOCOL_SEED,
        protocol_state.mint.as_ref(),
        &[protocol_state.bump],
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
        tokens,
        ctx.accounts.mint.decimals,
    )?;

    // Optional: Mint cNFT receipt
    if mint_receipt {
        require!(
            ctx.accounts.merkle_tree.is_some()
                && ctx.accounts.tree_authority.is_some()
                && ctx.accounts.bubblegum_program.is_some()
                && ctx.accounts.log_wrapper.is_some()
                && ctx.accounts.compression_program.is_some(),
            MiloError::MissingBubblegumAccounts
        );

        // Build metadata (brand-neutral)
        let metadata = mpl_bubblegum::types::MetadataArgs {
            name: format!("Attention Oracle: {} #{}", channel, epoch),
            symbol: "AO".to_string(),
            uri: format!("https://example.org/receipts/{}/{}", channel, epoch),
            seller_fee_basis_points: 0,
            primary_sale_happened: true,
            is_mutable: false,
            edition_nonce: None,
            token_standard: Some(mpl_bubblegum::types::TokenStandard::NonFungible),
            collection: None,
            uses: None,
            token_program_version: mpl_bubblegum::types::TokenProgramVersion::Original,
            creators: vec![],
        };

        // CPI to bubblegum mint_v1
        let mint_ix = mpl_bubblegum::instructions::MintV1 {
            tree_config: ctx.accounts.tree_authority.as_ref().unwrap().key(),
            leaf_owner: ctx
                .accounts
                .leaf_owner
                .as_ref()
                .unwrap_or(&ctx.accounts.claimer.to_account_info())
                .key(),
            leaf_delegate: ctx
                .accounts
                .leaf_delegate
                .as_ref()
                .unwrap_or(&ctx.accounts.claimer.to_account_info())
                .key(),
            merkle_tree: ctx.accounts.merkle_tree.as_ref().unwrap().key(),
            payer: ctx.accounts.claimer.key(),
            tree_creator_or_delegate: ctx.accounts.claimer.key(),
            log_wrapper: ctx.accounts.log_wrapper.as_ref().unwrap().key(),
            compression_program: ctx.accounts.compression_program.as_ref().unwrap().key(),
            system_program: ctx.accounts.system_program.key(),
        };

        let mint_ix_data = mpl_bubblegum::instructions::MintV1InstructionArgs { metadata };

        let ix = mint_ix.instruction(mint_ix_data);

        anchor_lang::solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.tree_authority.as_ref().unwrap().clone(),
                ctx.accounts
                    .leaf_owner
                    .as_ref()
                    .unwrap_or(&ctx.accounts.claimer.to_account_info())
                    .clone(),
                ctx.accounts
                    .leaf_delegate
                    .as_ref()
                    .unwrap_or(&ctx.accounts.claimer.to_account_info())
                    .clone(),
                ctx.accounts.merkle_tree.as_ref().unwrap().clone(),
                ctx.accounts.claimer.to_account_info(),
                ctx.accounts.log_wrapper.as_ref().unwrap().clone(),
                ctx.accounts.compression_program.as_ref().unwrap().clone(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        msg!(
            "cNFT receipt minted: channel={}, epoch={}, index={}",
            channel,
            epoch,
            index
        );
    }

    Ok(())
}
