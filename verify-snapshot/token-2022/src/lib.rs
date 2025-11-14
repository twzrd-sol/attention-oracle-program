use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

#[allow(unused_imports)]
pub(crate) use instructions::admin::__client_accounts_set_paused;
#[allow(unused_imports)]
pub(crate) use instructions::admin::__client_accounts_set_paused_open;
#[allow(unused_imports)]
pub(crate) use instructions::admin::__client_accounts_set_policy;
#[allow(unused_imports)]
pub(crate) use instructions::admin::__client_accounts_set_policy_open;
#[allow(unused_imports)]
pub(crate) use instructions::admin::__client_accounts_update_admin;
#[allow(unused_imports)]
pub(crate) use instructions::admin::__client_accounts_update_admin_open;
#[allow(unused_imports)]
pub(crate) use instructions::admin::__client_accounts_update_publisher;
#[allow(unused_imports)]
pub(crate) use instructions::admin::__client_accounts_update_publisher_open;
#[allow(unused_imports)]
pub(crate) use instructions::admin::__client_accounts_close_channel_state;
#[allow(unused_imports)]
pub(crate) use instructions::channel::__client_accounts_claim_channel;
#[allow(unused_imports)]
pub(crate) use instructions::channel::__client_accounts_set_channel_merkle_root;
#[allow(unused_imports)]
pub(crate) use instructions::claim::__client_accounts_claim;
#[allow(unused_imports)]
pub(crate) use instructions::claim::__client_accounts_claim_open;
#[allow(unused_imports)]
pub(crate) use instructions::cleanup::__client_accounts_close_epoch_state;
#[allow(unused_imports)]
pub(crate) use instructions::cleanup::__client_accounts_close_epoch_state_open;
#[allow(unused_imports)]
pub(crate) use instructions::governance::__client_accounts_update_fee_config;
#[allow(unused_imports)]
pub(crate) use instructions::governance::__client_accounts_update_fee_config_open;
#[allow(unused_imports)]
pub(crate) use instructions::initialize_mint::__client_accounts_initialize_mint;
#[allow(unused_imports)]
pub(crate) use instructions::initialize_mint::__client_accounts_initialize_mint_open;
#[allow(unused_imports)]
pub(crate) use instructions::merkle::__client_accounts_set_merkle_root;
#[allow(unused_imports)]
pub(crate) use instructions::merkle::__client_accounts_set_merkle_root_open;
#[allow(unused_imports)]
pub(crate) use instructions::merkle_ring::__client_accounts_claim_with_ring;
#[allow(unused_imports)]
pub(crate) use instructions::merkle_ring::__client_accounts_initialize_channel;
#[allow(unused_imports)]
pub(crate) use instructions::merkle_ring::__client_accounts_set_merkle_root_ring;

// Narrow imports for function signatures (no crate-wide re-exports)
use crate::instructions::admin::{
    CloseChannelState, SetPaused, SetPausedOpen, SetPolicy, SetPolicyOpen, UpdateAdmin,
    UpdateAdminOpen, UpdatePublisher, UpdatePublisherOpen,
};
use crate::instructions::channel::{ClaimChannel, SetChannelMerkleRoot};
use crate::instructions::claim::{Claim, ClaimOpen};
use crate::instructions::cleanup::{CloseEpochState, CloseEpochStateOpen};
use crate::instructions::cnft_verify::CnftReceiptProof;
use crate::instructions::governance::{UpdateFeeConfig, UpdateFeeConfigOpen};
use crate::instructions::initialize_mint::{InitializeMint, InitializeMintOpen};
use crate::instructions::merkle::{SetMerkleRoot, SetMerkleRootOpen};
use crate::instructions::merkle_ring::{ClaimWithRing, InitializeChannel, SetMerkleRootRing};
use crate::state::FeeSplit;

declare_id!("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");

#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;
#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "Attention Oracle - Verifiable Distribution Protocol (Token-2022)",
    project_url: "https://github.com/twzrd-sol/attention-oracle-program",
    contacts: "email:security@twzrd.xyz",
    policy: "https://github.com/twzrd-sol/attention-oracle-program/blob/main/SECURITY.md",
    preferred_languages: "en",
    source_code: "https://github.com/twzrd-sol/attention-oracle-program"
}

#[program]
pub mod token_2022 {
    use super::*;

    pub fn initialize_mint(
        ctx: Context<InitializeMint>,
        fee_basis_points: u16,
        max_fee: u64,
    ) -> Result<()> {
        instructions::initialize_mint::handler(ctx, fee_basis_points, max_fee)
    }

    pub fn initialize_mint_open(
        ctx: Context<InitializeMintOpen>,
        fee_basis_points: u16,
        max_fee: u64,
    ) -> Result<()> {
        instructions::initialize_mint::handler_open(ctx, fee_basis_points, max_fee)
    }

    pub fn set_merkle_root(
        ctx: Context<SetMerkleRoot>,
        root: [u8; 32],
        epoch: u64,
        claim_count: u32,
        streamer_key: Pubkey,
    ) -> Result<()> {
        instructions::merkle::set_merkle_root(ctx, root, epoch, claim_count, streamer_key)
    }

    pub fn claim(
        ctx: Context<Claim>,
        streamer_index: u8,
        index: u32,
        amount: u64,
        id: String,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::claim::claim(ctx, index, amount, id, proof)
    }

    pub fn claim_open(
        ctx: Context<ClaimOpen>,
        streamer_index: u8,
        index: u32,
        amount: u64,
        id: String,
        proof: Vec<[u8; 32]>,
        channel: Option<String>,
        twzrd_epoch: Option<u64>,
        receipt_proof: Option<CnftReceiptProof>,
    ) -> Result<()> {
        instructions::claim::claim_open(
            ctx,
            index,
            amount,
            id,
            proof,
            channel,
            twzrd_epoch,
            receipt_proof,
        )
    }

    pub fn update_fee_config(
        ctx: Context<UpdateFeeConfig>,
        new_basis_points: u16,
        fee_split: FeeSplit,
    ) -> Result<()> {
        instructions::governance::update_fee_config(ctx, new_basis_points, fee_split)
    }

    pub fn update_fee_config_open(
        ctx: Context<UpdateFeeConfigOpen>,
        new_basis_points: u16,
        fee_split: FeeSplit,
    ) -> Result<()> {
        instructions::governance::update_fee_config_open(ctx, new_basis_points, fee_split)
    }

    pub fn set_merkle_root_open(
        ctx: Context<SetMerkleRootOpen>,
        root: [u8; 32],
        epoch: u64,
        claim_count: u32,
        streamer_key: Pubkey,
    ) -> Result<()> {
        instructions::merkle::set_merkle_root_open(ctx, root, epoch, claim_count, streamer_key)
    }

    pub fn set_channel_merkle_root(
        ctx: Context<SetChannelMerkleRoot>,
        channel: String,
        epoch: u64,
        root: [u8; 32],
    ) -> Result<()> {
        instructions::channel::set_channel_merkle_root(ctx, channel, epoch, root)
    }

    pub fn update_publisher(ctx: Context<UpdatePublisher>, new_publisher: Pubkey) -> Result<()> {
        instructions::admin::update_publisher(ctx, new_publisher)
    }

    pub fn update_publisher_open(
        ctx: Context<UpdatePublisherOpen>,
        new_publisher: Pubkey,
    ) -> Result<()> {
        instructions::admin::update_publisher_open(ctx, new_publisher)
    }

    pub fn set_policy(ctx: Context<SetPolicy>, require_receipt: bool) -> Result<()> {
        instructions::admin::set_policy(ctx, require_receipt)
    }

    pub fn set_policy_open(ctx: Context<SetPolicyOpen>, require_receipt: bool) -> Result<()> {
        instructions::admin::set_policy_open(ctx, require_receipt)
    }

    pub fn set_paused(ctx: Context<SetPaused>, paused: bool) -> Result<()> {
        instructions::admin::set_paused(ctx, paused)
    }

    pub fn set_paused_open(ctx: Context<SetPausedOpen>, paused: bool) -> Result<()> {
        instructions::admin::set_paused_open(ctx, paused)
    }

    pub fn update_admin_open(ctx: Context<UpdateAdminOpen>, new_admin: Pubkey) -> Result<()> {
        instructions::admin::update_admin_open(ctx, new_admin)
    }

    pub fn update_admin(ctx: Context<UpdateAdmin>, new_admin: Pubkey) -> Result<()> {
        instructions::admin::update_admin(ctx, new_admin)
    }

    /// Close a ChannelState account and recover rent (admin-only)
    pub fn close_channel_state(ctx: Context<CloseChannelState>) -> Result<()> {
        instructions::admin::close_channel_state(ctx)
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
        instructions::channel::claim_channel_open(ctx, channel, epoch, index, amount, id, proof)
    }

    pub fn close_epoch_state(
        ctx: Context<CloseEpochState>,
        epoch: u64,
        streamer_key: Pubkey,
    ) -> Result<()> {
        instructions::cleanup::close_epoch_state(ctx, epoch, streamer_key)
    }

    pub fn close_epoch_state_open(
        ctx: Context<CloseEpochStateOpen>,
        epoch: u64,
        streamer_key: Pubkey,
    ) -> Result<()> {
        instructions::cleanup::close_epoch_state_open(ctx, epoch, streamer_key)
    }



    pub fn initialize_channel(ctx: Context<InitializeChannel>, streamer_key: Pubkey) -> Result<()> {
        instructions::merkle_ring::initialize_channel(ctx, streamer_key)
    }

    pub fn set_merkle_root_ring(
        ctx: Context<SetMerkleRootRing>,
        root: [u8; 32],
        epoch: u64,
        claim_count: u16,
        streamer_key: Pubkey,
    ) -> Result<()> {
        instructions::merkle_ring::set_merkle_root_ring(ctx, root, epoch, claim_count, streamer_key)
    }

    pub fn claim_with_ring(
        ctx: Context<ClaimWithRing>,
        epoch: u64,
        index: u32,
        amount: u64,
        proof: Vec<[u8; 32]>,
        id: String,
        streamer_key: Pubkey,
    ) -> Result<()> {
        instructions::merkle_ring::claim_with_ring(
            ctx,
            epoch,
            index,
            amount,
            proof,
            id,
            streamer_key,
        )
    }
}
