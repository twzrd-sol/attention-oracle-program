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

    // -------------------------------------------------------------------------
    // Cumulative Roots (V3) - With Stake Snapshot Binding (Anti-Gaming)
    // -------------------------------------------------------------------------

    /// V3 cumulative claim with stake snapshot verification.
    /// Prevents "boost gaming" where users unstake after snapshot.
    pub fn claim_cumulative_v3<'info>(
        ctx: Context<'_, '_, '_, 'info, ClaimCumulativeV3<'info>>,
        channel: String,
        root_seq: u64,
        cumulative_total: u64,
        stake_snapshot: u64,
        snapshot_slot: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::cumulative::claim_cumulative_v3(ctx, channel, root_seq, cumulative_total, stake_snapshot, snapshot_slot, proof)
    }

    /// V3 sponsored claim with stake snapshot verification.
    pub fn claim_cumulative_sponsored_v3<'info>(
        ctx: Context<'_, '_, '_, 'info, ClaimCumulativeSponsoredV3<'info>>,
        channel: String,
        root_seq: u64,
        cumulative_total: u64,
        stake_snapshot: u64,
        snapshot_slot: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::cumulative::claim_cumulative_sponsored_v3(ctx, channel, root_seq, cumulative_total, stake_snapshot, snapshot_slot, proof)
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

    /// Admin-only: Set the cutover epoch for V2 sunset enforcement.
    /// Once reached, V2 claims are disabled and users must use V3.
    pub fn update_channel_cutover_epoch(
        ctx: Context<UpdateChannelCutoverEpoch>,
        channel: String,
        new_cutover_epoch: u64,
    ) -> Result<()> {
        instructions::cumulative::update_channel_cutover_epoch(ctx, channel, new_cutover_epoch)
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
    // Global Root (V4) — Single Root, Per-User Totals
    // -------------------------------------------------------------------------

    pub fn initialize_global_root(ctx: Context<InitializeGlobalRoot>) -> Result<()> {
        instructions::global::initialize_global_root(ctx)
    }

    pub fn publish_global_root(
        ctx: Context<PublishGlobalRoot>,
        root_seq: u64,
        root: [u8; 32],
        dataset_hash: [u8; 32],
    ) -> Result<()> {
        instructions::global::publish_global_root(ctx, root_seq, root, dataset_hash)
    }

    pub fn claim_global<'info>(
        ctx: Context<'_, '_, '_, 'info, ClaimGlobal<'info>>,
        root_seq: u64,
        cumulative_total: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::global::claim_global(ctx, root_seq, cumulative_total, proof)
    }

    pub fn claim_global_sponsored<'info>(
        ctx: Context<'_, '_, '_, 'info, ClaimGlobalSponsored<'info>>,
        root_seq: u64,
        cumulative_total: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::global::claim_global_sponsored(ctx, root_seq, cumulative_total, proof)
    }

    // -------------------------------------------------------------------------
    // Token-2022 Transfer Fee Harvesting
    // -------------------------------------------------------------------------

    pub fn harvest_fees<'info>(
        ctx: Context<'_, '_, 'info, 'info, HarvestFees<'info>>,
    ) -> Result<()> {
        instructions::governance::harvest_and_distribute_fees(ctx)
    }

    pub fn withdraw_fees_from_mint(ctx: Context<WithdrawFeesFromMint>) -> Result<()> {
        instructions::governance::withdraw_fees_from_mint(ctx)
    }

    pub fn route_treasury(
        ctx: Context<RouteTreasury>,
        amount: u64,
        min_reserve: u64,
    ) -> Result<()> {
        instructions::governance::route_treasury(ctx, amount, min_reserve)
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

    /// Set the treasury wallet (fee destination owner).
    pub fn set_treasury(ctx: Context<SetTreasury>, new_treasury: Pubkey) -> Result<()> {
        instructions::admin::set_treasury(ctx, new_treasury)
    }

    // -------------------------------------------------------------------------
    // Channel Staking (Token-2022 Soulbound Receipts)
    // -------------------------------------------------------------------------

    /// Initialize a stake pool for a channel.
    pub fn initialize_stake_pool(ctx: Context<InitializeStakePool>) -> Result<()> {
        instructions::staking::initialize_stake_pool(ctx)
    }

    /// Stake tokens on a channel and receive a soulbound receipt NFT.
    pub fn stake_channel(
        ctx: Context<StakeChannel>,
        amount: u64,
        lock_duration: u64,
    ) -> Result<()> {
        instructions::staking::stake_channel(ctx, amount, lock_duration)
    }

    /// Unstake tokens by burning the receipt NFT.
    pub fn unstake_channel(ctx: Context<UnstakeChannel>) -> Result<()> {
        instructions::staking::unstake_channel(ctx)
    }

    /// Claim accumulated staking rewards.
    pub fn claim_channel_rewards(ctx: Context<ClaimChannelRewards>) -> Result<()> {
        instructions::staking::claim_channel_rewards(ctx)
    }

    /// Set the reward rate for a channel stake pool (admin only).
    pub fn set_reward_rate(ctx: Context<SetRewardRate>, new_rate: u64) -> Result<()> {
        instructions::staking::set_reward_rate(ctx, new_rate)
    }

    /// Migrate existing stake pool accounts to add reward fields (admin only).
    pub fn migrate_stake_pool(ctx: Context<MigrateStakePool>) -> Result<()> {
        instructions::staking::migrate_stake_pool(ctx)
    }

    /// Migrate existing user stake accounts to add reward fields (admin only).
    pub fn migrate_user_stake(ctx: Context<MigrateUserStake>) -> Result<()> {
        instructions::staking::migrate_user_stake(ctx)
    }

    /// Emergency unstake before lock expiry with 20% penalty (burned).
    pub fn emergency_unstake_channel(ctx: Context<EmergencyUnstakeChannel>) -> Result<()> {
        instructions::staking::emergency_unstake_channel(ctx)
    }

    /// Admin: Shut down a pool for emergency penalty-free exits.
    /// Stops reward accrual and waives all lock periods.
    pub fn admin_shutdown_pool(ctx: Context<AdminShutdownPool>, reason: String) -> Result<()> {
        instructions::staking::admin_shutdown_pool(ctx, reason)
    }

    /// Admin: Recover a shutdown pool without state loss.
    /// Unsets the shutdown flag, preserving all staking data and rewards.
    pub fn admin_recover_pool(ctx: Context<AdminRecoverPool>) -> Result<()> {
        instructions::staking::admin_recover_pool(ctx)
    }

    /// Admin: Close a fully-emptied shutdown pool, recovering remaining
    /// reward tokens and rent. Requires: is_shutdown, 0 stakers, 0 staked.
    pub fn close_stake_pool(ctx: Context<CloseStakePool>) -> Result<()> {
        instructions::staking::close_stake_pool(ctx)
    }
}
