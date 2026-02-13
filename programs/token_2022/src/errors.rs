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

    #[msg("Insufficient treasury funding for reward rate (need at least 1 day runway)")]
    InsufficientTreasuryFunding,

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

    #[msg("Reward rate exceeds maximum APR cap (15%)")]
    RewardRateExceedsMaxApr,

    #[msg("Pool is shutdown - no new stakes accepted")]
    PoolIsShutdown,

    #[msg("Claim pending rewards before unstaking")]
    PendingRewardsOnUnstake,

    #[msg("Reward claim would exceed available rewards (principal protection)")]
    ClaimExceedsAvailableRewards,

    #[msg("Pool must be shut down before closing")]
    PoolNotShutdown,

    // =========================================================================
    // V3 CLAIMS (STAKE SNAPSHOT)
    // =========================================================================
    #[msg("Stake snapshot mismatch - user has unstaked below snapshot amount")]
    StakeSnapshotMismatch,

    #[msg("Proof has expired - snapshot_slot is too old")]
    ProofExpired,

    #[msg("V2 claims are disabled after cutover epoch - use V3")]
    V2ClaimsDisabled,
}

// =============================================================================
// Exported error codes for cross-program CPI error matching.
// Anchor custom errors = 6000 + variant index.
// Validated by tests below — inserting new variants will break the test,
// forcing an update here and in any consumer (e.g. channel-vault compound).
// =============================================================================

/// Anchor error code offset for `#[error_code]` enums.
pub const ANCHOR_ERROR_OFFSET: u32 = 6000;

/// `OracleError::NoRewardsToClaim` (variant index 42)
pub const ORACLE_ERROR_NO_REWARDS_TO_CLAIM: u32 = ANCHOR_ERROR_OFFSET + 42;

/// `OracleError::PoolIsShutdown` (variant index 45)
pub const ORACLE_ERROR_POOL_IS_SHUTDOWN: u32 = ANCHOR_ERROR_OFFSET + 45;

/// `OracleError::ClaimExceedsAvailableRewards` (variant index 47)
pub const ORACLE_ERROR_CLAIM_EXCEEDS_AVAILABLE: u32 = ANCHOR_ERROR_OFFSET + 47;

#[cfg(test)]
mod tests {
    use super::*;

    /// Extract the Anchor error code number from an OracleError variant.
    fn error_code(e: OracleError) -> u32 {
        let err: anchor_lang::error::Error = e.into();
        match err {
            anchor_lang::error::Error::AnchorError(ae) => ae.error_code_number,
            _ => panic!("expected AnchorError"),
        }
    }

    #[test]
    fn error_code_constants_match_enum() {
        assert_eq!(
            ORACLE_ERROR_NO_REWARDS_TO_CLAIM,
            error_code(OracleError::NoRewardsToClaim),
            "NoRewardsToClaim code drifted — update ORACLE_ERROR_NO_REWARDS_TO_CLAIM"
        );
        assert_eq!(
            ORACLE_ERROR_POOL_IS_SHUTDOWN,
            error_code(OracleError::PoolIsShutdown),
            "PoolIsShutdown code drifted — update ORACLE_ERROR_POOL_IS_SHUTDOWN"
        );
        assert_eq!(
            ORACLE_ERROR_CLAIM_EXCEEDS_AVAILABLE,
            error_code(OracleError::ClaimExceedsAvailableRewards),
            "ClaimExceedsAvailableRewards code drifted — update ORACLE_ERROR_CLAIM_EXCEEDS_AVAILABLE"
        );
    }
}
