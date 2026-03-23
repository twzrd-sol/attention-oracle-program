//! Error codes for the AO v2 program.
//!
//! Must preserve existing Anchor error codes: `6000 + variant_index`.
//! The server and SDK check these numeric values — reordering or inserting
//! variants will break error handling across the stack.
//!
//! Each variant's doc comment includes its numeric code for quick reference.

use pinocchio::program_error::ProgramError;

/// Anchor custom error offset. Anchor error codes = 6000 + variant index.
pub const ANCHOR_ERROR_OFFSET: u32 = 6000;

/// Oracle protocol errors. Variant order MUST match the Anchor `OracleError` enum
/// in `programs/attention-oracle/src/errors.rs` exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum OracleError {
    // =========================================================================
    // ACCESS CONTROL (0-3)
    // =========================================================================
    /// 6000 — Unauthorized
    Unauthorized = 0,
    /// 6001 — Already initialized
    AlreadyInitialized = 1,
    /// 6002 — Protocol is paused
    ProtocolPaused = 2,
    /// 6003 — Invalid pubkey (cannot be default)
    InvalidPubkey = 3,

    // =========================================================================
    // CLAIMS & PROOFS (4-8)
    // =========================================================================
    /// 6004 — Invalid merkle proof
    InvalidProof = 4,
    /// 6005 — Invalid proof length
    InvalidProofLength = 5,
    /// 6006 — Invalid root sequence (must be strictly increasing)
    InvalidRootSeq = 6,
    /// 6007 — Root too old or missing from history window
    RootTooOldOrMissing = 7,
    /// 6008 — Claim state mismatch
    InvalidClaimState = 8,

    // =========================================================================
    // CHANNEL & EPOCH (9-11)
    // =========================================================================
    /// 6009 — Invalid channel state PDA
    InvalidChannelState = 9,
    /// 6010 — Channel not initialized
    ChannelNotInitialized = 10,
    /// 6011 — Requested epoch slot is not available
    SlotMismatch = 11,

    // =========================================================================
    // FEES & ECONOMICS (12-15)
    // =========================================================================
    /// 6012 — Fee basis points too high (max 1000 = 10%)
    InvalidFeeBps = 12,
    /// 6013 — Invalid fee split - must sum to 100
    InvalidFeeSplit = 13,
    /// 6014 — Creator fee share too high
    CreatorFeeTooHigh = 14,
    /// 6015 — Creator ATA required when creator fee > 0
    MissingCreatorAta = 15,

    // =========================================================================
    // TOKENS & TRANSFERS (16-21)
    // =========================================================================
    /// 6016 — Invalid mint
    InvalidMint = 16,
    /// 6017 — Invalid mint data
    InvalidMintData = 17,
    /// 6018 — Missing transfer fee extension
    MissingTransferFeeExtension = 18,
    /// 6019 — Invalid token program (expected Token-2022)
    InvalidTokenProgram = 19,
    /// 6020 — Insufficient treasury balance
    InsufficientTreasuryBalance = 20,
    /// 6021 — Insufficient treasury funding for reward rate
    InsufficientTreasuryFunding = 21,

    // =========================================================================
    // STAKING (22-26)
    // =========================================================================
    /// 6022 — Insufficient stake balance
    InsufficientStake = 22,
    /// 6023 — Tokens are still locked
    TokensLocked = 23,
    /// 6024 — Stake amount below minimum
    StakeBelowMinimum = 24,
    /// 6025 — Lock period too long
    LockPeriodTooLong = 25,
    /// 6026 — No pending rewards
    NoPendingRewards = 26,

    // =========================================================================
    // CHANNEL STAKING (27-36)
    // =========================================================================
    /// 6027 — Channel stake pool not initialized
    ChannelStakePoolNotInitialized = 27,
    /// 6028 — Channel stake pool already exists
    ChannelStakePoolExists = 28,
    /// 6029 — Cannot close non-empty stake pool
    StakePoolNotEmpty = 29,
    /// 6030 — Position already has NFT minted
    NftAlreadyMinted = 30,
    /// 6031 — Position does not have NFT
    NftNotMinted = 31,
    /// 6032 — NFT holder mismatch
    NftHolderMismatch = 32,
    /// 6033 — Lock period not expired
    LockNotExpired = 33,
    /// 6034 — Lock has expired, use standard unstake to avoid penalty
    LockExpiredUseStandardUnstake = 34,
    /// 6035 — Cannot reduce lock period
    LockReductionNotAllowed = 35,
    /// 6036 — Subject mismatch between stake pool and channel config
    SubjectMismatch = 36,

    // =========================================================================
    // IDENTITY / PASSPORT (37-39)
    // =========================================================================
    /// 6037 — Invalid user hash
    InvalidUserHash = 37,
    /// 6038 — Downgrades are not allowed
    DowngradeNotAllowed = 38,
    /// 6039 — Invalid tier
    InvalidTier = 39,

    // =========================================================================
    // GENERAL (40-48)
    // =========================================================================
    /// 6040 — Invalid input length
    InvalidInputLength = 40,
    /// 6041 — Math overflow
    MathOverflow = 41,
    /// 6042 — No rewards available to claim
    NoRewardsToClaim = 42,
    /// 6043 — Invalid channel name (must be 1-64 ASCII characters)
    InvalidChannelName = 43,
    /// 6044 — Reward rate exceeds maximum APR cap (15%)
    RewardRateExceedsMaxApr = 44,
    /// 6045 — Pool is shutdown - no new stakes accepted
    PoolIsShutdown = 45,
    /// 6046 — Claim pending rewards before unstaking
    PendingRewardsOnUnstake = 46,
    /// 6047 — Reward claim would exceed available rewards (principal protection)
    ClaimExceedsAvailableRewards = 47,
    /// 6048 — Pool must be shut down before closing
    PoolNotShutdown = 48,

    // =========================================================================
    // V3 CLAIMS (STAKE SNAPSHOT) (49-51)
    // =========================================================================
    /// 6049 — Stake snapshot mismatch
    StakeSnapshotMismatch = 49,
    /// 6050 — Proof has expired
    ProofExpired = 50,
    /// 6051 — V2 claims are disabled after cutover epoch - use V3
    V2ClaimsDisabled = 51,

    // =========================================================================
    // GLOBAL ROOT (V4) (52)
    // =========================================================================
    /// 6052 — Global root config not initialized
    GlobalRootNotInitialized = 52,

    // =========================================================================
    // CREATOR MARKETS (53-66)
    // =========================================================================
    /// 6053 — Invalid market state
    InvalidMarketState = 53,
    /// 6054 — Market metric is not supported
    UnsupportedMarketMetric = 54,
    /// 6055 — Market has already been resolved
    MarketAlreadyResolved = 55,
    /// 6056 — Market cannot be resolved yet
    MarketNotResolvableYet = 56,
    /// 6057 — Market tokens not initialized
    MarketTokensNotInitialized = 57,
    /// 6058 — Market tokens already initialized
    MarketTokensAlreadyInitialized = 58,
    /// 6059 — Zero shares minted - deposit too small after transfer fee
    ZeroSharesMinted = 59,
    /// 6060 — Unequal YES/NO share amounts for redemption
    UnequalShareAmounts = 60,
    /// 6061 — Market not resolved yet - cannot settle
    MarketNotResolved = 61,
    /// 6062 — Wrong outcome token
    WrongOutcomeToken = 62,
    /// 6063 — Insufficient vault balance for settlement
    InsufficientVaultBalance = 63,
    /// 6064 — Winning shares still outstanding
    WinningSharesStillOutstanding = 64,
    /// 6065 — Vault is not empty
    VaultNotEmpty = 65,
    /// 6066 — Position has already been settled
    AlreadySettled = 66,

    // =========================================================================
    // VAULT (67-75)
    // =========================================================================
    /// 6067 — Attention multiplier exceeds maximum allowed (5.0x = 50000 BPS)
    MaxMultiplierExceeded = 67,
    /// 6068 — No yield available to claim
    NothingToClaim = 68,
    /// 6069 — Strategy reserve would fall below the configured minimum
    InsufficientReserve = 69,
    /// 6070 — Strategy deployment exceeds the configured utilization cap
    UtilizationCapExceeded = 70,
    /// 6071 — Strategy vault is not active
    StrategyInactive = 71,
    /// 6072 — Strategy amount is below the minimum threshold
    StrategyAmountTooSmall = 72,
    /// 6073 — Invalid external account for strategy CPI
    InvalidExternalAccount = 73,
    /// 6074 — Invalid external state for strategy CPI
    InvalidExternalState = 74,
    /// 6075 — Requested withdrawal exceeds available strategy balance
    InsufficientStrategyBalance = 75,

    // =========================================================================
    // PRICE FEED (76-77)
    // =========================================================================
    /// 6076 — Price deviation exceeds maximum allowed (20%)
    PriceDeviationTooLarge = 76,
    /// 6077 — Price feed is stale
    PriceFeedStale = 77,

    // =========================================================================
    // PHASE 2 NAV / V5 CLAIMS (78-83, appended to preserve existing codes)
    // =========================================================================
    /// 6078 — Invalid merkle leaf version
    InvalidMerkleLeafVersion = 78,
    /// 6079 — Attention multiplier is below minimum allowed (1.0x = 10000 BPS)
    MultiplierBelowMinimum = 79,
    /// 6080 — NAV per share cannot decrease
    NavDecreaseNotAllowed = 80,
    /// 6081 — NAV per share is below minimum allowed (1.0x = 10000 BPS)
    NavBelowMinimum = 81,
    /// 6082 — NAV per share exceeds maximum allowed (5.0x = 50000 BPS)
    NavAboveMaximum = 82,
    /// 6083 — Direct claim_yield is deprecated; use claim_global merkle claims
    ClaimYieldDeprecated = 83,
}

impl From<OracleError> for ProgramError {
    #[inline]
    fn from(e: OracleError) -> Self {
        ProgramError::Custom(ANCHOR_ERROR_OFFSET + e as u32)
    }
}

// ============================================================================
// EXPORTED ERROR CODE CONSTANTS
//
// Used by cross-program CPI error matching (e.g. channel-vault compound).
// ============================================================================

/// `OracleError::NoRewardsToClaim` (variant index 42)
pub const ORACLE_ERROR_NO_REWARDS_TO_CLAIM: u32 = ANCHOR_ERROR_OFFSET + 42;

/// `OracleError::PoolIsShutdown` (variant index 45)
pub const ORACLE_ERROR_POOL_IS_SHUTDOWN: u32 = ANCHOR_ERROR_OFFSET + 45;

/// `OracleError::ClaimExceedsAvailableRewards` (variant index 47)
pub const ORACLE_ERROR_CLAIM_EXCEEDS_AVAILABLE: u32 = ANCHOR_ERROR_OFFSET + 47;

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Extract the custom error code from a ProgramError.
    fn error_code(e: OracleError) -> u32 {
        match ProgramError::from(e) {
            ProgramError::Custom(code) => code,
            _ => panic!("expected ProgramError::Custom"),
        }
    }

    #[test]
    fn error_codes_match_anchor() {
        // Spot-check critical error codes that the server and SDK depend on.
        assert_eq!(error_code(OracleError::Unauthorized), 6000);
        assert_eq!(error_code(OracleError::AlreadyInitialized), 6001);
        assert_eq!(error_code(OracleError::ProtocolPaused), 6002);
        assert_eq!(error_code(OracleError::InvalidPubkey), 6003);
        assert_eq!(error_code(OracleError::InvalidProof), 6004);
        assert_eq!(error_code(OracleError::InvalidProofLength), 6005);
        assert_eq!(error_code(OracleError::InvalidRootSeq), 6006);
        assert_eq!(error_code(OracleError::RootTooOldOrMissing), 6007);
        assert_eq!(error_code(OracleError::InvalidClaimState), 6008);
        assert_eq!(error_code(OracleError::InvalidChannelState), 6009);
        assert_eq!(error_code(OracleError::ChannelNotInitialized), 6010);
        assert_eq!(error_code(OracleError::SlotMismatch), 6011);
        assert_eq!(error_code(OracleError::InvalidFeeBps), 6012);
        assert_eq!(error_code(OracleError::InvalidFeeSplit), 6013);
        assert_eq!(error_code(OracleError::CreatorFeeTooHigh), 6014);
        assert_eq!(error_code(OracleError::MissingCreatorAta), 6015);
        assert_eq!(error_code(OracleError::InvalidMint), 6016);
        assert_eq!(error_code(OracleError::InvalidMintData), 6017);
        assert_eq!(error_code(OracleError::MissingTransferFeeExtension), 6018);
        assert_eq!(error_code(OracleError::InvalidTokenProgram), 6019);
        assert_eq!(error_code(OracleError::InsufficientTreasuryBalance), 6020);
        assert_eq!(error_code(OracleError::InsufficientTreasuryFunding), 6021);
        assert_eq!(error_code(OracleError::InsufficientStake), 6022);
        assert_eq!(error_code(OracleError::TokensLocked), 6023);
        assert_eq!(error_code(OracleError::StakeBelowMinimum), 6024);
        assert_eq!(error_code(OracleError::LockPeriodTooLong), 6025);
        assert_eq!(error_code(OracleError::NoPendingRewards), 6026);
        assert_eq!(
            error_code(OracleError::ChannelStakePoolNotInitialized),
            6027
        );
        assert_eq!(error_code(OracleError::ChannelStakePoolExists), 6028);
        assert_eq!(error_code(OracleError::StakePoolNotEmpty), 6029);
        assert_eq!(error_code(OracleError::NftAlreadyMinted), 6030);
        assert_eq!(error_code(OracleError::NftNotMinted), 6031);
        assert_eq!(error_code(OracleError::NftHolderMismatch), 6032);
        assert_eq!(error_code(OracleError::LockNotExpired), 6033);
        assert_eq!(error_code(OracleError::LockExpiredUseStandardUnstake), 6034);
        assert_eq!(error_code(OracleError::LockReductionNotAllowed), 6035);
        assert_eq!(error_code(OracleError::SubjectMismatch), 6036);
        assert_eq!(error_code(OracleError::InvalidUserHash), 6037);
        assert_eq!(error_code(OracleError::DowngradeNotAllowed), 6038);
        assert_eq!(error_code(OracleError::InvalidTier), 6039);
        assert_eq!(error_code(OracleError::InvalidInputLength), 6040);
        assert_eq!(error_code(OracleError::MathOverflow), 6041);
        assert_eq!(error_code(OracleError::NoRewardsToClaim), 6042);
        assert_eq!(error_code(OracleError::InvalidChannelName), 6043);
        assert_eq!(error_code(OracleError::RewardRateExceedsMaxApr), 6044);
        assert_eq!(error_code(OracleError::PoolIsShutdown), 6045);
        assert_eq!(error_code(OracleError::PendingRewardsOnUnstake), 6046);
        assert_eq!(error_code(OracleError::ClaimExceedsAvailableRewards), 6047);
        assert_eq!(error_code(OracleError::PoolNotShutdown), 6048);
        assert_eq!(error_code(OracleError::StakeSnapshotMismatch), 6049);
        assert_eq!(error_code(OracleError::ProofExpired), 6050);
        assert_eq!(error_code(OracleError::V2ClaimsDisabled), 6051);
        assert_eq!(error_code(OracleError::GlobalRootNotInitialized), 6052);
        assert_eq!(error_code(OracleError::InvalidMarketState), 6053);
        assert_eq!(error_code(OracleError::UnsupportedMarketMetric), 6054);
        assert_eq!(error_code(OracleError::MarketAlreadyResolved), 6055);
        assert_eq!(error_code(OracleError::MarketNotResolvableYet), 6056);
        assert_eq!(error_code(OracleError::MarketTokensNotInitialized), 6057);
        assert_eq!(
            error_code(OracleError::MarketTokensAlreadyInitialized),
            6058
        );
        assert_eq!(error_code(OracleError::ZeroSharesMinted), 6059);
        assert_eq!(error_code(OracleError::UnequalShareAmounts), 6060);
        assert_eq!(error_code(OracleError::MarketNotResolved), 6061);
        assert_eq!(error_code(OracleError::WrongOutcomeToken), 6062);
        assert_eq!(error_code(OracleError::InsufficientVaultBalance), 6063);
        assert_eq!(error_code(OracleError::WinningSharesStillOutstanding), 6064);
        assert_eq!(error_code(OracleError::VaultNotEmpty), 6065);
        assert_eq!(error_code(OracleError::AlreadySettled), 6066);
        assert_eq!(error_code(OracleError::MaxMultiplierExceeded), 6067);
        assert_eq!(error_code(OracleError::NothingToClaim), 6068);
        assert_eq!(error_code(OracleError::InsufficientReserve), 6069);
        assert_eq!(error_code(OracleError::UtilizationCapExceeded), 6070);
        assert_eq!(error_code(OracleError::StrategyInactive), 6071);
        assert_eq!(error_code(OracleError::StrategyAmountTooSmall), 6072);
        assert_eq!(error_code(OracleError::InvalidExternalAccount), 6073);
        assert_eq!(error_code(OracleError::InvalidExternalState), 6074);
        assert_eq!(error_code(OracleError::InsufficientStrategyBalance), 6075);
        assert_eq!(error_code(OracleError::PriceDeviationTooLarge), 6076);
        assert_eq!(error_code(OracleError::PriceFeedStale), 6077);
        assert_eq!(error_code(OracleError::InvalidMerkleLeafVersion), 6078);
        assert_eq!(error_code(OracleError::MultiplierBelowMinimum), 6079);
        assert_eq!(error_code(OracleError::NavDecreaseNotAllowed), 6080);
        assert_eq!(error_code(OracleError::NavBelowMinimum), 6081);
        assert_eq!(error_code(OracleError::NavAboveMaximum), 6082);
        assert_eq!(error_code(OracleError::ClaimYieldDeprecated), 6083);
    }

    #[test]
    fn exported_constants_match() {
        assert_eq!(
            ORACLE_ERROR_NO_REWARDS_TO_CLAIM,
            error_code(OracleError::NoRewardsToClaim)
        );
        assert_eq!(
            ORACLE_ERROR_POOL_IS_SHUTDOWN,
            error_code(OracleError::PoolIsShutdown)
        );
        assert_eq!(
            ORACLE_ERROR_CLAIM_EXCEEDS_AVAILABLE,
            error_code(OracleError::ClaimExceedsAvailableRewards)
        );
    }

    /// Verify total variant count matches Anchor enum (84 variants: 0..=83).
    #[test]
    fn last_variant_index() {
        assert_eq!(OracleError::ClaimYieldDeprecated as u32, 83);
    }
}
