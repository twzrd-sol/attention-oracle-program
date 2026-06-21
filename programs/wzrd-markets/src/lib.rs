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
        config._reserved = [0u8; 64];

        emit!(MarketsConfigInitialized {
            config: config_key,
            admin,
            usdc_mint,
            resolver_multisig,
            slot,
        });
        Ok(())
    }

    // ─── Phase 1-3 roadmap (NOT YET IMPLEMENTED) ─────────────────────────────
    //
    // Phase 1 — market lifecycle + complete-set rail:
    //   - create_market(ctx, args)             // + snapshot resolution root+seq at create (audit H-01)
    //   - initialize_market_tokens(ctx)        // Token-2022-only YES/NO mints + mint-authority PDA
    //   - mint_complete_set(ctx, amount)       // 1 USDC -> 1 YES + 1 NO (the fixed-par rail; pre-resolution only)
    //   - redeem_complete_set(ctx, amount)     // inverse; preserve the net_received-mints-supply solvency invariant
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
