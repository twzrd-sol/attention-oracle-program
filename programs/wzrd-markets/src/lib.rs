//! wzrd-markets — CPMM outcome-token prediction markets.
//!
//! A constant-product (`x * y = k`) AMM over per-market YES/NO Token-2022
//! outcome tokens, collateralized in fee-exempt USDC, resolved by an in-house
//! allow-listed publisher with a multisig override and settled 1:1 on the
//! winning side. This is a NEW program, separately deployed and audited — it
//! does NOT modify the immutable AO program or wzrd-rails.
//!
//! See `docs/cpmm-outcome-token-build-scope.md` for the full design and the
//! audit findings (H-01 finality snapshot, H-02 in-house publisher, M-04 merkle
//! unification, L-08 fee-exempt collateral) this program implements.
//!
//! Phase 0 (this commit): workspace wiring + vendored constant-product curve
//! math + the full state skeleton + exactly ONE real instruction
//! (`initialize_markets_config`) to prove the program compiles, deploys, and
//! has a working init. No funds-bearing logic.

use anchor_lang::prelude::*;
use anchor_spl::token_2022::{Token2022, ID as TOKEN_2022_PROGRAM_ID};
use anchor_spl::token_interface::{
    self, Burn, Mint as MintInterface, MintTo, TokenAccount as TokenAccountInterface,
    TokenInterface, TransferChecked,
};

pub mod curve;
pub mod error;
pub mod events;
pub mod state;

pub use error::*;
pub use events::*;
pub use state::*;

use curve::{ConstantProductCurve, RoundDirection};

/// Phase 2 swap direction discriminants (passed as a `u8` on the `swap` IX so an
/// out-of-range value round-trips through Borsh without an aborting deserialize —
/// `swap` validates the range explicitly, mirroring `MarketMetric`).
#[non_exhaustive]
pub struct SwapDirection;

impl SwapDirection {
    /// YES in, NO out.
    pub const YES_TO_NO: u8 = 0;
    /// NO in, YES out.
    pub const NO_TO_YES: u8 = 1;
}

/// Declare the pool-PDA signer seeds as locals in the calling scope so they
/// outlive the `CpiContext::new_with_signer` borrow.
///
/// Expands to:
/// ```ignore
/// let $market_bytes = pool.market.to_bytes();
/// let $bump = [pool.bump];
/// let $pool_seeds: &[&[u8]] = &[POOL_SEED, &$market_bytes, &$bump];
/// let $signer: &[&[&[u8]]] = &[$pool_seeds];
/// ```
///
/// The seeds are `[POOL_SEED, market.key(), &[pool.bump]]` — BYTE-IDENTICAL to
/// the `seeds = [POOL_SEED, market.key().as_ref()]` the pool PDA was `init`-ed
/// with (scope §8 non-negotiable), and the bump is read from the stored
/// `pool.bump`. A `macro_rules!` that DECLARES locals (rather than returning
/// owned data) is the only borrow-checker-clean way to keep the `&[&[u8]]`
/// slices valid across the CPI without heap allocation.
macro_rules! pool_signer_seeds {
    ($pool:expr, $market_bytes:ident, $bump:ident, $pool_seeds:ident, $signer:ident) => {
        let $market_bytes = $pool.market.to_bytes();
        let $bump = [$pool.bump];
        let $pool_seeds: &[&[u8]] = &[POOL_SEED, $market_bytes.as_ref(), &$bump];
        let $signer: &[&[&[u8]]] = &[$pool_seeds];
    };
}

// TODO: real program id before deploy. Placeholder keypair generated 2026-06-21
// solely so Phase 0 compiles + deploys to a local validator; it is NOT the
// production program id and MUST be replaced (with a vanity/published keypair)
// before any audit or mainnet deploy.
declare_id!("DKMJTZgk6obi2BfTyxSuB4P2S4mLW2HGwC7SpTtrCkfG");

#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "wzrd-markets",
    project_url: "https://github.com/twzrd-sol/attention-oracle-program",
    contacts: "email:security@twzrd.xyz",
    policy: "https://github.com/twzrd-sol/attention-oracle-program/blob/main/SECURITY.md",
    preferred_languages: "en",
    source_code: "https://github.com/twzrd-sol/attention-oracle-program"
}

#[program]
pub mod wzrd_markets {
    use super::*;

    /// Initialize the program's global config. One-time, per deployment.
    ///
    /// The signer becomes the admin. Stores the (fee-exempt) USDC collateral
    /// mint and the resolver multisig; the publisher allow-list starts empty
    /// (populated later by a Phase 3 admin instruction).
    ///
    /// This is the ONLY funds-touching-adjacent instruction in Phase 0 and it
    /// moves no funds — it exists to prove the program loads and inits. Market,
    /// pool, swap, and resolution instructions are Phase 1-3 (see the roadmap
    /// below).
    ///
    /// Preconditions:
    ///   - Config PDA must not already exist (the `init` constraint enforces).
    ///   - Caller signs and pays rent.
    ///
    /// Postconditions:
    ///   - MarketsConfig { admin = signer, usdc_mint, resolver_multisig,
    ///     publisher_allowlist = [], bump }.
    ///
    /// Like wzrd-rails' `initialize_config`, this intentionally does NOT verify
    /// that `usdc_mint` is a real mint or that `resolver_multisig` is a valid
    /// multisig — those are trust-the-admin parameters re-checked at the point
    /// of use in later phases.
    pub fn initialize_markets_config(
        ctx: Context<InitializeMarketsConfig>,
        usdc_mint: Pubkey,
        resolver_multisig: Pubkey,
    ) -> Result<()> {
        let slot = Clock::get()?.slot;
        let config_key = ctx.accounts.config.key();
        let admin = ctx.accounts.admin.key();

        let config = &mut ctx.accounts.config;
        config.bump = ctx.bumps.config;
        config.admin = admin;
        config.usdc_mint = usdc_mint;
        config.resolver_multisig = resolver_multisig;
        config.publisher_allowlist = Vec::new();
        config.next_market_id = 0;
        config._reserved = [0u8; 56];

        emit!(MarketsConfigInitialized {
            config: config_key,
            admin,
            usdc_mint,
            resolver_multisig,
            slot,
        });
        Ok(())
    }

    /// Phase 1 — open a market over a streamer's future attention metric.
    ///
    /// Admin-gated (Phase-1 trust choice: markets are curated; permissionless
    /// creation is a later decision). Snapshots the resolution root + seq AT
    /// CREATE-TIME (audit H-01) so the finality anchor cannot drift after the
    /// market opens. The caller-supplied `market_id` must equal
    /// `config.next_market_id` (sequential, gap-free → collision-free PDA seed).
    ///
    /// Preconditions:
    ///   - signer == config.admin (Unauthorized otherwise).
    ///   - market_id == config.next_market_id (InvalidMarketId otherwise).
    ///   - metric is a defined MarketMetric (InvalidMetric otherwise).
    ///   - resolution_root != [0; 32] (ZeroResolutionRoot otherwise).
    ///   - resolve_deadline_slot > current slot (DeadlineInPast otherwise).
    ///   - Market PDA does not already exist (the `init` constraint enforces).
    ///
    /// Postconditions:
    ///   - Market PDA fully populated; token fields default until
    ///     `initialize_market_tokens`. `config.next_market_id` incremented.
    #[allow(clippy::too_many_arguments)]
    pub fn create_market(
        ctx: Context<CreateMarket>,
        market_id: u64,
        streamer_ref: [u8; 32],
        metric: u8,
        target: u64,
        resolution_root: [u8; 32],
        resolution_root_seq: u64,
        resolve_deadline_slot: u64,
        dispute_window_slots: u64,
    ) -> Result<()> {
        let clock_slot = Clock::get()?.slot;

        // Admin gate (Phase-1 curated markets).
        require_keys_eq!(
            ctx.accounts.admin.key(),
            ctx.accounts.config.admin,
            MarketsError::Unauthorized
        );
        // Sequential, gap-free id → the [MARKET_SEED, market_id] PDA is unique.
        require_eq!(
            market_id,
            ctx.accounts.config.next_market_id,
            MarketsError::InvalidMarketId
        );
        require!(MarketMetric::is_valid(metric), MarketsError::InvalidMetric);
        // H-01: bind to a root that already exists; require a non-zero snapshot.
        require!(
            resolution_root != [0u8; 32],
            MarketsError::ZeroResolutionRoot
        );
        require!(
            resolve_deadline_slot > clock_slot,
            MarketsError::DeadlineInPast
        );

        let creator = ctx.accounts.admin.key();
        let market_key = ctx.accounts.market.key();

        let market = &mut ctx.accounts.market;
        market.bump = ctx.bumps.market;
        market.version = Market::VERSION;
        market.market_id = market_id;
        market.creator = creator;
        market.streamer_ref = streamer_ref;
        market.metric = metric;
        market.target = target;
        market.resolution_root = resolution_root; // H-01 snapshot
        market.resolution_root_seq = resolution_root_seq; // H-01 snapshot
        market.created_slot = clock_slot;
        market.resolve_deadline_slot = resolve_deadline_slot;
        market.resolved = false;
        market.outcome = false;
        market.settled_supply = 0;
        market.dispute_window_slots = dispute_window_slots;
        market.yes_mint = Pubkey::default();
        market.no_mint = Pubkey::default();
        market.vault = Pubkey::default();
        market.tokens_initialized = false;
        market._reserved = [0u8; 64];

        // Advance the monotonic counter for the next market.
        let config = &mut ctx.accounts.config;
        config.next_market_id = config
            .next_market_id
            .checked_add(1)
            .ok_or(MarketsError::MathOverflow)?;

        emit!(MarketCreated {
            market: market_key,
            market_id,
            creator,
            streamer_ref,
            metric,
            target,
            resolution_root,
            resolution_root_seq,
            resolve_deadline_slot,
            slot: clock_slot,
        });
        Ok(())
    }

    /// Phase 1 — create the per-market YES + NO Token-2022 mints (fee-free, 6
    /// decimals to match USDC) and the USDC collateral vault, all PDA-owned.
    ///
    /// Outcome mints use Anchor `init` with the `mint::*` constraints (no
    /// Token-2022 extensions in Phase 1, so no manual CPI is needed — minimal).
    /// The mint authority is the per-market PDA `[MINT_AUTH_SEED, market_id]`,
    /// which signs `mint_to` / `burn` in the complete-set rail. The vault is a
    /// USDC token account owned by the Market PDA (the vault authority).
    ///
    /// Preconditions:
    ///   - Market exists (created by `create_market`).
    ///   - !market.tokens_initialized (MarketAlreadyHasTokens otherwise; the
    ///     `init` of the mints/vault also enforces single-creation).
    ///
    /// Postconditions:
    ///   - market.{yes_mint,no_mint,vault} set; market.tokens_initialized = true.
    pub fn initialize_market_tokens(ctx: Context<InitializeMarketTokens>) -> Result<()> {
        let slot = Clock::get()?.slot;
        require!(
            !ctx.accounts.market.tokens_initialized,
            MarketsError::MarketAlreadyHasTokens
        );

        let market_key = ctx.accounts.market.key();
        let market_id = ctx.accounts.market.market_id;
        let yes_mint = ctx.accounts.yes_mint.key();
        let no_mint = ctx.accounts.no_mint.key();
        let vault = ctx.accounts.vault.key();
        let mint_authority = ctx.accounts.mint_authority.key();

        let market = &mut ctx.accounts.market;
        market.yes_mint = yes_mint;
        market.no_mint = no_mint;
        market.vault = vault;
        market.tokens_initialized = true;

        emit!(TokensInitialized {
            market: market_key,
            market_id,
            yes_mint,
            no_mint,
            vault,
            mint_authority,
            slot,
        });
        Ok(())
    }

    /// Phase 1 — the fixed-par complete-set rail: deposit N USDC → mint exactly
    /// N YES AND N NO. Pre-resolution only.
    ///
    /// AUDIT MR-1 (ported, verified sound): snapshot `vault_before` → transfer
    /// USDC in → `vault.reload()` → `net_received = vault_after - vault_before` →
    /// mint EXACTLY `net_received` of each outcome. USDC is fee-exempt so
    /// `net_received == amount`, but the before/after sampling is kept as the
    /// defense-in-depth the audit endorsed (it costs nothing and protects any
    /// future collateral change). It is NOT shortcut to `amount`.
    ///
    /// Invariant preserved: `vault.amount == yes_mint.supply == no_mint.supply`.
    pub fn mint_complete_set(ctx: Context<MintCompleteSet>, amount: u64) -> Result<()> {
        require!(
            ctx.accounts.market.tokens_initialized,
            MarketsError::TokensNotInitialized
        );
        require!(!ctx.accounts.market.resolved, MarketsError::MarketResolved);
        require!(amount > 0, MarketsError::ZeroAmount);

        // MR-1: snapshot the vault BEFORE the transfer.
        let vault_before = ctx.accounts.vault.amount;

        // Transfer USDC depositor → vault. USDC is fee-exempt; depositor signs.
        token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.usdc_token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.depositor_usdc.to_account_info(),
                    mint: ctx.accounts.usdc_mint.to_account_info(),
                    to: ctx.accounts.vault.to_account_info(),
                    authority: ctx.accounts.depositor.to_account_info(),
                },
            ),
            amount,
            ctx.accounts.usdc_mint.decimals,
        )?;

        // MR-1: reload and compute exactly what landed.
        ctx.accounts.vault.reload()?;
        let vault_after = ctx.accounts.vault.amount;
        let net_received = vault_after
            .checked_sub(vault_before)
            .ok_or(MarketsError::MathOverflow)?;
        require!(net_received > 0, MarketsError::ZeroAmount);

        // Mint exactly net_received YES AND NO; mint-authority PDA signs.
        let market_id_bytes = ctx.accounts.market.market_id.to_le_bytes();
        let mint_auth_seeds: &[&[u8]] = &[
            MINT_AUTH_SEED,
            market_id_bytes.as_ref(),
            &[ctx.bumps.mint_authority],
        ];
        let signer_seeds: &[&[&[u8]]] = &[mint_auth_seeds];

        token_interface::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.outcome_token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.yes_mint.to_account_info(),
                    to: ctx.accounts.depositor_yes.to_account_info(),
                    authority: ctx.accounts.mint_authority.to_account_info(),
                },
                signer_seeds,
            ),
            net_received,
        )?;
        token_interface::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.outcome_token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.no_mint.to_account_info(),
                    to: ctx.accounts.depositor_no.to_account_info(),
                    authority: ctx.accounts.mint_authority.to_account_info(),
                },
                signer_seeds,
            ),
            net_received,
        )?;

        emit!(CompleteSetMinted {
            market: ctx.accounts.market.key(),
            market_id: ctx.accounts.market.market_id,
            depositor: ctx.accounts.depositor.key(),
            deposit_amount: amount,
            net_amount: net_received,
        });
        Ok(())
    }

    /// Phase 1 — the inverse rail: burn N YES AND N NO → return N USDC.
    /// Pre-resolution only (post-resolution settlement is Phase 3).
    ///
    /// Burns `amount` from each outcome (redeemer is the authority on their own
    /// token accounts), then transfers `amount` USDC out of the vault, signed by
    /// the Market PDA (the vault authority). Solvency is preserved: equal YES+NO
    /// burned, equal USDC returned, so `vault == yes_supply == no_supply` holds.
    pub fn redeem_complete_set(ctx: Context<RedeemCompleteSet>, amount: u64) -> Result<()> {
        require!(
            ctx.accounts.market.tokens_initialized,
            MarketsError::TokensNotInitialized
        );
        require!(!ctx.accounts.market.resolved, MarketsError::MarketResolved);
        require!(amount > 0, MarketsError::ZeroAmount);

        // Clean pre-check (the burn CPI would also fail, but this gives a typed
        // error instead of an opaque token-program code).
        require!(
            ctx.accounts.redeemer_yes.amount >= amount && ctx.accounts.redeemer_no.amount >= amount,
            MarketsError::InsufficientOutcomeBalance
        );

        // Burn `amount` YES AND NO from the redeemer (they sign for their ATAs).
        token_interface::burn(
            CpiContext::new(
                ctx.accounts.outcome_token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.yes_mint.to_account_info(),
                    from: ctx.accounts.redeemer_yes.to_account_info(),
                    authority: ctx.accounts.redeemer.to_account_info(),
                },
            ),
            amount,
        )?;
        token_interface::burn(
            CpiContext::new(
                ctx.accounts.outcome_token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.no_mint.to_account_info(),
                    from: ctx.accounts.redeemer_no.to_account_info(),
                    authority: ctx.accounts.redeemer.to_account_info(),
                },
            ),
            amount,
        )?;

        // Transfer `amount` USDC vault → redeemer; the Market PDA signs.
        let market_id_bytes = ctx.accounts.market.market_id.to_le_bytes();
        let market_bump = ctx.accounts.market.bump;
        let market_seeds: &[&[u8]] = &[MARKET_SEED, market_id_bytes.as_ref(), &[market_bump]];
        let signer_seeds: &[&[&[u8]]] = &[market_seeds];

        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.usdc_token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.vault.to_account_info(),
                    mint: ctx.accounts.usdc_mint.to_account_info(),
                    to: ctx.accounts.redeemer_usdc.to_account_info(),
                    authority: ctx.accounts.market.to_account_info(),
                },
                signer_seeds,
            ),
            amount,
            ctx.accounts.usdc_mint.decimals,
        )?;

        emit!(CompleteSetRedeemed {
            market: ctx.accounts.market.key(),
            market_id: ctx.accounts.market.market_id,
            redeemer: ctx.accounts.redeemer.key(),
            amount,
        });
        Ok(())
    }

    /// Phase 2 — create the constant-product YES/NO pool for a market.
    ///
    /// Creates the Pool PDA `[POOL_SEED, market.key()]`, the LP mint
    /// `[LP_MINT_SEED, market.key()]` (Token-2022, pool PDA = mint authority), and
    /// the pool's YES + NO token accounts (owned by the pool PDA, for the market's
    /// recorded yes_mint / no_mint). Seeds the cold-start bounding-phase virtual
    /// liquidity `V` so the first trades price against a sane ~0.5 baseline
    /// instead of dividing by zero.
    ///
    /// `virtual_liquidity` (V) is VIRTUAL — the pool never holds V of any token.
    /// It is added to the curve INPUTS only (scope §2 / §4). The pool's real
    /// token-account balances are the hard ceiling on every transfer-out; V shifts
    /// the price, never the payout solvency.
    ///
    /// Preconditions:
    ///   - Market exists and `tokens_initialized` (TokensNotInitialized otherwise).
    ///   - Pool PDA does not already exist (the `init` constraint enforces;
    ///     re-init surfaces as the account-already-in-use abort. PoolAlreadyExists
    ///     is retained for explicitness / future non-`init` paths).
    ///
    /// Postconditions:
    ///   - Pool { bounding_phase_active = true, virtual_liquidity = V,
    ///     yes_reserve = 0, no_reserve = 0, lp_supply = 0, lp_mint, bump }.
    pub fn initialize_pool(ctx: Context<InitializePool>, virtual_liquidity: u64) -> Result<()> {
        let slot = Clock::get()?.slot;
        require!(
            ctx.accounts.market.tokens_initialized,
            MarketsError::TokensNotInitialized
        );

        let market_key = ctx.accounts.market.key();
        let pool_key = ctx.accounts.pool.key();
        let lp_mint = ctx.accounts.lp_mint.key();

        let pool = &mut ctx.accounts.pool;
        pool.bump = ctx.bumps.pool;
        pool.market = market_key;
        pool.yes_reserve = 0;
        pool.no_reserve = 0;
        pool.lp_mint = lp_mint;
        pool.lp_supply = 0;
        pool.bounding_phase_active = true;
        pool.virtual_liquidity = virtual_liquidity;
        pool._reserved = [0u8; 32];

        emit!(PoolInitialized {
            market: market_key,
            pool: pool_key,
            lp_mint,
            yes_reserve: 0,
            no_reserve: 0,
            virtual_liquidity,
            slot,
        });
        Ok(())
    }

    /// Phase 2 — provide YES + NO liquidity, receive LP tokens.
    ///
    /// First LP sets the ratio (deposits exactly `max_yes` + `max_no`, mints
    /// `sqrt(max_yes * max_no)` LP — the geometric-mean initial supply, matching
    /// Uniswap-style first-mint). Subsequent LPs must match the current
    /// `yes_reserve : no_reserve` ratio: the handler computes the required NO for
    /// the supplied YES (and vice versa), takes the feasible side, transfers in
    /// the matched amounts, and mints LP proportional to the share added
    /// (`lp_minted = lp_supply * yes_in / yes_reserve`).
    ///
    /// `min_lp` is the slippage / ratio guard (RatioMismatch / ZeroLiquidity if
    /// the matched deposit mints fewer LP than `min_lp` or rounds to zero).
    ///
    /// Bounding-phase transition: once this add brings BOTH real reserves
    /// `>= virtual_liquidity`, `bounding_phase_active` flips to false (real
    /// liquidity now dominates the virtual floor; scope §2 threshold).
    ///
    /// Trading is halted post-resolution (MarketTradingHalted).
    pub fn add_liquidity(
        ctx: Context<AddLiquidity>,
        max_yes: u64,
        max_no: u64,
        min_lp: u64,
    ) -> Result<()> {
        require!(
            !ctx.accounts.market.resolved,
            MarketsError::MarketTradingHalted
        );
        require!(max_yes > 0 && max_no > 0, MarketsError::ZeroAmount);

        let yes_reserve = ctx.accounts.pool.yes_reserve;
        let no_reserve = ctx.accounts.pool.no_reserve;
        let lp_supply = ctx.accounts.pool.lp_supply;

        // Compute the matched (yes_in, no_in) deposit and the LP to mint.
        let (yes_in, no_in, lp_to_mint) =
            compute_add_liquidity(max_yes, max_no, yes_reserve, no_reserve, lp_supply)?;

        require!(lp_to_mint > 0, MarketsError::ZeroLiquidity);
        require!(lp_to_mint >= min_lp, MarketsError::RatioMismatch);

        // ── Transfer YES + NO from the provider into the pool's reserves ──
        // (provider signs for their own source ATAs).
        token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.outcome_token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.provider_yes.to_account_info(),
                    mint: ctx.accounts.yes_mint.to_account_info(),
                    to: ctx.accounts.pool_yes.to_account_info(),
                    authority: ctx.accounts.provider.to_account_info(),
                },
            ),
            yes_in,
            ctx.accounts.yes_mint.decimals,
        )?;
        token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.outcome_token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.provider_no.to_account_info(),
                    mint: ctx.accounts.no_mint.to_account_info(),
                    to: ctx.accounts.pool_no.to_account_info(),
                    authority: ctx.accounts.provider.to_account_info(),
                },
            ),
            no_in,
            ctx.accounts.no_mint.decimals,
        )?;

        // ── Mint LP to the provider; the pool PDA is the LP mint authority ──
        pool_signer_seeds!(
            ctx.accounts.pool,
            pool_market_bytes,
            pool_bump,
            pool_seeds,
            pool_signer
        );
        token_interface::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.lp_token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.lp_mint.to_account_info(),
                    to: ctx.accounts.provider_lp.to_account_info(),
                    authority: ctx.accounts.pool.to_account_info(),
                },
                pool_signer,
            ),
            lp_to_mint,
        )?;

        // ── Update reserves + supply from the REAL deposit ──
        let pool = &mut ctx.accounts.pool;
        pool.yes_reserve = yes_reserve
            .checked_add(yes_in)
            .ok_or(MarketsError::MathOverflow)?;
        pool.no_reserve = no_reserve
            .checked_add(no_in)
            .ok_or(MarketsError::MathOverflow)?;
        pool.lp_supply = lp_supply
            .checked_add(lp_to_mint)
            .ok_or(MarketsError::MathOverflow)?;

        // Bounding-phase transition: real liquidity now dominates the virtual
        // floor once BOTH real reserves >= V. The floor is no longer needed.
        if pool.bounding_phase_active
            && pool.yes_reserve >= pool.virtual_liquidity
            && pool.no_reserve >= pool.virtual_liquidity
        {
            pool.bounding_phase_active = false;
        }

        emit!(LiquidityAdded {
            pool: pool.key(),
            provider: ctx.accounts.provider.key(),
            yes_in,
            no_in,
            lp_minted: lp_to_mint,
            yes_reserve: pool.yes_reserve,
            no_reserve: pool.no_reserve,
            lp_supply: pool.lp_supply,
            bounding_phase_active: pool.bounding_phase_active,
        });
        Ok(())
    }

    /// Phase 2 — burn LP tokens, withdraw YES + NO pro-rata.
    ///
    /// Uses `lp_tokens_to_trading_tokens(lp_amount, lp_supply, yes_reserve,
    /// no_reserve, Floor)` — FLOOR rounding so the LP receives `<=` their exact
    /// pro-rata share and the pool keeps the dust (never overpays; this is what
    /// keeps `k` from decreasing on withdraw). Burns the LP (the holder is the
    /// authority on their own LP account), transfers YES + NO out (the pool PDA
    /// signs), and updates reserves / supply.
    ///
    /// `min_yes` / `min_no` are the slippage guards (RatioMismatch if either
    /// floored output falls below its bound).
    ///
    /// Remove is allowed post-resolution so LPs can always exit (scope §6 — only
    /// swap/add halt at resolution; withdrawals do not trap liquidity).
    pub fn remove_liquidity(
        ctx: Context<RemoveLiquidity>,
        lp_amount: u64,
        min_yes: u64,
        min_no: u64,
    ) -> Result<()> {
        require!(lp_amount > 0, MarketsError::ZeroAmount);

        let yes_reserve = ctx.accounts.pool.yes_reserve;
        let no_reserve = ctx.accounts.pool.no_reserve;
        let lp_supply = ctx.accounts.pool.lp_supply;

        require!(lp_supply > 0, MarketsError::ZeroLiquidity);
        require!(
            lp_amount <= lp_supply,
            MarketsError::InsufficientPoolLiquidity
        );

        // FLOOR rounding: LP gets <= pro-rata, pool keeps dust (curve invariant).
        let (yes_out, no_out) =
            compute_remove_liquidity(lp_amount, lp_supply, yes_reserve, no_reserve)?;

        require!(yes_out > 0 || no_out > 0, MarketsError::ZeroLiquidity);
        require!(
            yes_out >= min_yes && no_out >= min_no,
            MarketsError::RatioMismatch
        );

        // ── Burn the LP from the holder (they sign for their own LP account) ──
        token_interface::burn(
            CpiContext::new(
                ctx.accounts.lp_token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.lp_mint.to_account_info(),
                    from: ctx.accounts.provider_lp.to_account_info(),
                    authority: ctx.accounts.provider.to_account_info(),
                },
            ),
            lp_amount,
        )?;

        // ── Transfer YES + NO out of the pool; the pool PDA signs ──
        pool_signer_seeds!(
            ctx.accounts.pool,
            pool_market_bytes,
            pool_bump,
            pool_seeds,
            pool_signer
        );
        if yes_out > 0 {
            token_interface::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.outcome_token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.pool_yes.to_account_info(),
                        mint: ctx.accounts.yes_mint.to_account_info(),
                        to: ctx.accounts.provider_yes.to_account_info(),
                        authority: ctx.accounts.pool.to_account_info(),
                    },
                    pool_signer,
                ),
                yes_out,
                ctx.accounts.yes_mint.decimals,
            )?;
        }
        if no_out > 0 {
            token_interface::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.outcome_token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.pool_no.to_account_info(),
                        mint: ctx.accounts.no_mint.to_account_info(),
                        to: ctx.accounts.provider_no.to_account_info(),
                        authority: ctx.accounts.pool.to_account_info(),
                    },
                    pool_signer,
                ),
                no_out,
                ctx.accounts.no_mint.decimals,
            )?;
        }

        // ── Update reserves + supply ──
        let pool = &mut ctx.accounts.pool;
        pool.yes_reserve = yes_reserve
            .checked_sub(yes_out)
            .ok_or(MarketsError::MathOverflow)?;
        pool.no_reserve = no_reserve
            .checked_sub(no_out)
            .ok_or(MarketsError::MathOverflow)?;
        pool.lp_supply = lp_supply
            .checked_sub(lp_amount)
            .ok_or(MarketsError::MathOverflow)?;

        emit!(LiquidityRemoved {
            pool: pool.key(),
            provider: ctx.accounts.provider.key(),
            lp_burned: lp_amount,
            yes_out,
            no_out,
            yes_reserve: pool.yes_reserve,
            no_reserve: pool.no_reserve,
            lp_supply: pool.lp_supply,
        });
        Ok(())
    }

    /// Phase 2 — the moving-odds primitive: swap YES <-> NO against the curve.
    ///
    /// `direction`: `0 = YesToNo` (YES in, NO out), `1 = NoToYes` (NO in, YES out).
    ///
    /// `amount_out = swap_base_input_without_fees(amount_in, effective_input,
    /// effective_output)` where `effective_* = real_reserve (+ V if
    /// bounding_phase_active)`. The virtual floor V shifts the PRICE (so the first
    /// trade on a thin pool prices near 0.5 instead of dividing by zero) — it does
    /// NOT add payable tokens.
    ///
    /// THE PHANTOM-PAYOUT GUARD: `amount_out` is checked against the pool's REAL
    /// output token-account balance and the swap REVERTS (InsufficientPoolLiquidity)
    /// if it would pay more than the pool holds. This is the hard solvency ceiling
    /// the virtual floor can never breach (scope §4 / §8, test #4).
    ///
    /// Slippage: `amount_out >= min_amount_out` (SlippageExceeded otherwise).
    /// Fee = 0 for v1. Trading halts post-resolution (MarketTradingHalted).
    pub fn swap(
        ctx: Context<Swap>,
        amount_in: u64,
        min_amount_out: u64,
        direction: u8,
    ) -> Result<()> {
        require!(
            !ctx.accounts.market.resolved,
            MarketsError::MarketTradingHalted
        );
        require!(amount_in > 0, MarketsError::ZeroAmount);
        require!(
            direction == SwapDirection::YES_TO_NO || direction == SwapDirection::NO_TO_YES,
            MarketsError::InvalidMarketState
        );

        let bounding = ctx.accounts.pool.bounding_phase_active;
        let virtual_liquidity = ctx.accounts.pool.virtual_liquidity;
        let yes_reserve = ctx.accounts.pool.yes_reserve;
        let no_reserve = ctx.accounts.pool.no_reserve;

        // Effective reserves include the virtual floor ONLY while bounding.
        let v: u128 = if bounding {
            virtual_liquidity as u128
        } else {
            0
        };
        let (eff_in, eff_out, real_out) = match direction {
            SwapDirection::YES_TO_NO => (
                (yes_reserve as u128) + v,
                (no_reserve as u128) + v,
                no_reserve,
            ),
            // NO_TO_YES
            _ => (
                (no_reserve as u128) + v,
                (yes_reserve as u128) + v,
                yes_reserve,
            ),
        };

        // Curve math (hardened to Option — None maps to MathOverflow).
        let amount_out_u128 =
            ConstantProductCurve::swap_base_input_without_fees(amount_in as u128, eff_in, eff_out)
                .ok_or(MarketsError::MathOverflow)?;
        let amount_out: u64 =
            u64::try_from(amount_out_u128).map_err(|_| MarketsError::MathOverflow)?;

        // Slippage guard.
        require!(amount_out >= min_amount_out, MarketsError::SlippageExceeded);

        // ── THE PHANTOM-PAYOUT GUARD ──
        // The pool can only pay what it actually holds. The virtual floor shifted
        // the price calculation above, but real_out (the real output reserve) is
        // the hard ceiling. If the calculated payout exceeds it, REVERT — never
        // pay phantom tokens. This is what makes the arb loop coherent.
        require!(
            amount_out <= real_out,
            MarketsError::InsufficientPoolLiquidity
        );

        // ── Pull amount_in INTO the input reserve (trader signs) ──
        // ── Push amount_out OUT of the output reserve (pool PDA signs) ──
        pool_signer_seeds!(
            ctx.accounts.pool,
            pool_market_bytes,
            pool_bump,
            pool_seeds,
            pool_signer
        );
        match direction {
            SwapDirection::YES_TO_NO => {
                // YES in
                token_interface::transfer_checked(
                    CpiContext::new(
                        ctx.accounts.outcome_token_program.to_account_info(),
                        TransferChecked {
                            from: ctx.accounts.trader_yes.to_account_info(),
                            mint: ctx.accounts.yes_mint.to_account_info(),
                            to: ctx.accounts.pool_yes.to_account_info(),
                            authority: ctx.accounts.trader.to_account_info(),
                        },
                    ),
                    amount_in,
                    ctx.accounts.yes_mint.decimals,
                )?;
                // NO out
                token_interface::transfer_checked(
                    CpiContext::new_with_signer(
                        ctx.accounts.outcome_token_program.to_account_info(),
                        TransferChecked {
                            from: ctx.accounts.pool_no.to_account_info(),
                            mint: ctx.accounts.no_mint.to_account_info(),
                            to: ctx.accounts.trader_no.to_account_info(),
                            authority: ctx.accounts.pool.to_account_info(),
                        },
                        pool_signer,
                    ),
                    amount_out,
                    ctx.accounts.no_mint.decimals,
                )?;
            }
            _ => {
                // NO_TO_YES: NO in
                token_interface::transfer_checked(
                    CpiContext::new(
                        ctx.accounts.outcome_token_program.to_account_info(),
                        TransferChecked {
                            from: ctx.accounts.trader_no.to_account_info(),
                            mint: ctx.accounts.no_mint.to_account_info(),
                            to: ctx.accounts.pool_no.to_account_info(),
                            authority: ctx.accounts.trader.to_account_info(),
                        },
                    ),
                    amount_in,
                    ctx.accounts.no_mint.decimals,
                )?;
                // YES out
                token_interface::transfer_checked(
                    CpiContext::new_with_signer(
                        ctx.accounts.outcome_token_program.to_account_info(),
                        TransferChecked {
                            from: ctx.accounts.pool_yes.to_account_info(),
                            mint: ctx.accounts.yes_mint.to_account_info(),
                            to: ctx.accounts.trader_yes.to_account_info(),
                            authority: ctx.accounts.pool.to_account_info(),
                        },
                        pool_signer,
                    ),
                    amount_out,
                    ctx.accounts.yes_mint.decimals,
                )?;
            }
        }

        // ── Update the REAL reserves (input += amount_in, output -= amount_out) ──
        let pool = &mut ctx.accounts.pool;
        match direction {
            SwapDirection::YES_TO_NO => {
                pool.yes_reserve = yes_reserve
                    .checked_add(amount_in)
                    .ok_or(MarketsError::MathOverflow)?;
                pool.no_reserve = no_reserve
                    .checked_sub(amount_out)
                    .ok_or(MarketsError::MathOverflow)?;
            }
            _ => {
                pool.no_reserve = no_reserve
                    .checked_add(amount_in)
                    .ok_or(MarketsError::MathOverflow)?;
                pool.yes_reserve = yes_reserve
                    .checked_sub(amount_out)
                    .ok_or(MarketsError::MathOverflow)?;
            }
        }

        // Implied price of NO (bps) over the REAL reserves: in the CPMM-prediction
        // model an outcome's price is the OPPOSITE reserve over the total.
        let implied_no_price_bps = implied_no_price_bps(pool.yes_reserve, pool.no_reserve);

        emit!(Swapped {
            pool: pool.key(),
            trader: ctx.accounts.trader.key(),
            direction,
            amount_in,
            amount_out,
            yes_reserve: pool.yes_reserve,
            no_reserve: pool.no_reserve,
            implied_no_price_bps,
        });
        Ok(())
    }

    // Phase 3 — resolution + settlement (in-house publisher, audit H-01/H-02/M-04/M-05):
    //   - publish_attention_root(ctx, args)    // in-house allow-listed publisher (advances AttentionRootConfig.last_published_seq); reuse rails listen-payout pattern
    //   - resolve_market(ctx, proof)           // verify proof vs the CREATE-TIME-snapshotted root; + dispute window before final
    //   - settle(ctx)                          // burn winning outcome token 1:1 for USDC; preserve solvency invariant
    //   - resolve_override(ctx, outcome)       // multisig-gated fallback for disputed/missing data
    //   - sweep_residual(ctx) / close_market(ctx)  // admin-gated, supply==0 guards
}

// ─── Phase 2 pure helpers ─────────────────────────────────────────────────────
//
// Extracted to `#[inline(never)]` free functions so their working set stays OFF
// the IX-handler stack frames (SBF 4096-byte-per-frame limit; CLAUDE.md SBF
// constraint). All math is checked — no panics, no prod unwraps.

/// Integer square root via Newton's method (used for the first-LP geometric-mean
/// initial supply `sqrt(yes * no)`, the Uniswap-style bootstrap mint).
///
/// Exact floor sqrt for all `u128`. Converges in O(log bits) iterations.
#[inline(never)]
fn integer_sqrt(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    // Initial guess: 2^(ceil(bits/2)). `n.ilog2()` is the index of the MSB.
    let mut x = 1u128 << (n.ilog2() / 2 + 1);
    loop {
        // x_next = (x + n / x) / 2; monotonically non-increasing to floor(sqrt).
        let x_next = (x + n / x) / 2;
        if x_next >= x {
            return x;
        }
        x = x_next;
    }
}

/// Compute the matched `(yes_in, no_in, lp_to_mint)` for an `add_liquidity`.
///
/// - **First LP** (`lp_supply == 0`): deposit exactly `(max_yes, max_no)` and mint
///   `sqrt(max_yes * max_no)` LP (geometric-mean initial supply). The first LP
///   freely sets the ratio.
/// - **Subsequent LP**: match the current `yes_reserve : no_reserve` ratio.
///   Compute the NO required for `max_yes` (`required_no = max_yes * no_reserve /
///   yes_reserve`); if that fits within `max_no`, deposit `(max_yes,
///   required_no)`. Otherwise the NO side is scarcer — compute the YES required
///   for `max_no` and deposit `(required_yes, max_no)`. LP minted is proportional
///   to the share added: `lp_to_mint = lp_supply * yes_in / yes_reserve`.
///
/// Returns `MathOverflow` on any checked-arithmetic failure.
#[inline(never)]
fn compute_add_liquidity(
    max_yes: u64,
    max_no: u64,
    yes_reserve: u64,
    no_reserve: u64,
    lp_supply: u64,
) -> Result<(u64, u64, u64)> {
    // First LP: free ratio, geometric-mean initial supply.
    if lp_supply == 0 || yes_reserve == 0 || no_reserve == 0 {
        let product = (max_yes as u128)
            .checked_mul(max_no as u128)
            .ok_or(MarketsError::MathOverflow)?;
        let lp = integer_sqrt(product);
        let lp_u64 = u64::try_from(lp).map_err(|_| MarketsError::MathOverflow)?;
        return Ok((max_yes, max_no, lp_u64));
    }

    // Subsequent LP: match the current ratio, bounded by the scarcer side.
    let yes_reserve_u = yes_reserve as u128;
    let no_reserve_u = no_reserve as u128;

    // NO required to pair with all of max_yes at the current ratio.
    let required_no = (max_yes as u128)
        .checked_mul(no_reserve_u)
        .ok_or(MarketsError::MathOverflow)?
        / yes_reserve_u;

    let (yes_in_u, no_in_u) = if required_no <= max_no as u128 {
        // YES side is the binding constraint; take all of max_yes.
        (max_yes as u128, required_no)
    } else {
        // NO side is scarcer; take all of max_no and the YES it pairs with.
        let required_yes = (max_no as u128)
            .checked_mul(yes_reserve_u)
            .ok_or(MarketsError::MathOverflow)?
            / no_reserve_u;
        (required_yes, max_no as u128)
    };

    // LP proportional to the YES share added: lp_supply * yes_in / yes_reserve.
    // (Using the YES side; the NO side is matched to the same ratio.)
    let lp_to_mint_u = (lp_supply as u128)
        .checked_mul(yes_in_u)
        .ok_or(MarketsError::MathOverflow)?
        / yes_reserve_u;

    let yes_in = u64::try_from(yes_in_u).map_err(|_| MarketsError::MathOverflow)?;
    let no_in = u64::try_from(no_in_u).map_err(|_| MarketsError::MathOverflow)?;
    let lp_to_mint = u64::try_from(lp_to_mint_u).map_err(|_| MarketsError::MathOverflow)?;
    Ok((yes_in, no_in, lp_to_mint))
}

/// Compute the `(yes_out, no_out)` returned for burning `lp_amount` LP, via the
/// vendored `lp_tokens_to_trading_tokens(..., Floor)` — FLOOR rounding so the LP
/// receives `<=` their pro-rata share and the pool keeps the dust (this is what
/// keeps `k` from decreasing on withdraw; scope §3 / §8).
#[inline(never)]
fn compute_remove_liquidity(
    lp_amount: u64,
    lp_supply: u64,
    yes_reserve: u64,
    no_reserve: u64,
) -> Result<(u64, u64)> {
    let result = ConstantProductCurve::lp_tokens_to_trading_tokens(
        lp_amount as u128,
        lp_supply as u128,
        yes_reserve as u128,
        no_reserve as u128,
        RoundDirection::Floor,
    )
    .ok_or(MarketsError::MathOverflow)?;
    let yes_out = u64::try_from(result.token_0_amount).map_err(|_| MarketsError::MathOverflow)?;
    let no_out = u64::try_from(result.token_1_amount).map_err(|_| MarketsError::MathOverflow)?;
    Ok((yes_out, no_out))
}

/// Implied price of NO in basis points over the REAL reserves. In the
/// CPMM-prediction model an outcome's price is the OPPOSITE reserve over the
/// total (buying an outcome depletes its reserve → scarcer → pricier), so
/// price(NO) = `yes_reserve * 10_000 / (yes_reserve + no_reserve)`. Returns 0
/// when the pool is empty (no defined price). Saturating/checked so it can never
/// panic in the emit path.
#[inline(never)]
fn implied_no_price_bps(yes_reserve: u64, no_reserve: u64) -> u64 {
    let total = (yes_reserve as u128) + (no_reserve as u128);
    if total == 0 {
        return 0;
    }
    let bps = (yes_reserve as u128) * 10_000u128 / total;
    u64::try_from(bps).unwrap_or(10_000)
}

/// Accounts for `initialize_markets_config` (Phase 0).
///
/// Mirrors wzrd-rails' `InitializeConfig`: a single `init` of the config PDA by
/// the signer-admin. No token program is needed — Phase 0 moves no funds.
#[derive(Accounts)]
pub struct InitializeMarketsConfig<'info> {
    #[account(
        init,
        payer = admin,
        space = MarketsConfig::LEN,
        seeds = [MARKETS_CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, MarketsConfig>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Accounts for `create_market` (Phase 1).
///
/// Admin-gated: the signer must equal `config.admin` (checked in the handler so
/// the failure surfaces as `Unauthorized`, not a constraint mismatch). The
/// Market PDA is `init`-ed at `[MARKET_SEED, market_id]`; `config` is `mut` to
/// advance `next_market_id`.
#[derive(Accounts)]
#[instruction(market_id: u64)]
pub struct CreateMarket<'info> {
    #[account(
        mut,
        seeds = [MARKETS_CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Account<'info, MarketsConfig>,
    #[account(
        init,
        payer = admin,
        space = Market::LEN,
        seeds = [MARKET_SEED, &market_id.to_le_bytes()],
        bump,
    )]
    pub market: Account<'info, Market>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Accounts for `initialize_market_tokens` (Phase 1).
///
/// Creates the YES + NO Token-2022 mints (fee-free, 6 decimals to match USDC),
/// mint authority = the per-market PDA `[MINT_AUTH_SEED, market_id]`, and the
/// USDC collateral vault owned by the Market PDA. The `init` constraints on the
/// mints/vault enforce single-creation; the handler also guards
/// `!tokens_initialized` for a clean typed error.
#[derive(Accounts)]
pub struct InitializeMarketTokens<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [MARKETS_CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Account<'info, MarketsConfig>,

    #[account(
        mut,
        seeds = [MARKET_SEED, &market.market_id.to_le_bytes()],
        bump = market.bump,
    )]
    pub market: Account<'info, Market>,

    /// USDC collateral mint (the vault holds this). Pinned to the config mint so
    /// the collateral cannot be swapped at token-init time.
    #[account(
        address = config.usdc_mint @ MarketsError::InvalidMarketState,
        mint::token_program = usdc_token_program,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// YES outcome mint — Token-2022, fee-free, 6 decimals, PDA-owned authority.
    #[account(
        init,
        payer = payer,
        seeds = [YES_MINT_SEED, &market.market_id.to_le_bytes()],
        bump,
        mint::decimals = 6,
        mint::authority = mint_authority,
        mint::token_program = outcome_token_program,
    )]
    pub yes_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// NO outcome mint — Token-2022, fee-free, 6 decimals, PDA-owned authority.
    #[account(
        init,
        payer = payer,
        seeds = [NO_MINT_SEED, &market.market_id.to_le_bytes()],
        bump,
        mint::decimals = 6,
        mint::authority = mint_authority,
        mint::token_program = outcome_token_program,
    )]
    pub no_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// USDC collateral vault — owned (authority) by the Market PDA, which signs
    /// the redeem transfer-out.
    #[account(
        init,
        payer = payer,
        seeds = [VAULT_SEED, &market.market_id.to_le_bytes()],
        bump,
        token::mint = usdc_mint,
        token::authority = market,
        token::token_program = usdc_token_program,
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Mint-authority PDA (signs YES/NO mint/burn). No data stored.
    /// CHECK: PDA derived from `[MINT_AUTH_SEED, market_id]`; used only as a
    /// signing authority for the outcome mints.
    #[account(
        seeds = [MINT_AUTH_SEED, &market.market_id.to_le_bytes()],
        bump,
    )]
    pub mint_authority: UncheckedAccount<'info>,

    /// Token-2022 program (outcome YES/NO mints are Token-2022).
    #[account(address = TOKEN_2022_PROGRAM_ID @ MarketsError::InvalidMarketState)]
    pub outcome_token_program: Program<'info, Token2022>,
    /// Token program backing the USDC mint (standard SPL or Token-2022).
    pub usdc_token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

/// Accounts for `mint_complete_set` (Phase 1).
///
/// Deposit N USDC → mint N YES + N NO (the fixed-par rail). The mint-authority
/// PDA signs the two `mint_to` CPIs; the depositor signs the USDC transfer-in.
#[derive(Accounts)]
pub struct MintCompleteSet<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(
        seeds = [MARKET_SEED, &market.market_id.to_le_bytes()],
        bump = market.bump,
    )]
    pub market: Account<'info, Market>,

    #[account(
        seeds = [MARKETS_CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Account<'info, MarketsConfig>,

    /// USDC collateral mint (pinned to the config mint; the vault's
    /// `token::mint = usdc_mint` constraint ties it to the recorded vault too).
    #[account(
        address = config.usdc_mint @ MarketsError::InvalidMarketState,
        mint::token_program = usdc_token_program,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// YES outcome mint (must match the market's recorded yes_mint).
    #[account(
        mut,
        address = market.yes_mint @ MarketsError::InvalidMarketState,
        seeds = [YES_MINT_SEED, &market.market_id.to_le_bytes()],
        bump,
        mint::token_program = outcome_token_program,
    )]
    pub yes_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// NO outcome mint (must match the market's recorded no_mint).
    #[account(
        mut,
        address = market.no_mint @ MarketsError::InvalidMarketState,
        seeds = [NO_MINT_SEED, &market.market_id.to_le_bytes()],
        bump,
        mint::token_program = outcome_token_program,
    )]
    pub no_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// USDC vault (PDA-owned by the market).
    #[account(
        mut,
        address = market.vault @ MarketsError::InvalidMarketState,
        seeds = [VAULT_SEED, &market.market_id.to_le_bytes()],
        bump,
        token::mint = usdc_mint,
        token::authority = market,
        token::token_program = usdc_token_program,
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Depositor's USDC source account.
    #[account(
        mut,
        token::mint = usdc_mint,
        token::authority = depositor,
        token::token_program = usdc_token_program,
    )]
    pub depositor_usdc: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Depositor's YES receiving account.
    #[account(
        mut,
        token::mint = yes_mint,
        token::authority = depositor,
        token::token_program = outcome_token_program,
    )]
    pub depositor_yes: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Depositor's NO receiving account.
    #[account(
        mut,
        token::mint = no_mint,
        token::authority = depositor,
        token::token_program = outcome_token_program,
    )]
    pub depositor_no: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Mint-authority PDA (signs the two `mint_to`).
    /// CHECK: PDA derived from `[MINT_AUTH_SEED, market_id]`; signing authority
    /// for the outcome mints only.
    #[account(
        seeds = [MINT_AUTH_SEED, &market.market_id.to_le_bytes()],
        bump,
    )]
    pub mint_authority: UncheckedAccount<'info>,

    /// Token-2022 program (outcome mints).
    #[account(address = TOKEN_2022_PROGRAM_ID @ MarketsError::InvalidMarketState)]
    pub outcome_token_program: Program<'info, Token2022>,
    /// Token program backing USDC.
    pub usdc_token_program: Interface<'info, TokenInterface>,
}

/// Accounts for `redeem_complete_set` (Phase 1).
///
/// Burn N YES + N NO → return N USDC. The redeemer signs the two burns (they own
/// their outcome accounts); the Market PDA signs the USDC transfer-out.
#[derive(Accounts)]
pub struct RedeemCompleteSet<'info> {
    #[account(mut)]
    pub redeemer: Signer<'info>,

    #[account(
        seeds = [MARKET_SEED, &market.market_id.to_le_bytes()],
        bump = market.bump,
    )]
    pub market: Account<'info, Market>,

    #[account(
        seeds = [MARKETS_CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Account<'info, MarketsConfig>,

    /// USDC collateral mint (pinned to the config mint).
    #[account(
        address = config.usdc_mint @ MarketsError::InvalidMarketState,
        mint::token_program = usdc_token_program,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// YES outcome mint (must match the market's recorded yes_mint).
    #[account(
        mut,
        address = market.yes_mint @ MarketsError::InvalidMarketState,
        seeds = [YES_MINT_SEED, &market.market_id.to_le_bytes()],
        bump,
        mint::token_program = outcome_token_program,
    )]
    pub yes_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// NO outcome mint (must match the market's recorded no_mint).
    #[account(
        mut,
        address = market.no_mint @ MarketsError::InvalidMarketState,
        seeds = [NO_MINT_SEED, &market.market_id.to_le_bytes()],
        bump,
        mint::token_program = outcome_token_program,
    )]
    pub no_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// USDC vault (PDA-owned by the market; the market signs the transfer-out).
    #[account(
        mut,
        address = market.vault @ MarketsError::InvalidMarketState,
        seeds = [VAULT_SEED, &market.market_id.to_le_bytes()],
        bump,
        token::mint = usdc_mint,
        token::authority = market,
        token::token_program = usdc_token_program,
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Redeemer's USDC destination account.
    #[account(
        mut,
        token::mint = usdc_mint,
        token::authority = redeemer,
        token::token_program = usdc_token_program,
    )]
    pub redeemer_usdc: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Redeemer's YES source account (burned from).
    #[account(
        mut,
        token::mint = yes_mint,
        token::authority = redeemer,
        token::token_program = outcome_token_program,
    )]
    pub redeemer_yes: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Redeemer's NO source account (burned from).
    #[account(
        mut,
        token::mint = no_mint,
        token::authority = redeemer,
        token::token_program = outcome_token_program,
    )]
    pub redeemer_no: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Token-2022 program (outcome mints).
    #[account(address = TOKEN_2022_PROGRAM_ID @ MarketsError::InvalidMarketState)]
    pub outcome_token_program: Program<'info, Token2022>,
    /// Token program backing USDC.
    pub usdc_token_program: Interface<'info, TokenInterface>,
}

/// Accounts for `initialize_pool` (Phase 2).
///
/// Creates the Pool PDA `[POOL_SEED, market.key()]`, the LP mint
/// `[LP_MINT_SEED, market.key()]` (Token-2022, pool PDA = mint authority), and the
/// pool's YES + NO reserve token accounts (Associated Token Accounts owned by the
/// pool PDA, for the market's recorded yes_mint / no_mint). The pool PDA is its
/// own LP-mint authority and reserve-account owner, and signs every Phase-2
/// transfer-out / LP mint with the BYTE-IDENTICAL `[POOL_SEED, market.key()]`
/// seeds (the stored `pool.bump`).
#[derive(Accounts)]
pub struct InitializePool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [MARKET_SEED, &market.market_id.to_le_bytes()],
        bump = market.bump,
    )]
    pub market: Account<'info, Market>,

    /// The constant-product pool over this market's YES/NO outcome tokens.
    /// `init` enforces single-creation (a second `initialize_pool` aborts with
    /// account-already-in-use).
    #[account(
        init,
        payer = payer,
        space = Pool::LEN,
        seeds = [POOL_SEED, market.key().as_ref()],
        bump,
    )]
    pub pool: Account<'info, Pool>,

    /// YES outcome mint (pinned to the market's recorded yes_mint).
    #[account(
        address = market.yes_mint @ MarketsError::InvalidMarketState,
        mint::token_program = outcome_token_program,
    )]
    pub yes_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// NO outcome mint (pinned to the market's recorded no_mint).
    #[account(
        address = market.no_mint @ MarketsError::InvalidMarketState,
        mint::token_program = outcome_token_program,
    )]
    pub no_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// LP mint — Token-2022, 6 decimals (matches the outcome mints), pool PDA is
    /// the mint authority (it signs `mint_to` in `add_liquidity`).
    #[account(
        init,
        payer = payer,
        seeds = [LP_MINT_SEED, market.key().as_ref()],
        bump,
        mint::decimals = 6,
        mint::authority = pool,
        mint::token_program = lp_token_program,
    )]
    pub lp_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// Pool's YES reserve account (ATA owned by the pool PDA).
    #[account(
        init,
        payer = payer,
        associated_token::mint = yes_mint,
        associated_token::authority = pool,
        associated_token::token_program = outcome_token_program,
    )]
    pub pool_yes: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Pool's NO reserve account (ATA owned by the pool PDA).
    #[account(
        init,
        payer = payer,
        associated_token::mint = no_mint,
        associated_token::authority = pool,
        associated_token::token_program = outcome_token_program,
    )]
    pub pool_no: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Token-2022 program (outcome mints).
    #[account(address = TOKEN_2022_PROGRAM_ID @ MarketsError::InvalidMarketState)]
    pub outcome_token_program: Program<'info, Token2022>,
    /// Token-2022 program (LP mint is also Token-2022).
    #[account(address = TOKEN_2022_PROGRAM_ID @ MarketsError::InvalidMarketState)]
    pub lp_token_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,
    pub system_program: Program<'info, System>,
}

/// Accounts for `add_liquidity` (Phase 2).
///
/// The provider deposits YES + NO from their own accounts (they sign the two
/// transfers-in) and receives LP minted by the pool PDA. The pool's reserve ATAs
/// + LP mint are pinned to the pool's recorded `lp_mint` / the market's mints.
#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(mut)]
    pub provider: Signer<'info>,

    #[account(
        seeds = [MARKET_SEED, &market.market_id.to_le_bytes()],
        bump = market.bump,
    )]
    pub market: Account<'info, Market>,

    #[account(
        mut,
        seeds = [POOL_SEED, market.key().as_ref()],
        bump = pool.bump,
        has_one = market @ MarketsError::InvalidMarketState,
        has_one = lp_mint @ MarketsError::InvalidMarketState,
    )]
    pub pool: Account<'info, Pool>,

    /// YES outcome mint (pinned to the market's recorded yes_mint).
    #[account(
        mut,
        address = market.yes_mint @ MarketsError::InvalidMarketState,
        mint::token_program = outcome_token_program,
    )]
    pub yes_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// NO outcome mint (pinned to the market's recorded no_mint).
    #[account(
        mut,
        address = market.no_mint @ MarketsError::InvalidMarketState,
        mint::token_program = outcome_token_program,
    )]
    pub no_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// LP mint (pinned to the pool's recorded lp_mint; pool PDA is its authority).
    #[account(
        mut,
        address = pool.lp_mint @ MarketsError::InvalidMarketState,
        seeds = [LP_MINT_SEED, market.key().as_ref()],
        bump,
        mint::token_program = lp_token_program,
    )]
    pub lp_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// Pool's YES reserve (ATA owned by the pool PDA).
    #[account(
        mut,
        associated_token::mint = yes_mint,
        associated_token::authority = pool,
        associated_token::token_program = outcome_token_program,
    )]
    pub pool_yes: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Pool's NO reserve (ATA owned by the pool PDA).
    #[account(
        mut,
        associated_token::mint = no_mint,
        associated_token::authority = pool,
        associated_token::token_program = outcome_token_program,
    )]
    pub pool_no: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Provider's YES source account.
    #[account(
        mut,
        token::mint = yes_mint,
        token::authority = provider,
        token::token_program = outcome_token_program,
    )]
    pub provider_yes: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Provider's NO source account.
    #[account(
        mut,
        token::mint = no_mint,
        token::authority = provider,
        token::token_program = outcome_token_program,
    )]
    pub provider_no: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Provider's LP destination account.
    #[account(
        mut,
        token::mint = lp_mint,
        token::authority = provider,
        token::token_program = lp_token_program,
    )]
    pub provider_lp: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Token-2022 program (outcome mints).
    #[account(address = TOKEN_2022_PROGRAM_ID @ MarketsError::InvalidMarketState)]
    pub outcome_token_program: Program<'info, Token2022>,
    /// Token-2022 program (LP mint).
    #[account(address = TOKEN_2022_PROGRAM_ID @ MarketsError::InvalidMarketState)]
    pub lp_token_program: Program<'info, Token2022>,
}

/// Accounts for `remove_liquidity` (Phase 2).
///
/// The provider burns LP from their own account (they sign the burn) and the
/// pool PDA signs the YES + NO transfers-out. Remove is allowed post-resolution
/// so LPs can always exit.
#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    #[account(mut)]
    pub provider: Signer<'info>,

    #[account(
        seeds = [MARKET_SEED, &market.market_id.to_le_bytes()],
        bump = market.bump,
    )]
    pub market: Account<'info, Market>,

    #[account(
        mut,
        seeds = [POOL_SEED, market.key().as_ref()],
        bump = pool.bump,
        has_one = market @ MarketsError::InvalidMarketState,
        has_one = lp_mint @ MarketsError::InvalidMarketState,
    )]
    pub pool: Account<'info, Pool>,

    /// YES outcome mint (pinned to the market's recorded yes_mint).
    #[account(
        mut,
        address = market.yes_mint @ MarketsError::InvalidMarketState,
        mint::token_program = outcome_token_program,
    )]
    pub yes_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// NO outcome mint (pinned to the market's recorded no_mint).
    #[account(
        mut,
        address = market.no_mint @ MarketsError::InvalidMarketState,
        mint::token_program = outcome_token_program,
    )]
    pub no_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// LP mint (pinned to the pool's recorded lp_mint).
    #[account(
        mut,
        address = pool.lp_mint @ MarketsError::InvalidMarketState,
        seeds = [LP_MINT_SEED, market.key().as_ref()],
        bump,
        mint::token_program = lp_token_program,
    )]
    pub lp_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// Pool's YES reserve (ATA owned by the pool PDA; the pool signs transfer-out).
    #[account(
        mut,
        associated_token::mint = yes_mint,
        associated_token::authority = pool,
        associated_token::token_program = outcome_token_program,
    )]
    pub pool_yes: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Pool's NO reserve (ATA owned by the pool PDA; the pool signs transfer-out).
    #[account(
        mut,
        associated_token::mint = no_mint,
        associated_token::authority = pool,
        associated_token::token_program = outcome_token_program,
    )]
    pub pool_no: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Provider's YES destination account.
    #[account(
        mut,
        token::mint = yes_mint,
        token::authority = provider,
        token::token_program = outcome_token_program,
    )]
    pub provider_yes: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Provider's NO destination account.
    #[account(
        mut,
        token::mint = no_mint,
        token::authority = provider,
        token::token_program = outcome_token_program,
    )]
    pub provider_no: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Provider's LP source account (burned from; they sign the burn).
    #[account(
        mut,
        token::mint = lp_mint,
        token::authority = provider,
        token::token_program = lp_token_program,
    )]
    pub provider_lp: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Token-2022 program (outcome mints).
    #[account(address = TOKEN_2022_PROGRAM_ID @ MarketsError::InvalidMarketState)]
    pub outcome_token_program: Program<'info, Token2022>,
    /// Token-2022 program (LP mint).
    #[account(address = TOKEN_2022_PROGRAM_ID @ MarketsError::InvalidMarketState)]
    pub lp_token_program: Program<'info, Token2022>,
}

/// Accounts for `swap` (Phase 2).
///
/// The trader pulls the input outcome token in (they sign their source account)
/// and the pool PDA signs the output transfer-out. Both pool reserve ATAs are
/// `mut`; the swap updates the REAL reserves and the price moves. Trading halts
/// post-resolution.
#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub trader: Signer<'info>,

    #[account(
        seeds = [MARKET_SEED, &market.market_id.to_le_bytes()],
        bump = market.bump,
    )]
    pub market: Account<'info, Market>,

    #[account(
        mut,
        seeds = [POOL_SEED, market.key().as_ref()],
        bump = pool.bump,
        has_one = market @ MarketsError::InvalidMarketState,
    )]
    pub pool: Account<'info, Pool>,

    /// YES outcome mint (pinned to the market's recorded yes_mint).
    #[account(
        mut,
        address = market.yes_mint @ MarketsError::InvalidMarketState,
        mint::token_program = outcome_token_program,
    )]
    pub yes_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// NO outcome mint (pinned to the market's recorded no_mint).
    #[account(
        mut,
        address = market.no_mint @ MarketsError::InvalidMarketState,
        mint::token_program = outcome_token_program,
    )]
    pub no_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// Pool's YES reserve (ATA owned by the pool PDA).
    #[account(
        mut,
        associated_token::mint = yes_mint,
        associated_token::authority = pool,
        associated_token::token_program = outcome_token_program,
    )]
    pub pool_yes: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Pool's NO reserve (ATA owned by the pool PDA).
    #[account(
        mut,
        associated_token::mint = no_mint,
        associated_token::authority = pool,
        associated_token::token_program = outcome_token_program,
    )]
    pub pool_no: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Trader's YES account (source for YesToNo, destination for NoToYes).
    #[account(
        mut,
        token::mint = yes_mint,
        token::authority = trader,
        token::token_program = outcome_token_program,
    )]
    pub trader_yes: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Trader's NO account (destination for YesToNo, source for NoToYes).
    #[account(
        mut,
        token::mint = no_mint,
        token::authority = trader,
        token::token_program = outcome_token_program,
    )]
    pub trader_no: Box<InterfaceAccount<'info, TokenAccountInterface>>,

    /// Token-2022 program (outcome mints).
    #[account(address = TOKEN_2022_PROGRAM_ID @ MarketsError::InvalidMarketState)]
    pub outcome_token_program: Program<'info, Token2022>,
}
