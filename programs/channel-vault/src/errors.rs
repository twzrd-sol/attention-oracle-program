//! Error definitions for ChannelVault.

use anchor_lang::prelude::*;

#[error_code]
pub enum VaultError {
    #[msg("Math overflow")]
    MathOverflow,

    #[msg("Unauthorized")]
    Unauthorized,

    #[msg("Invalid mint")]
    InvalidMint,

    #[msg("Invalid token program")]
    InvalidTokenProgram,

    #[msg("Vault is paused")]
    VaultPaused,

    #[msg("Deposit amount too small")]
    DepositTooSmall,

    #[msg("No shares outstanding")]
    NoSharesOutstanding,

    #[msg("Insufficient shares")]
    InsufficientShares,

    #[msg("Slippage exceeded - received less than minimum")]
    SlippageExceeded,

    #[msg("Invalid oracle account")]
    InvalidOracleAccount,

    #[msg("Vault stake not active")]
    VaultStakeNotActive,

    #[msg("Nothing to compound")]
    NothingToCompound,

    #[msg("Withdrawal queue not complete")]
    WithdrawQueueNotComplete,

    #[msg("Withdrawal already completed")]
    WithdrawAlreadyCompleted,

    #[msg("Invalid withdrawal request")]
    InvalidWithdrawRequest,

    #[msg("Insufficient vault balance")]
    InsufficientVaultBalance,

    #[msg("Oracle stake still locked")]
    OracleStakeLocked,

    #[msg("Oracle stake not locked - use standard unstake")]
    OracleStakeNotLocked,

    #[msg("Cannot emergency redeem from pending deposits")]
    CannotEmergencyRedeemPending,

    #[msg("Vault already initialized")]
    VaultAlreadyInitialized,

    #[msg("Invalid channel config")]
    InvalidChannelConfig,

    #[msg("Vault not empty - has shares, deposits, or balance")]
    VaultNotEmpty,

    #[msg("Insufficient buffer and reserve for instant redeem")]
    InsufficientReserve,

    #[msg("Instant redeem only available when Oracle stake is locked")]
    InstantRedeemNotAvailable,

    #[msg("Vault is insolvent - withdrawals exceed assets")]
    VaultInsolvent,
}
