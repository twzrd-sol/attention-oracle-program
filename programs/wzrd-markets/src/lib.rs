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

    // ─── Phase 2-3 roadmap (NOT YET IMPLEMENTED) ─────────────────────────────
    //
    // Phase 2 — the CPMM (moving-odds engine), uses `curve::ConstantProductCurve`:
    //   - initialize_pool(ctx, seed_args)      // create YES/NO constant-product pool + LP mint; seed bounding-phase virtual liquidity
    //   - add_liquidity(ctx, args)             // LP provides both outcome sides; lp_tokens_to_trading_tokens accounting
    //   - remove_liquidity(ctx, args)
    //   - swap(ctx, args)                      // buy/sell YES or NO with min_amount_out slippage guard (swap_base_input/output_without_fees)
    //   ACCEPTANCE GATE (Phase 2): the mint/swap arbitrage loop keeps sum(YES,NO) coherent vs collateral.
    //
    // Phase 3 — resolution + settlement (in-house publisher, audit H-01/H-02/M-04/M-05):
    //   - publish_attention_root(ctx, args)    // in-house allow-listed publisher (advances AttentionRootConfig.last_published_seq); reuse rails listen-payout pattern
    //   - resolve_market(ctx, proof)           // verify proof vs the CREATE-TIME-snapshotted root; + dispute window before final
    //   - settle(ctx)                          // burn winning outcome token 1:1 for USDC; preserve solvency invariant
    //   - resolve_override(ctx, outcome)       // multisig-gated fallback for disputed/missing data
    //   - sweep_residual(ctx) / close_market(ctx)  // admin-gated, supply==0 guards
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
