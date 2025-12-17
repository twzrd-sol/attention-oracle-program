use anchor_lang::prelude::*;

#[error_code]
pub enum OracleError {
    #[msg("Unauthorized")]
    Unauthorized,

    #[msg("Already initialized")]
    AlreadyInitialized,

    #[msg("Fee basis points too high (max 1000 = 10%)")]
    InvalidFeeBps,

    #[msg("Invalid fee split - must sum to 100")]
    InvalidFeeSplit,

    #[msg("Fee too high - maximum 10%")]
    FeeTooHigh,

    #[msg("Protocol is paused")]
    ProtocolPaused,

    #[msg("Insufficient treasury balance")]
    InsufficientTreasuryBalance,

    #[msg("Already claimed")]
    AlreadyClaimed,

    #[msg("Invalid merkle proof")]
    InvalidProof,

    #[msg("Epoch closed")]
    EpochClosed,

    #[msg("Invalid index")]
    InvalidIndex,

    #[msg("Invalid amount")]
    InvalidAmount,

    #[msg("Invalid input length")]
    InvalidInputLength,

    #[msg("Invalid proof length")]
    InvalidProofLength,

    #[msg("Invalid mint")]
    InvalidMint,

    #[msg("Invalid channel state PDA")]
    InvalidChannelState,

    #[msg("Requested epoch slot is not available")]
    SlotMismatch,

    #[msg("Invalid subject key")]
    InvalidStreamer,

    #[msg("Invalid epoch")]
    InvalidEpoch,

    #[msg("Invalid epoch state PDA")]
    InvalidEpochState,

    #[msg("Channel not initialized")]
    ChannelNotInitialized,

    #[msg("Invalid root for channel epoch")]
    InvalidRoot,

    #[msg("Batch already pushed")]
    AlreadyPushed,

    // New errors introduced by audit hardening
    #[msg("Epoch already initialized")]
    EpochAlreadyInitialized,

    #[msg("Epoch not fully claimed")]
    EpochNotFullyClaimed,

    #[msg("Epoch must be strictly increasing for this slot")]
    EpochNotIncreasing,

    #[msg("Invalid pubkey (cannot be default)")]
    InvalidPubkey,

    #[msg("Invalid user hash")]
    InvalidUserHash,

    #[msg("Downgrades are not allowed")]
    DowngradeNotAllowed,

    #[msg("Invalid tier")]
    InvalidTier,

    #[msg("Invalid mint data")]
    InvalidMintData,

    #[msg("Missing transfer fee extension")]
    MissingTransferFeeExtension,

    #[msg("Math overflow")]
    MathOverflow,

    #[msg("Account already at or above target size")]
    AccountTooLarge,

    // =========================================================================
    // STAKING ERRORS (V1)
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
    // CREATOR ERRORS (V1)
    // =========================================================================
    #[msg("Creator fee share too high")]
    CreatorFeeTooHigh,

    #[msg("Channel meta not initialized")]
    ChannelMetaNotInitialized,

    #[msg("Invalid token program (expected Token-2022)")]
    InvalidTokenProgram,
}
