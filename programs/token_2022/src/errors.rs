//! Error definitions for the Attention Oracle Protocol.

use anchor_lang::prelude::*;

#[error_code]
pub enum OracleError {
    // =========================================================================
    // ACCESS CONTROL
    // =========================================================================
    #[msg("Unauthorized")]
    Unauthorized,

    #[msg("Already initialized")]
    AlreadyInitialized,

    #[msg("Protocol is paused")]
    ProtocolPaused,

    #[msg("Invalid pubkey (cannot be default)")]
    InvalidPubkey,

    // =========================================================================
    // CLAIMS & PROOFS
    // =========================================================================
    #[msg("Invalid merkle proof")]
    InvalidProof,

    #[msg("Invalid proof length")]
    InvalidProofLength,

    #[msg("Invalid root sequence (must be strictly increasing)")]
    InvalidRootSeq,

    #[msg("Root too old or missing from history window")]
    RootTooOldOrMissing,

    #[msg("Claim state mismatch")]
    InvalidClaimState,

    // =========================================================================
    // CHANNEL & EPOCH
    // =========================================================================
    #[msg("Invalid channel state PDA")]
    InvalidChannelState,

    #[msg("Channel not initialized")]
    ChannelNotInitialized,

    #[msg("Requested epoch slot is not available")]
    SlotMismatch,

    // =========================================================================
    // FEES & ECONOMICS
    // =========================================================================
    #[msg("Fee basis points too high (max 1000 = 10%)")]
    InvalidFeeBps,

    #[msg("Invalid fee split - must sum to 100")]
    InvalidFeeSplit,

    #[msg("Creator fee share too high")]
    CreatorFeeTooHigh,

    #[msg("Creator ATA required when creator fee > 0")]
    MissingCreatorAta,

    // =========================================================================
    // TOKENS & TRANSFERS
    // =========================================================================
    #[msg("Invalid mint")]
    InvalidMint,

    #[msg("Invalid mint data")]
    InvalidMintData,

    #[msg("Missing transfer fee extension")]
    MissingTransferFeeExtension,

    #[msg("Invalid token program (expected Token-2022)")]
    InvalidTokenProgram,

    #[msg("Insufficient treasury balance")]
    InsufficientTreasuryBalance,

    // =========================================================================
    // STAKING
    // =========================================================================
    #[msg("Insufficient stake balance")]
    InsufficientStake,

    #[msg("Tokens are still locked")]
    TokensLocked,

    #[msg("Stake amount below minimum")]
    StakeBelowMinimum,

    #[msg("Lock period too long")]
    LockPeriodTooLong,

    #[msg("No pending rewards")]
    NoPendingRewards,

    // =========================================================================
    // CHANNEL STAKING
    // =========================================================================
    #[msg("Channel stake pool not initialized")]
    ChannelStakePoolNotInitialized,

    #[msg("Channel stake pool already exists")]
    ChannelStakePoolExists,

    #[msg("Cannot close non-empty stake pool")]
    StakePoolNotEmpty,

    #[msg("Position already has NFT minted")]
    NftAlreadyMinted,

    #[msg("Position does not have NFT")]
    NftNotMinted,

    #[msg("NFT holder mismatch")]
    NftHolderMismatch,

    #[msg("Lock period not expired")]
    LockNotExpired,

    #[msg("Lock has expired, use standard unstake to avoid penalty")]
    LockExpiredUseStandardUnstake,

    #[msg("Cannot reduce lock period")]
    LockReductionNotAllowed,

    #[msg("Subject mismatch between stake pool and channel config")]
    SubjectMismatch,

    // =========================================================================
    // IDENTITY / PASSPORT
    // =========================================================================
    #[msg("Invalid user hash")]
    InvalidUserHash,

    #[msg("Downgrades are not allowed")]
    DowngradeNotAllowed,

    #[msg("Invalid tier")]
    InvalidTier,

    // =========================================================================
    // GENERAL
    // =========================================================================
    #[msg("Invalid input length")]
    InvalidInputLength,

    #[msg("Math overflow")]
    MathOverflow,

    #[msg("No rewards available to claim")]
    NoRewardsToClaim,

    #[msg("Invalid channel name (must be 1-64 ASCII characters)")]
    InvalidChannelName,
}
