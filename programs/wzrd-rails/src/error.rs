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
}
