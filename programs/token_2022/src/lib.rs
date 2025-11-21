#![allow(ambiguous_glob_reexports)]
// Hygiene: Enforce standard lints
#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
// Anchor entrypoints necessarily exceed argument-count norms and return Result
// without per-function # Errors sections; allow these pedantic lints while
// keeping everything else strict.
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

use anchor_lang::prelude::*;

#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

pub use constants::*;
pub use errors::*;
pub use events::*;
pub use instructions::*;
pub use state::*;

declare_id!("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");

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

    // -------------------------------------------------------------------------
    // Canonical Initialization
    // -------------------------------------------------------------------------

    /// Initialize protocol with existing Token-2022 mint
    pub fn initialize_mint(
        ctx: Context<InitializeMint>,
        fee_basis_points: u16,
        max_fee: u64,
    ) -> Result<()> {
        instructions::initialize_mint::handler(ctx, fee_basis_points, max_fee)
    }

    // -------------------------------------------------------------------------
    // Canonical Claims (Ring Buffer)
    // -------------------------------------------------------------------------

    /// Ring-buffer publish: store latest epoch root in channel state
    pub fn set_channel_merkle_root(
        ctx: Context<SetChannelMerkleRoot>,
        channel: String,
        epoch: u64,
        root: [u8; 32],
    ) -> Result<()> {
        instructions::channel::set_channel_merkle_root(ctx, channel, epoch, root)
    }

    /// Claim using channel ring buffer epoch state
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

    /// Claim with optional cNFT receipt minting (fee-only, rent-free)
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
        instructions::channel::claim_channel_open_with_receipt(
            ctx,
            channel,
            epoch,
            index,
            amount,
            id,
            proof,
            mint_receipt,
        )
    }

    // -------------------------------------------------------------------------
    // Token-2022 Hooks & Governance
    // -------------------------------------------------------------------------

    /// Transfer hook: currently emits event for indexers; future: fee routing
    pub fn transfer_hook(ctx: Context<TransferHook>, amount: u64) -> Result<()> {
        instructions::hooks::transfer_hook(ctx, amount)
    }

    /// Governance: update fee config (basis points)
    pub fn update_fee_config(
        ctx: Context<UpdateFeeConfig>,
        new_basis_points: u16,
        fee_split: FeeSplit,
    ) -> Result<()> {
        instructions::governance::update_fee_config(ctx, new_basis_points, fee_split)
    }

    /// Governance (open): update fee config for mint-keyed instance
    pub fn update_fee_config_open(
        ctx: Context<UpdateFeeConfigOpen>,
        new_basis_points: u16,
        fee_split: FeeSplit,
    ) -> Result<()> {
        instructions::governance::update_fee_config_open(ctx, new_basis_points, fee_split)
    }

    /// Governance: update tier multipliers for dynamic fee allocation
    pub fn update_tier_multipliers(
        ctx: Context<UpdateTierMultipliers>,
        new_multipliers: [u32; 6],
    ) -> Result<()> {
        instructions::governance::update_tier_multipliers(ctx, new_multipliers)
    }

    /// Harvest withheld fees from Token-2022 mint and distribute to treasury/creator pool
    pub fn harvest_fees(ctx: Context<HarvestFees>) -> Result<()> {
        instructions::governance::harvest_and_distribute_fees(ctx)
    }

    // -------------------------------------------------------------------------
    // Admin & Policy
    // -------------------------------------------------------------------------

    /// Admin: set/rotate allowlisted publisher (singleton)
    pub fn update_publisher(ctx: Context<UpdatePublisher>, new_publisher: Pubkey) -> Result<()> {
        instructions::admin::update_publisher(ctx, new_publisher)
    }

    /// Admin: set/rotate allowlisted publisher (open variant keyed by mint)
    pub fn update_publisher_open(
        ctx: Context<UpdatePublisherOpen>,
        new_publisher: Pubkey,
    ) -> Result<()> {
        instructions::admin::update_publisher_open(ctx, new_publisher)
    }

    /// Admin: set receipt requirement policy (singleton)
    pub fn set_policy(ctx: Context<SetPolicy>, require_receipt: bool) -> Result<()> {
        instructions::admin::set_policy(ctx, require_receipt)
    }

    /// Admin: set receipt requirement policy (open variant keyed by mint)
    pub fn set_policy_open(ctx: Context<SetPolicyOpen>, require_receipt: bool) -> Result<()> {
        instructions::admin::set_policy_open(ctx, require_receipt)
    }

    /// Admin: emergency pause/unpause (singleton)
    pub fn set_paused(ctx: Context<SetPaused>, paused: bool) -> Result<()> {
        instructions::admin::set_paused(ctx, paused)
    }

    /// Admin: emergency pause/unpause (open variant keyed by mint)
    pub fn set_paused_open(ctx: Context<SetPausedOpen>, paused: bool) -> Result<()> {
        instructions::admin::set_paused_open(ctx, paused)
    }

    /// Admin: transfer admin authority (open variant keyed by mint)
    pub fn update_admin_open(ctx: Context<UpdateAdminOpen>, new_admin: Pubkey) -> Result<()> {
        instructions::admin::update_admin_open(ctx, new_admin)
    }

    /// Admin: transfer admin authority (singleton)
    pub fn update_admin(ctx: Context<UpdateAdmin>, new_admin: Pubkey) -> Result<()> {
        instructions::admin::update_admin(ctx, new_admin)
    }

    // -------------------------------------------------------------------------
    // Passport (Identity)
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

    pub fn upgrade_passport_proved(
        ctx: Context<UpgradePassportProved>,
        user_hash: [u8; 32],
        new_tier: u8,
        new_score: u64,
        epoch_count: u32,
        weighted_presence: u64,
        badges: u32,
        leaf_hash: [u8; 32],
        proof_nodes: Vec<[u8; 32]>,
        leaf_bytes: Vec<u8>,
    ) -> Result<()> {
        instructions::passport::upgrade_passport_proved(
            ctx,
            user_hash,
            new_tier,
            new_score,
            epoch_count,
            weighted_presence,
            badges,
            leaf_hash,
            proof_nodes,
            leaf_bytes,
        )
    }

    /// Gate: require that an owner holds at least `min` Points
    pub fn require_points_ge(ctx: Context<RequirePoints>, min: u64) -> Result<()> {
        instructions::points::require_points_ge(ctx, min)
    }

    // -------------------------------------------------------------------------
    // Legacy Paths (Feature Gated)
    // -------------------------------------------------------------------------

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
        receipt_proof: Option<CnftReceiptProof>,
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
            receipt_proof,
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
    pub fn claim_points_open(
        ctx: Context<ClaimPointsOpen>,
        index: u32,
        amount: u64,
        id: String,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::points::claim_points_open(ctx, index, amount, id, proof)
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

    // -------------------------------------------------------------------------
    // Demo Paths (Feature Gated)
    // -------------------------------------------------------------------------

    #[cfg(feature = "demo")]
    pub fn initialize_channel(ctx: Context<InitializeChannel>, subject_id: Pubkey) -> Result<()> {
        instructions::merkle_ring::initialize_channel(ctx, subject_id)
    }

    #[cfg(feature = "demo")]
    pub fn set_merkle_root_ring(
        ctx: Context<SetMerkleRootRing>,
        root: [u8; 32],
        epoch: u64,
        claim_count: u16,
        subject_id: Pubkey,
    ) -> Result<()> {
        instructions::merkle_ring::set_merkle_root_ring(ctx, root, epoch, claim_count, subject_id)
    }

    #[cfg(feature = "demo")]
    pub fn claim_with_ring(
        ctx: Context<ClaimWithRing>,
        epoch: u64,
        index: u32,
        amount: u64,
        proof: Vec<[u8; 32]>,
        subject_id: Pubkey,
    ) -> Result<()> {
        instructions::merkle_ring::claim_with_ring(ctx, epoch, index, amount, proof, subject_id)
    }

    #[cfg(feature = "demo")]
    pub fn close_old_epoch_state(ctx: Context<CloseOldEpochState>) -> Result<()> {
        instructions::merkle_ring::close_old_epoch_state(ctx)
    }
}
