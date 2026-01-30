//! Set vLOFI token metadata via Metaplex Token Metadata program.
//!
//! Creates or updates on-chain metadata (name, symbol, URI) for vLOFI mints
//! so wallets, explorers, and DEXes can display them properly.
//!
//! The vault PDA is the mint authority, so it must sign the create CPI.
//! Admin is set as update_authority for future metadata changes.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed},
};

use crate::constants::{METADATA_PROGRAM_ID, VAULT_SEED};
use crate::errors::VaultError;
use crate::state::ChannelVault;

/// Metaplex metadata PDA seed
const METADATA_SEED: &[u8] = b"metadata";

#[derive(Accounts)]
pub struct SetVlofiMetadata<'info> {
    #[account(
        mut,
        constraint = admin.key() == vault.admin @ VaultError::Unauthorized,
    )]
    pub admin: Signer<'info>,

    #[account(
        seeds = [VAULT_SEED, vault.channel_config.as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, ChannelVault>,

    /// CHECK: Validated against vault.vlofi_mint
    #[account(
        constraint = vlofi_mint.key() == vault.vlofi_mint @ VaultError::InvalidMint,
    )]
    pub vlofi_mint: UncheckedAccount<'info>,

    /// CHECK: Validated by PDA derivation in handler
    #[account(mut)]
    pub metadata: UncheckedAccount<'info>,

    /// CHECK: Validated against known program ID constant
    #[account(
        constraint = metadata_program.key() == METADATA_PROGRAM_ID @ VaultError::InvalidMetadataProgram,
    )]
    pub metadata_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

/// Borsh-serialize a DataV2 struct for Metaplex CPI.
fn serialize_data_v2(name: &str, symbol: &str, uri: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    // name: String
    buf.extend_from_slice(&(name.len() as u32).to_le_bytes());
    buf.extend_from_slice(name.as_bytes());
    // symbol: String
    buf.extend_from_slice(&(symbol.len() as u32).to_le_bytes());
    buf.extend_from_slice(symbol.as_bytes());
    // uri: String
    buf.extend_from_slice(&(uri.len() as u32).to_le_bytes());
    buf.extend_from_slice(uri.as_bytes());
    // seller_fee_basis_points: u16 = 0
    buf.extend_from_slice(&0u16.to_le_bytes());
    // creators: Option<Vec<Creator>> = None
    buf.push(0);
    // collection: Option<Collection> = None
    buf.push(0);
    // uses: Option<Uses> = None
    buf.push(0);
    buf
}

pub fn handler(
    ctx: Context<SetVlofiMetadata>,
    name: String,
    symbol: String,
    uri: String,
) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let metadata_program_key = ctx.accounts.metadata_program.key();

    // Verify metadata PDA derivation
    let (expected_metadata, _) = Pubkey::find_program_address(
        &[
            METADATA_SEED,
            metadata_program_key.as_ref(),
            ctx.accounts.vlofi_mint.key().as_ref(),
        ],
        &metadata_program_key,
    );
    require!(
        ctx.accounts.metadata.key() == expected_metadata,
        VaultError::InvalidMetadataAccount
    );

    let is_create = ctx.accounts.metadata.data_is_empty();

    if is_create {
        // Vault PDA signs as mint_authority for CreateMetadataAccountV3
        let channel_config_key = vault.channel_config;
        let bump = vault.bump;
        let signer_seeds: &[&[&[u8]]] = &[&[
            VAULT_SEED,
            channel_config_key.as_ref(),
            &[bump],
        ]];

        // Borsh: enum variant 33 = CreateMetadataAccountV3
        let mut data = vec![33u8];
        data.extend(serialize_data_v2(&name, &symbol, &uri));
        data.push(1); // is_mutable = true
        data.push(0); // collection_details = None

        let ix = Instruction {
            program_id: metadata_program_key,
            accounts: vec![
                AccountMeta::new(ctx.accounts.metadata.key(), false),
                AccountMeta::new_readonly(ctx.accounts.vlofi_mint.key(), false),
                AccountMeta::new_readonly(ctx.accounts.vault.key(), true),  // mint_authority (PDA signer)
                AccountMeta::new(ctx.accounts.admin.key(), true),           // payer
                AccountMeta::new_readonly(ctx.accounts.admin.key(), false), // update_authority
                AccountMeta::new_readonly(ctx.accounts.system_program.key(), false),
            ],
            data,
        };

        invoke_signed(
            &ix,
            &[
                ctx.accounts.metadata.to_account_info(),
                ctx.accounts.vlofi_mint.to_account_info(),
                ctx.accounts.vault.to_account_info(),
                ctx.accounts.admin.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            signer_seeds,
        )?;

        msg!("vLOFI metadata created: {} ({})", name, symbol);
    } else {
        // UpdateMetadataAccountV2 - admin signs as update_authority
        // Borsh: enum variant 15 = UpdateMetadataAccountV2
        let mut data = vec![15u8];
        data.push(1); // data: Option<DataV2> = Some
        data.extend(serialize_data_v2(&name, &symbol, &uri));
        data.push(0); // update_authority: Option<Pubkey> = None (keep current)
        data.push(0); // primary_sale_happened: Option<bool> = None
        data.push(0); // is_mutable: Option<bool> = None

        let ix = Instruction {
            program_id: metadata_program_key,
            accounts: vec![
                AccountMeta::new(ctx.accounts.metadata.key(), false),
                AccountMeta::new_readonly(ctx.accounts.admin.key(), true), // update_authority
            ],
            data,
        };

        invoke(
            &ix,
            &[
                ctx.accounts.metadata.to_account_info(),
                ctx.accounts.admin.to_account_info(),
            ],
        )?;

        msg!("vLOFI metadata updated: {} ({})", name, symbol);
    }

    Ok(())
}
