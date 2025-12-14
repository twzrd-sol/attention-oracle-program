#![allow(ambiguous_glob_reexports)]
// Security: Enforce strict lints for production-grade DeFi code
#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
// Allow necessary Anchor boilerplate patterns
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
//!
//! ## Architecture
//!
//! 1. **Ring-Buffer State**: Utilizes a fixed-size circular buffer to store historical
//!    Merkle roots. This ensures O(1) storage costs regardless of protocol longevity.
//! 2. **Treasury-Backed Distribution**: Tokens are transferred from a pre-funded treasury
//!    rather than minted, ensuring strict supply caps and enabling circular economic flows.
//! 3. **Token-2022 Integration**: Native support for Transfer Hooks and Extended Metadata,
//!    enabling programmable yield and dynamic fee enforcement at the token standard level.
//!
//! ## Security Model
//!
//! - **Cryptographic Verification**: All claims are validated against on-chain Merkle roots.
//! - **Bitmap Replay Protection**: Bit-level tracking prevents double-spending of claims.
//! - **Role-Based Access**: Strict separation between Admin (Governance) and Publisher (Oracle).

use anchor_lang::prelude::*;

#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod token_transfer;

pub use constants::*;
pub use errors::*;
pub use events::*;
pub use instructions::*;
pub use state::*;
pub use token_transfer::*;

// Program ID
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

    /// Initialize the protocol state and bind it to a Token-2022 mint.
    /// This establishes the Treasury and Fee Configuration PDAs.
    pub fn initialize_mint(
        ctx: Context<InitializeMint>,
        fee_basis_points: u16,
        max_fee: u64,
    ) -> Result<()> {
        instructions::initialize_mint::handler(ctx, fee_basis_points, max_fee)
    }

    // -------------------------------------------------------------------------
    // Oracle & Distribution (Ring Buffer)
    // -------------------------------------------------------------------------

    /// Publish a new Merkle root for a specific channel epoch.
    /// Updates the ring buffer, overwriting the oldest slot if full.
    pub fn set_channel_merkle_root(
        ctx: Context<SetChannelMerkleRoot>,
        channel: String,
        epoch: u64,
        root: [u8; 32],
    ) -> Result<()> {
        instructions::channel::set_channel_merkle_root(ctx, channel, epoch, root)
    }

    /// Execute a claim against a valid Merkle root in the ring buffer.
    /// Verifies the proof and transfers tokens from the Treasury.
    pub fn claim_channel_open<'info>(
        ctx: Context<'_, '_, '_, 'info, ClaimChannel<'info>>,
        channel: String,
        epoch: u64,
        index: u32,
        amount: u64,
        id: String,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::channel::claim_channel_open(ctx, channel, epoch, index, amount, id, proof)
    }

    /// Push-distribute CCM to multiple recipients in a single transaction.
    /// Publisher-only operation for batch airdrops/rewards.
    /// Recipient ATAs must pre-exist (passed as remaining_accounts).
    pub fn push_distribute<'info>(
        ctx: Context<'_, '_, 'info, 'info, PushDistribute<'info>>,
        recipients: Vec<Pubkey>,
        amounts: Vec<u64>,
        epoch: u64,
        channel: String,
        batch_idx: u32,
    ) -> Result<()> {
        instructions::push_distribute::push_distribute(ctx, recipients, amounts, epoch, channel, batch_idx)
    }

    /// Close a channel state account and reclaim rent to the admin.
    /// Critical for cleaning up disabled streams (e.g. Twitch migration).
    pub fn close_channel(ctx: Context<CloseChannel>, channel: String) -> Result<()> {
        instructions::channel::close_channel(ctx, channel)
    }

    /// Migrate a channel state account from old size (728 bytes) to new size (5688 bytes).
    /// Required after CHANNEL_MAX_CLAIMS upgrade from 1024 to 4096.
    /// Publisher or admin can call. Preserves existing slot data.
    pub fn migrate_channel_state(ctx: Context<MigrateChannelState>, channel: String) -> Result<()> {
        instructions::migrate_channel::migrate_channel_state(ctx, channel)
    }

    /// Resize a channel state account to match the current `CHANNEL_RING_SLOTS`.
    /// Required after increasing the ring buffer window (e.g., 10 → 2016 epochs).
    pub fn resize_channel_state(ctx: Context<ResizeChannelState>) -> Result<()> {
        instructions::resize_channel::resize_channel_state(ctx)
    }

    // -------------------------------------------------------------------------
    // Governance (DeFi Rails)
    // -------------------------------------------------------------------------

    /// Update the base transfer fee configuration.
    pub fn update_fee_config(
        ctx: Context<UpdateFeeConfig>,
        new_basis_points: u16,
        fee_split: FeeSplit,
    ) -> Result<()> {
        instructions::governance::update_fee_config(ctx, new_basis_points, fee_split)
    }

    /// Update fee configuration for a specific mint instance (Open Pattern).
    pub fn update_fee_config_open(
        ctx: Context<UpdateFeeConfigOpen>,
        new_basis_points: u16,
        fee_split: FeeSplit,
    ) -> Result<()> {
        instructions::governance::update_fee_config_open(ctx, new_basis_points, fee_split)
    }

    /// Update the dynamic tier multipliers used for fee discounting/rewards.
    pub fn update_tier_multipliers(
        ctx: Context<UpdateTierMultipliers>,
        new_multipliers: [u32; 6],
    ) -> Result<()> {
        instructions::governance::update_tier_multipliers(ctx, new_multipliers)
    }

    /// Harvest withheld fees from the mint and distribute to protocol destinations.
    /// This closes the economic loop by refilling the Treasury.
    pub fn harvest_fees(ctx: Context<HarvestFees>) -> Result<()> {
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

    /// Bootstrap mint for CCM-v2 (before protocol_state initialized).
    /// Admin-only, uses hardcoded ADMIN_AUTHORITY.
    pub fn admin_mint_v2(ctx: Context<AdminMintV2>, amount: u64) -> Result<()> {
        instructions::admin::admin_mint_v2(ctx, amount)
    }

    // -------------------------------------------------------------------------
    // Identity Layer (Passport)
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
    // Staking System (V1)
    // -------------------------------------------------------------------------

    /// Initialize the stake pool for a mint (admin only)
    pub fn initialize_stake_pool(
        ctx: Context<InitializeStakePool>,
        reward_rate: u64,
    ) -> Result<()> {
        instructions::staking::initialize_stake_pool(ctx, reward_rate)
    }

    /// Stake CCM tokens with optional lock period
    pub fn stake(ctx: Context<Stake>, amount: u64, lock_slots: u64) -> Result<()> {
        instructions::staking::stake(ctx, amount, lock_slots)
    }

    /// Unstake CCM tokens (after lock expires)
    pub fn unstake(ctx: Context<Unstake>, amount: u64) -> Result<()> {
        instructions::staking::unstake(ctx, amount)
    }

    /// Delegate stake to a channel (backs creator for network effects)
    pub fn delegate_stake(ctx: Context<DelegateStake>, subject_id: Option<[u8; 32]>) -> Result<()> {
        instructions::staking::delegate_stake(ctx, subject_id)
    }

    /// Claim accumulated staking rewards
    pub fn claim_stake_rewards(ctx: Context<ClaimStakeRewards>) -> Result<()> {
        instructions::staking::claim_stake_rewards(ctx)
    }

    // -------------------------------------------------------------------------
    // Creator Extensions (V1)
    // -------------------------------------------------------------------------

    /// Initialize channel metadata for creator revenue sharing
    pub fn initialize_channel_meta(
        ctx: Context<InitializeChannelMeta>,
        channel: String,
        creator_wallet: Pubkey,
        fee_share_bps: u16,
    ) -> Result<()> {
        instructions::creator::initialize_channel_meta(ctx, channel, creator_wallet, fee_share_bps)
    }

    /// Update the creator wallet for fee distribution
    pub fn set_creator_wallet(ctx: Context<SetCreatorWallet>, new_wallet: Pubkey) -> Result<()> {
        instructions::creator::set_creator_wallet(ctx, new_wallet)
    }

    /// Update the creator fee share percentage
    pub fn set_creator_fee_share(
        ctx: Context<SetCreatorFeeShare>,
        new_fee_share_bps: u16,
    ) -> Result<()> {
        instructions::creator::set_creator_fee_share(ctx, new_fee_share_bps)
    }

    /// Update total delegated stake for a channel (publisher operation)
    pub fn update_total_delegated(
        ctx: Context<UpdateTotalDelegated>,
        total_delegated: u64,
    ) -> Result<()> {
        instructions::creator::update_total_delegated(ctx, total_delegated)
    }

    // -------------------------------------------------------------------------
    // Migration (CCM-v1 → CCM-v2)
    // -------------------------------------------------------------------------

    /// Migrate CCM tokens from v1 (no TransferFeeConfig) to v2 (with TransferFeeConfig).
    /// Burns v1 tokens and mints v2 tokens at 1:1 ratio.
    #[cfg(feature = "migration")]
    pub fn migrate(ctx: Context<Migrate>, amount: u64) -> Result<()> {
        instructions::migrate::migrate(ctx, amount)
    }

    // -------------------------------------------------------------------------
    // Legacy / Deprecated Paths
    // -------------------------------------------------------------------------
    // Note: These are feature-gated and should be disabled in production builds
    // unless required for backward compatibility with V1 state.

    #[cfg(feature = "legacy")]
    pub fn initialize_mint_open(
        ctx: Context<InitializeMintOpen>,
        fee_basis_points: u16,
        max_fee: u64,
    ) -> Result<()> {
        instructions::initialize_mint::handler_open(ctx, fee_basis_points, max_fee)
    }

    #[cfg(feature = "legacy")]
    pub fn set_merkle_root(
        ctx: Context<SetMerkleRoot>,
        root: [u8; 32],
        epoch: u64,
        claim_count: u32,
        subject_id: Pubkey,
    ) -> Result<()> {
        instructions::merkle::set_merkle_root(ctx, root, epoch, claim_count, subject_id)
    }

    #[cfg(feature = "legacy")]
    pub fn claim(
        ctx: Context<Claim>,
        subject_index: u8,
        index: u32,
        amount: u64,
        id: String,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::claim::claim(ctx, subject_index, index, amount, id, proof)
    }

    #[cfg(feature = "legacy")]
    pub fn claim_open(
        ctx: Context<ClaimOpen>,
        subject_index: u8,
        index: u32,
        amount: u64,
        id: String,
        proof: Vec<[u8; 32]>,
        channel: Option<String>,
        twzrd_epoch: Option<u64>,
    ) -> Result<()> {
        instructions::claim::claim_open(
            ctx,
            subject_index,
            index,
            amount,
            id,
            proof,
            channel,
            twzrd_epoch,
        )
    }

    #[cfg(feature = "legacy")]
    pub fn set_merkle_root_open(
        ctx: Context<SetMerkleRootOpen>,
        root: [u8; 32],
        epoch: u64,
        claim_count: u32,
        subject_id: Pubkey,
    ) -> Result<()> {
        instructions::merkle::set_merkle_root_open(ctx, root, epoch, claim_count, subject_id)
    }

    #[cfg(feature = "legacy")]
    pub fn close_epoch_state(
        ctx: Context<CloseEpochState>,
        epoch: u64,
        subject_id: Pubkey,
    ) -> Result<()> {
        instructions::cleanup::close_epoch_state(ctx, epoch, subject_id)
    }

    #[cfg(feature = "legacy")]
    pub fn force_close_epoch_state_legacy(
        ctx: Context<ForceCloseEpochStateLegacy>,
        epoch: u64,
        subject_id: Pubkey,
    ) -> Result<()> {
        instructions::cleanup::force_close_epoch_state_legacy(ctx, epoch, subject_id)
    }

    #[cfg(feature = "legacy")]
    pub fn force_close_epoch_state_open(
        ctx: Context<ForceCloseEpochStateOpen>,
        epoch: u64,
        subject_id: Pubkey,
        mint: Pubkey,
    ) -> Result<()> {
        instructions::cleanup::force_close_epoch_state_open(ctx, epoch, subject_id, mint)
    }

    /// Close a channel state account (admin only, via ProtocolState)
    pub fn close_channel_state(ctx: Context<CloseChannelState>, subject_id: Pubkey) -> Result<()> {
        instructions::cleanup::close_channel_state(ctx, subject_id)
    }

    /// Force close legacy channel state (hardcoded admin, for pre-ops cleanup)
    pub fn force_close_channel_state_legacy(
        ctx: Context<ForceCloseChannelStateLegacy>,
        subject_id: Pubkey,
        mint: Pubkey,
    ) -> Result<()> {
        instructions::cleanup::force_close_channel_state_legacy(ctx, subject_id, mint)
    }

    // -------------------------------------------------------------------------
    // Lofi Bank Integration (Claim + Auto-Stake)
    // -------------------------------------------------------------------------

    /// Claim tokens from a channel epoch with optional auto-stake to lofi-bank.
    /// Atomically claims merkle proof rewards and stakes a percentage.
    pub fn claim_channel_and_stake<'info>(
        ctx: Context<'_, '_, '_, 'info, ClaimChannelAndStake<'info>>,
        channel: String,
        epoch: u64,
        index: u32,
        amount: u64,
        id: String,
        proof: Vec<[u8; 32]>,
        auto_stake: bool,
        stake_percent: u8,
        lock_epochs: u32,
    ) -> Result<()> {
        instructions::claim_stake::claim_channel_and_stake(
            ctx,
            channel,
            epoch,
            index,
            amount,
            id,
            proof,
            auto_stake,
            stake_percent,
            lock_epochs,
        )
    }
}
