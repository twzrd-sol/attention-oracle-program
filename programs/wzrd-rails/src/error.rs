//! wzrd-rails custom program errors.
//!
//! New variants added alongside the IX that raises them. Each variant carries
//! the precondition that failed — never a generic "something went wrong."

use anchor_lang::prelude::*;

#[error_code]
pub enum RailsError {
    #[msg("Unauthorized: signer is not the configured admin.")]
    Unauthorized = 0,

    #[msg("Stake amount must be positive.")]
    StakeAmountZero = 1,

    #[msg("Invalid CCM mint account.")]
    InvalidMint = 2,

    #[msg("Invalid token program.")]
    InvalidTokenProgram = 3,

    #[msg("Stake is still within lock window; unstake not yet allowed.")]
    LockActive = 4,

    #[msg("Reward pool empty; nothing to claim.")]
    NoRewardsAvailable = 5,

    #[msg("Compensation merkle root already set; one-time IX.")]
    CompensationAlreadySet = 6,

    #[msg("Compensation proof did not verify against the stored root.")]
    CompensationInvalidProof = 7,

    #[msg("Compensation recipient already claimed.")]
    CompensationAlreadyClaimed = 8,

    #[msg("Math overflow during reward accumulator calculation.")]
    MathOverflow = 9,

    #[msg("No active stake to unstake.")]
    NothingStaked = 10,

    #[msg("Reward rate exceeds the configured per-slot safety cap.")]
    RewardRateTooHigh = 11,

    #[msg("Compensation vault is underfunded for this claim.")]
    CompensationUnavailable = 12,

    #[msg("Pool ID must equal total_pools (sequential numbering enforced).")]
    InvalidPoolId = 13,
}

#[error_code]
pub enum ListenPayoutError {
    #[msg("Listen payout publishing and claiming are paused")]
    Paused = 100,

    #[msg("Authority is not in the publisher allow-list")]
    UnauthorizedPublisher = 101,

    #[msg("Schema version does not match LISTEN_PAYOUT_ALLOCATION_LEAF_SCHEMA_V1")]
    SchemaVersionMismatch = 102,

    #[msg("Window ID must be strictly greater than the last published")]
    WindowIdNotMonotonic = 103,

    #[msg("Leaf count must be greater than zero")]
    ZeroLeafCount = 104,

    #[msg("Leaf count exceeds MAX_LEAVES_PER_WINDOW")]
    LeafCountExceedsMax = 105,

    #[msg("Merkle root must not be all zeros")]
    ZeroMerkleRoot = 106,

    #[msg("Total amount exceeds the per-window CCM cap")]
    ExceedsPerWindowCap = 107,

    #[msg("Leaf window_id does not match the payout window account")]
    LeafWindowMismatch = 108,

    #[msg("Claimer signer does not match leaf.wallet_pubkey")]
    ClaimerWalletMismatch = 109,

    #[msg("Leaf index is out of bounds for this window")]
    LeafIndexOutOfBounds = 110,

    #[msg("This leaf has already been claimed")]
    AlreadyClaimed = 111,

    #[msg("Merkle proof exceeds maximum allowed length")]
    ProofTooLong = 112,

    #[msg("Merkle proof does not verify against published root")]
    InvalidMerkleProof = 113,

    #[msg("Cannot claim a zero-amount leaf")]
    ZeroAmountClaim = 114,

    #[msg("Caller is not the admin of this payout config")]
    NotAdmin = 115,

    #[msg("Allow-list cannot be empty")]
    EmptyAllowlist = 116,

    #[msg("Allow-list exceeds MAX_PUBLISHERS")]
    TooManyPublishers = 117,

    #[msg("Allow-list contains duplicate publishers")]
    DuplicatePublisher = 118,
}
