//! wzrd-markets custom program errors.
//!
//! Starter set for Phase 0. New variants are added alongside the IX that raises
//! them (Phase 1-3). Each variant carries the precondition that failed — never
//! a generic "something went wrong." Explicit discriminants mirror wzrd-rails so
//! error codes stay stable across SDK regeneration.

use anchor_lang::prelude::*;

#[error_code]
pub enum MarketsError {
    #[msg("Unauthorized: signer is not the configured admin / resolver.")]
    Unauthorized = 0,

    #[msg("Market is not in a state that permits this operation.")]
    InvalidMarketState = 1,

    #[msg("Market has already been resolved.")]
    MarketAlreadyResolved = 2,

    #[msg("Arithmetic overflow / underflow in curve or accounting math.")]
    MathOverflow = 3,

    #[msg("Swap output fell below the caller's minimum-out slippage bound.")]
    SlippageExceeded = 4,

    #[msg("Operation violates the bounding-phase (cold-start) constraints.")]
    BoundingPhaseViolation = 5,

    // ─── Phase 1: market lifecycle + complete-set rail ───────────────────────
    #[msg("Amount must be greater than zero.")]
    ZeroAmount = 6,

    #[msg("Market tokens (YES/NO mints + vault) have already been initialized.")]
    MarketAlreadyHasTokens = 7,

    #[msg("Market tokens have not been initialized yet; call initialize_market_tokens first.")]
    TokensNotInitialized = 8,

    #[msg("Market is resolved; complete-set mint/redeem is pre-resolution only.")]
    MarketResolved = 9,

    #[msg("Redeemer holds fewer outcome tokens than the requested redeem amount.")]
    InsufficientOutcomeBalance = 10,

    #[msg("market_id is not the next sequential id expected by config.")]
    InvalidMarketId = 11,

    #[msg("resolve_deadline_slot must be strictly greater than the current slot.")]
    DeadlineInPast = 12,

    #[msg("resolution_root must be non-zero (a published attention root is required).")]
    ZeroResolutionRoot = 13,

    #[msg("metric is not a defined MarketMetric discriminant.")]
    InvalidMetric = 14,

    // ─── Phase 2: CPMM pool + liquidity + swap ───────────────────────────────
    #[msg("Pool already exists for this market.")]
    PoolAlreadyExists = 15,

    #[msg("Pool has not been initialized yet; call initialize_pool first.")]
    PoolNotInitialized = 16,

    #[msg(
        "Pool holds fewer output tokens than the swap would pay out (the \
           virtual-liquidity floor shifts price, never payout solvency)."
    )]
    InsufficientPoolLiquidity = 17,

    #[msg(
        "Deposited YES/NO amounts do not match the current pool ratio within \
           the supplied bounds."
    )]
    RatioMismatch = 18,

    #[msg(
        "Liquidity / LP amount resolves to zero — refusing a no-op that would \
           mint or burn nothing."
    )]
    ZeroLiquidity = 19,

    #[msg("Market is resolved; trading is halted (swap/add are pre-resolution only).")]
    MarketTradingHalted = 20,

    // ─── Phase 3: resolution + settlement ────────────────────────────────────
    // Merkle proof verification (conventions v1 §3/§4). Cases 21-25 are the
    // verifier's rejection vocabulary; 21 and "wrong-domain → 22" are the
    // M-04/CH-3 silent-failure kill switches (a wrong-domain proof folds to a
    // root that does not equal the snapshot → InvalidMerkleProof, never silently
    // accepted).
    #[msg("Merkle proof exceeds MARKETS_MAX_PROOF_LEN (rejected before the fold).")]
    ProofTooLong = 21,

    #[msg("Merkle proof does not verify against the market's create-time snapshot root.")]
    InvalidMerkleProof = 22,

    #[msg("Resolution leaf market_id does not match the market being resolved.")]
    LeafMarketMismatch = 23,

    #[msg("Resolution leaf streamer_ref does not match the market's streamer_ref.")]
    LeafStreamerMismatch = 24,

    #[msg("Resolution leaf metric does not match the market's metric.")]
    LeafMetricMismatch = 25,

    // resolve_market lifecycle.
    #[msg("Market is not resolved yet; this operation requires a resolved outcome.")]
    MarketNotResolved = 26,

    #[msg("Resolution deadline has passed; the market is in never-resolved recovery.")]
    ResolutionDeadlinePassed = 27,

    // settle / dispute window.
    #[msg("The dispute / challenge window is still open; settlement is not yet final.")]
    DisputeWindowOpen = 28,

    #[msg("Market resolved INVALID; recover collateral via redeem_complete_set, not settle.")]
    MarketInvalidUseRedeem = 29,

    #[msg("The dispute window has already been extended once; a second extension is refused.")]
    DisputeAlreadyExtended = 30,

    // publish_attention_root.
    #[msg("Publisher is not in the config allow-list.")]
    UnauthorizedPublisher = 31,

    #[msg("Attention root for this window has already been published (one root per window).")]
    WindowAlreadyPublished = 32,

    #[msg("Leaf schema_version does not match MARKETS_RESOLUTION_LEAF_SCHEMA_V1.")]
    InvalidLeafSchemaVersion = 33,

    // resolve_override (multisig).
    #[msg("Fewer than resolver_threshold distinct multisig members signed the override.")]
    MultisigThresholdNotMet = 34,

    #[msg("The override window has closed (override is a pre-settle remedy only).")]
    OverrideWindowClosed = 35,

    #[msg("A resolver-multisig member must not be the admin (resolve/override separation).")]
    MultisigMemberIsAdmin = 36,

    // sweep_residual / close_market.
    #[msg("Outcome-token supply is non-zero; cannot sweep / close while obligations remain.")]
    SupplyNotZero = 37,

    // Allow-list management / config.
    #[msg("Publisher allow-list is full (MAX_PUBLISHERS reached).")]
    PublisherAllowlistFull = 38,

    #[msg("Publisher is already present in the allow-list.")]
    PublisherAlreadyPresent = 39,

    #[msg("Publisher is not present in the allow-list.")]
    PublisherNotFound = 40,

    #[msg("Pubkey::default() is not a valid publisher / member.")]
    InvalidPubkey = 41,

    #[msg("resolver_threshold must be in 1..=N and consistent with the multisig.")]
    InvalidThreshold = 42,

    #[msg("dispute_window_slots must be greater than zero.")]
    ZeroDisputeWindow = 43,

    #[msg("Vault still holds more than the dust threshold; cannot close.")]
    VaultNotDrained = 44,

    #[msg("Account does not match the one recorded on the market (mint/vault mismatch).")]
    AccountMismatch = 45,
}
