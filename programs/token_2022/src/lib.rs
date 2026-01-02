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

    /// Invisible staking: Claims rewards directly into the stake vault.
    pub fn claim_and_stake_sponsored<'info>(
        ctx: Context<'_, '_, '_, 'info, ClaimAndStakeSponsored<'info>>,
        channel: String,
        root_seq: u64,
        cumulative_total: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::cumulative::claim_and_stake_sponsored(ctx, channel, root_seq, cumulative_total, proof)
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

    // -------------------------------------------------------------------------
    // Governance
    // -------------------------------------------------------------------------

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

    pub fn update_tier_multipliers(
        ctx: Context<UpdateTierMultipliers>,
        new_multipliers: [u32; 6],
    ) -> Result<()> {
        instructions::governance::update_tier_multipliers(ctx, new_multipliers)
    }

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

    // -------------------------------------------------------------------------
    // Identity
    // -------------------------------------------------------------------------

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
            ctx,
            user_hash,
            new_tier,
            new_score,
            epoch_count,
            weighted_presence,
            badges,
            leaf_hash,
        )
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

    // -------------------------------------------------------------------------
    // Staking
    // -------------------------------------------------------------------------

    pub fn initialize_stake_pool(
        ctx: Context<InitializeStakePool>,
        reward_rate: u64,
    ) -> Result<()> {
        instructions::staking::initialize_stake_pool(ctx, reward_rate)
    }

    pub fn stake<'info>(
        ctx: Context<'_, '_, '_, 'info, Stake<'info>>,
        amount: u64,
        lock_slots: u64,
    ) -> Result<()> {
        instructions::staking::stake(ctx, amount, lock_slots)
    }

    pub fn unstake<'info>(
        ctx: Context<'_, '_, '_, 'info, Unstake<'info>>,
        amount: u64,
    ) -> Result<()> {
        instructions::staking::unstake(ctx, amount)
    }

    pub fn delegate_stake(ctx: Context<DelegateStake>, subject_id: Option<[u8; 32]>) -> Result<()> {
        instructions::staking::delegate_stake(ctx, subject_id)
    }

    pub fn claim_stake_rewards<'info>(
        ctx: Context<'_, '_, '_, 'info, ClaimStakeRewards<'info>>,
    ) -> Result<()> {
        instructions::staking::claim_stake_rewards(ctx)
    }

    // -------------------------------------------------------------------------
    // Migration (Feature-gated)
    // -------------------------------------------------------------------------

    #[cfg(feature = "migration")]
    pub fn migrate(ctx: Context<Migrate>, amount: u64) -> Result<()> {
        instructions::migrate::migrate(ctx, amount)
    }
}
