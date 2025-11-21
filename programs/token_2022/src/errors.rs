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

    #[msg("Drip threshold not met")]
    DripThresholdNotMet,

    #[msg("Drip already executed for this tier")]
    DripAlreadyExecuted,

    #[msg("Invalid drip tier")]
    InvalidDripTier,

    #[msg("Insufficient treasury balance")]
    InsufficientTreasuryBalance,

    #[msg("Pool not initialized")]
    PoolNotInitialized,

    #[msg("Volume too low for operation")]
    VolumeTooLow,

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

    #[msg("Insufficient points for gated action")]
    InsufficientPoints,

    #[msg("Invalid input length")]
    InvalidInputLength,

    #[msg("Invalid proof length")]
    InvalidProofLength,

    #[msg("Invalid mint")]
    InvalidMint,

    #[msg("TWZRD Layer-1 receipt required for this claim")]
    ReceiptRequired,

    #[msg("Invalid channel state PDA")]
    InvalidChannelState,

    #[msg("Requested epoch slot is not available")]
    SlotMismatch,

    #[msg("Missing required bubblegum accounts for cNFT minting")]
    MissingBubblegumAccounts,

    #[msg("Invalid subject key")]
    InvalidStreamer,

    #[msg("Invalid epoch")]
    InvalidEpoch,

    #[msg("Invalid epoch state PDA")]
    InvalidEpochState,

    #[msg("Channel not initialized")]
    ChannelNotInitialized,

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
}
