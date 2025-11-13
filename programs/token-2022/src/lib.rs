use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

#[allow(unused_imports)]
pub(crate) use instructions::admin::__client_accounts_close_channel_state;
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
use crate::instructions::channel::{ClaimChannel, ClaimChannelWithReceipt, SetChannelMerkleRoot};
use crate::instructions::claim::{Claim, ClaimOpen};
use crate::instructions::cleanup::{
    CloseEpochState, CloseEpochStateOpen, ForceCloseEpochStateLegacy,
    ForceCloseEpochStateOpen,
};
use crate::instructions::cnft_verify::CnftReceiptProof;
use crate::instructions::governance::{UpdateFeeConfig, UpdateFeeConfigOpen};
use crate::instructions::hooks::TransferHook;
use crate::instructions::initialize_mint::{InitializeMint, InitializeMintOpen};
use crate::instructions::liquidity::TriggerLiquidityDrip;
use crate::instructions::merkle::{SetMerkleRoot, SetMerkleRootOpen};
use crate::instructions::merkle_ring::{ClaimWithRing, InitializeChannel, SetMerkleRootRing};
use crate::instructions::passport::{
    MintPassportOpen, ReissuePassportOpen, RevokePassportOpen, UpgradePassportOpen,
    UpgradePassportProved,
};
use crate::instructions::points::{ClaimPointsOpen, RequirePoints as RequirePointsGe};
use crate::state::FeeSplit;

declare_id!("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");

#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;
#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "Verifiable Distribution Protocol (Token-2022)",
    project_url: "https://github.com/twzrd-sol/attention-oracle-program",
    contacts: "email:ccm@twzrd.xyz",
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

    // Transfer hook for automatic fee collection
    pub fn transfer_hook(ctx: Context<TransferHook>, amount: u64) -> Result<()> {
        instructions::hooks::transfer_hook(ctx, amount)
    }

    // Points system instructions
    pub fn claim_points_open(
        ctx: Context<ClaimPointsOpen>,
        index: u32,
        amount: u64,
        id: String,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::points::claim_points_open(ctx, index, amount, id, proof)
    }

    pub fn require_points_ge(ctx: Context<RequirePointsGe>, min: u64) -> Result<()> {
        instructions::points::require_points_ge(ctx, min)
    }

    // Advanced channel claims with receipt
    pub fn claim_channel_open_with_receipt(
        ctx: Context<ClaimChannelWithReceipt>,
        epoch: u64,
        index: u32,
        amount: u64,
        proof: Vec<[u8; 32]>,
        id: String,
        streamer_key: Pubkey,
        receipt_proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::channel::claim_channel_with_receipt(
            ctx,
            epoch,
            index,
            amount,
            proof,
            id,
            streamer_key,
            receipt_proof,
        )
    }

    // Advanced cleanup instructions
    pub fn force_close_epoch_state_legacy(
        ctx: Context<ForceCloseEpochStateLegacy>,
        epoch: u64,
        streamer_key: Pubkey,
    ) -> Result<()> {
        instructions::cleanup::force_close_epoch_state_legacy(ctx, epoch, streamer_key)
    }

    pub fn force_close_epoch_state_open(
        ctx: Context<ForceCloseEpochStateOpen>,
        epoch: u64,
        streamer_key: Pubkey,
        mint: Pubkey,
    ) -> Result<()> {
        instructions::cleanup::force_close_epoch_state_open(ctx, epoch, streamer_key, mint)
    }


    // Liquidity management
    pub fn trigger_liquidity_drip(ctx: Context<TriggerLiquidityDrip>) -> Result<()> {
        instructions::liquidity::trigger_liquidity_drip(ctx)
    }

    // Passport/Identity system instructions
    pub fn mint_passport_open(
        ctx: Context<MintPassportOpen>,
        user_hash: [u8; 32],
        owner: Pubkey,
        tier: u8,
        score: u64,
    ) -> Result<()> {
        instructions::passport::mint_passport_open(ctx, user_hash, owner, tier, score)
    }

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
        instructions::passport::upgrade_passport_open(
            ctx, user_hash, new_tier, new_score, epoch_count, weighted_presence, badges, leaf_hash
        )
    }

    pub fn upgrade_passport_proved(
        ctx: Context<UpgradePassportProved>,
        proof: Vec<[u8; 32]>,
        index: u32,
        score: u32,
        tier: u8,
    ) -> Result<()> {
        instructions::passport::upgrade_passport_proved(ctx, proof, index, score, tier)
    }

    pub fn reissue_passport_open(
        ctx: Context<ReissuePassportOpen>,
        user_hash: [u8; 32],
        new_owner: Pubkey,
    ) -> Result<()> {
        instructions::passport::reissue_passport_open(ctx, user_hash, new_owner)
    }

    pub fn revoke_passport_open(
        ctx: Context<RevokePassportOpen>,
        user_hash: [u8; 32],
    ) -> Result<()> {
        instructions::passport::revoke_passport_open(ctx, user_hash)
    }
}
