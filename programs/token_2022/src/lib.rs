#![allow(ambiguous_glob_reexports)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::too_many_arguments, clippy::missing_errors_doc)]
#![allow(clippy::doc_markdown, clippy::must_use_candidate, clippy::needless_pass_by_value)]
#![allow(clippy::manual_div_ceil)]
#![allow(clippy::items_after_statements, clippy::needless_for_each, clippy::needless_borrow)]
#![allow(clippy::missing_panics_doc, clippy::too_many_lines, clippy::uninlined_format_args)]
#![allow(clippy::or_fun_call, clippy::explicit_iter_loop, clippy::used_underscore_binding)]
#![allow(clippy::needless_borrows_for_generic_args, clippy::cast_possible_truncation, clippy::cast_lossless)]
#![allow(clippy::no_effect_underscore_binding, clippy::pub_underscore_fields)]
#![allow(clippy::too_long_first_doc_paragraph, clippy::unnecessary_cast, clippy::len_zero)]
#![allow(clippy::wildcard_imports, clippy::missing_const_for_fn, clippy::use_self)]

//! # Attention Oracle Protocol
//!
//! A verifiable, high-throughput distribution primitive for the Solana blockchain.

use anchor_lang::prelude::*;

#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod merkle_proof;
pub mod state;
pub mod token_transfer;

pub use constants::*;
pub use errors::*;
pub use events::*;
pub use instructions::*;
pub use merkle_proof::*;
pub use state::*;
pub use token_transfer::*;

declare_id!("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "Attention Oracle Protocol",
    project_url: "https://github.com/twzrd-sol/attention-oracle-program",
    contacts: "email:security@twzrd.xyz",
    policy: "https://github.com/twzrd-sol/attention-oracle-program/blob/main/SECURITY.md",
    preferred_languages: "en",
    source_code: "https://github.com/twzrd-sol/attention-oracle-program"
}

#[program]
pub mod token_2022 {
    use super::*;

    // -------------------------------------------------------------------------
    // Protocol Initialization
    // -------------------------------------------------------------------------

    pub fn initialize_mint(
        ctx: Context<InitializeMint>,
        fee_basis_points: u16,
        max_fee: u64,
    ) -> Result<()> {
        instructions::initialize_mint::handler(ctx, fee_basis_points, max_fee)
    }

    // -------------------------------------------------------------------------
    // Cumulative Roots (V2) - ACTIVE CLAIM SYSTEM
    // -------------------------------------------------------------------------

    pub fn initialize_channel_cumulative(
        ctx: Context<InitializeChannelCumulative>,
        channel: String,
        cutover_epoch: u64,
        creator_wallet: Pubkey,
        creator_fee_bps: u16,
    ) -> Result<()> {
        instructions::cumulative::initialize_channel_cumulative(
            ctx,
            channel,
            cutover_epoch,
            creator_wallet,
            creator_fee_bps,
        )
    }

    pub fn publish_cumulative_root(
        ctx: Context<PublishCumulativeRoot>,
        channel: String,
        root_seq: u64,
        root: [u8; 32],
        dataset_hash: [u8; 32],
    ) -> Result<()> {
        instructions::cumulative::publish_cumulative_root(ctx, channel, root_seq, root, dataset_hash)
    }

    pub fn claim_cumulative<'info>(
        ctx: Context<'_, '_, '_, 'info, ClaimCumulative<'info>>,
        channel: String,
        root_seq: u64,
        cumulative_total: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::cumulative::claim_cumulative(ctx, channel, root_seq, cumulative_total, proof)
    }

    pub fn claim_cumulative_sponsored<'info>(
        ctx: Context<'_, '_, '_, 'info, ClaimCumulativeSponsored<'info>>,
        channel: String,
        root_seq: u64,
        cumulative_total: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::cumulative::claim_cumulative_sponsored(ctx, channel, root_seq, cumulative_total, proof)
    }

    /// Migrate existing ChannelConfigV2 accounts to add creator_wallet fields.
    pub fn migrate_channel_config_v2(
        ctx: Context<MigrateChannelConfigV2>,
        channel: String,
        creator_wallet: Pubkey,
        creator_fee_bps: u16,
    ) -> Result<()> {
        instructions::cumulative::migrate_channel_config_v2(ctx, channel, creator_wallet, creator_fee_bps)
    }

    /// Update creator fee on already-migrated ChannelConfigV2.
    pub fn update_channel_creator_fee(
        ctx: Context<UpdateChannelCreatorFee>,
        channel: String,
        new_creator_fee_bps: u16,
    ) -> Result<()> {
        instructions::cumulative::update_channel_creator_fee(ctx, channel, new_creator_fee_bps)
    }

    /// Admin-only: Recover from skipped root sequence to unbrick a channel.
    pub fn admin_recover_root_seq(
        ctx: Context<AdminRecoverRootSeq>,
        channel: String,
        new_seq: u64,
    ) -> Result<()> {
        instructions::cumulative::admin_recover_root_seq(ctx, channel, new_seq)
    }

    /// Close a channel config to reclaim rent (Admin only).
    pub fn close_channel(ctx: Context<CloseChannel>, channel: String) -> Result<()> {
        instructions::cumulative::close_channel(ctx, channel)
    }

    // -------------------------------------------------------------------------
    // Token-2022 Transfer Fee Harvesting
    // -------------------------------------------------------------------------

    pub fn harvest_fees<'info>(
        ctx: Context<'_, '_, 'info, 'info, HarvestFees<'info>>,
    ) -> Result<()> {
        instructions::governance::harvest_and_distribute_fees(ctx)
    }

    // -------------------------------------------------------------------------
    // Access Control
    // -------------------------------------------------------------------------

    pub fn update_publisher(ctx: Context<UpdatePublisher>, new_publisher: Pubkey) -> Result<()> {
        instructions::admin::update_publisher(ctx, new_publisher)
    }

    pub fn update_publisher_open(
        ctx: Context<UpdatePublisherOpen>,
        new_publisher: Pubkey,
    ) -> Result<()> {
        instructions::admin::update_publisher_open(ctx, new_publisher)
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

    // -------------------------------------------------------------------------
    // Channel Staking
    // -------------------------------------------------------------------------

    /// Initialize a stake pool for a channel.
    pub fn initialize_channel_stake_pool(
        ctx: Context<InitializeChannelStakePool>,
        channel: String,
    ) -> Result<()> {
        instructions::channel_staking::initialize_channel_stake_pool(ctx, channel)
    }

    /// Stake tokens on a channel.
    pub fn stake_channel(
        ctx: Context<StakeChannel>,
        channel: String,
        amount: u64,
        lock_slots: u64,
    ) -> Result<()> {
        instructions::channel_staking::stake_channel(ctx, channel, amount, lock_slots)
    }

    /// Unstake tokens from a channel (after lock expires).
    pub fn unstake_channel(
        ctx: Context<UnstakeChannel>,
        channel: String,
        amount: u64,
    ) -> Result<()> {
        instructions::channel_staking::unstake_channel(ctx, channel, amount)
    }

    /// Extend lock period for additional boost.
    pub fn extend_lock(
        ctx: Context<ExtendLock>,
        channel: String,
        additional_slots: u64,
    ) -> Result<()> {
        instructions::channel_staking::extend_lock(ctx, channel, additional_slots)
    }

    // -------------------------------------------------------------------------
    // NFT Stake Positions
    // -------------------------------------------------------------------------

    /// Mint a transferable NFT representing a stake position.
    pub fn mint_stake_position_nft(
        ctx: Context<MintStakePositionNft>,
        channel: String,
    ) -> Result<()> {
        instructions::stake_nft::mint_stake_position_nft(ctx, channel)
    }

    /// Unstake tokens using NFT ownership (NFT holder can unstake).
    pub fn unstake_with_nft(
        ctx: Context<UnstakeWithNft>,
        channel: String,
        amount: u64,
    ) -> Result<()> {
        instructions::stake_nft::unstake_with_nft(ctx, channel, amount)
    }

    // admin_withdraw removed - treasury locked to claims only
    // See: https://solscan.io/tx/L53wKdRPTYKCwR1DJJQjFr34SYsCzjqcyNgXP7BbZAV7Yasz7bDwqP2no6ozm7tLVMawUcADGhZPXRNe4wQajeh
}
