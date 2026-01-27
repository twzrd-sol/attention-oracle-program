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
    #[msg("Already claimed")]
    AlreadyClaimed,

    #[msg("Invalid merkle proof")]
    InvalidProof,

    #[msg("Invalid proof length")]
    InvalidProofLength,

    #[msg("Invalid root for channel epoch")]
    InvalidRoot,

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

    #[msg("Invalid epoch")]
    InvalidEpoch,

    #[msg("Invalid epoch state PDA")]
    InvalidEpochState,

    #[msg("Epoch closed")]
    EpochClosed,

    #[msg("Epoch already initialized")]
    EpochAlreadyInitialized,

    #[msg("Epoch not fully claimed")]
    EpochNotFullyClaimed,

    #[msg("Epoch must be strictly increasing for this slot")]
    EpochNotIncreasing,

    // =========================================================================
    // FEES & ECONOMICS
    // =========================================================================
    #[msg("Fee basis points too high (max 1000 = 10%)")]
    InvalidFeeBps,

    #[msg("Invalid fee split - must sum to 100")]
    InvalidFeeSplit,

    #[msg("Fee too high - maximum 10%")]
    FeeTooHigh,

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

    #[msg("Invalid amount")]
    InvalidAmount,

    #[msg("Withdrawal exceeds per-transaction limit (50M CCM)")]
    ExceedsWithdrawLimit,

    #[msg("Withdrawal exceeds daily limit (100M CCM)")]
    DailyLimitExceeded,

    // =========================================================================
    // STAKING
    // =========================================================================
    #[msg("Stake pool not initialized")]
    StakePoolNotInitialized,

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
    // IDENTITY / PASSPORT
    // =========================================================================
    #[msg("Invalid subject key")]
    InvalidStreamer,

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

    #[msg("Invalid index")]
    InvalidIndex,

    #[msg("Math overflow")]
    MathOverflow,

    #[msg("Account already at or above target size")]
    AccountTooLarge,

    #[msg("Batch already pushed")]
    AlreadyPushed,

    #[msg("Channel meta not initialized")]
    ChannelMetaNotInitialized,

    #[msg("Invalid channel name (must be 1-64 ASCII characters)")]
    InvalidChannelName,
}
