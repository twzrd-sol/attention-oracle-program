# Build Scope: CPMM Outcome-Token Prediction Markets (wzrd-rails)

> **Status**: Scope / design. Not yet implemented.
> **Date**: 2026-06-21
> **Author**: Plamen audit follow-on
> **Decision basis**: the Plamen audit port-readiness verdict (`AUDIT_REPORT.md`) + the 45-day Solana DeFi trend scan.

---

## 0. The decision this implements

The audit found `markets.rs` (the CTF reference) is collateral-sound but mints YES+NO **1:1 at par** — it settles binary outcomes but gives **no moving odds**. The product goal ("bet long/short on a streamer's future attention") needs prices that move. The trend scan confirmed:

- **Nobody on Solana ships full Gnosis-CTF** — porting `markets.rs` verbatim makes us the first, with no precedent.
- **CPMM outcome shares is the open default** (Hedgehog-style), and we already run a CPMM off-chain (`index_math.rs` on the streaming-index branch) — lowest friction.
- **LMSR / pm-AMM have zero live Solana deployment** — being the first is a research project, not a build.
- **Resolution: every live Solana PM uses a trusted publisher/multisig.** Our existing allow-listed-publisher + signed-attestation + merkle design IS the current norm; harden it with Switchboard On-Demand + a multisig override, do not invent a dispute oracle.

**Chosen architecture: constant-product (x*y=k) AMM over per-market YES/NO SPL outcome tokens, collateralized in a fee-exempt collateral mint, resolved by an in-house allow-listed publisher with a multisig override, settled 1:1 on the winning side.** Outcome tokens are designed to be Jupiter-routable (one SPL token per outcome) so markets can later be listed/routed rather than competing head-on.

---

## 1. What we reuse vs build vs change

### Reuse from `markets.rs` (the CTF reference — collateral math is sound, verified by the audit)
- The market lifecycle state machine: `create_market` -> `initialize_market_tokens` -> (trade) -> `resolve_market` -> `settle` -> `sweep_residual` -> `close_market`. The audit (MR-7) confirmed the guards are correct.
- The 1:1 collateral solvency invariant for mint/redeem/settle (audit MR-1: `vault.amount == yes_supply == no_supply`, last-winner-insolvency REFUTED). **Preserve the `net_received`-mints-supply pattern.**
- The YES/NO mint + vault + mint-authority-PDA structure.

### Build new (the CPMM layer — this is what `markets.rs` does NOT have)
- A **constant-product pool** holding YES and NO reserves. Price of YES = `no_reserve / (yes_reserve + no_reserve)` (bounded in (0,1), the implied probability). This is the moving-odds engine.
- `add_liquidity` / `remove_liquidity` (LP provides both sides, gets LP tokens; needed to bootstrap a market) — fork the mechanics from `raydium-io/raydium-cp-swap` (audited Anchor/Token-2022 CPMM).
- `swap` (buy/sell YES or NO against the pool with slippage bound + min-out). This replaces the fixed-par `mint_shares`/`redeem_shares` as the primary trade path.
- A **bootstrapping "bounding phase"** for thin long-tail streamer markets (borrowed from Path Protocol's documented design): seed the pool with a fixed virtual-liquidity floor so the first trades have a sane price, then let real LP take over. This is the cold-start fix for markets on streamers nobody has bet on yet.

### Change from `markets.rs` (the audit-mandated fixes — these MUST land in the port, not the original immutable program)
- **Resolution source (audit H-02 / G3)**: `markets.rs` reads the immutable AO global root. The port stands up an **in-house `AttentionRootConfig` publisher inside wzrd-rails** (reuse the existing `publish_listen_payout_root` + allowlist pattern) + a **multisig override**. Do NOT cross-program-read the immutable AO (audit option (a) — inherits the finality bug + spoof risk).
- **Finality (audit H-01)**: snapshot the resolution root (`root` + `seq`) into `MarketState` at **create-time**, not re-read at resolve-time. Add a resolution deadline + admin pro-rata recovery for never-resolved markets + a dispute/challenge window before settlement is final.
- **Merkle stack unification (audit M-04)**: the port must pick ONE keccak lib + ONE node-domain convention + ONE `MAX_PROOF_LEN` before any tree is built. Use the rails domain-separated style (`solana_keccak_hasher`, leaf+node domains). Lock a golden vector.
- **Collateral fee-exemption (audit L-08 / MR-2)**: do NOT collateralize in fee-charging CCM for a market where collateral cycles repeatedly — the Token-2022 transfer fee compounds into a house edge each trade. Use a fee-exempt collateral path (e.g. USDC, or a fee-exempt PDA) for the AMM pool.
- **Outcome-token standard (audit H-P5 / AC-4)**: pick ONE — Token-2022 outcome mints uniformly (drop the SPL/either duality). If Token-2022, carry the before/after fee-sampling pattern; simpler if outcome mints are fee-free.

---

## 2. Instruction set (the on-chain surface to build)

| Instruction | Source | Notes |
|-------------|--------|-------|
| `create_market` | adapt markets.rs | + snapshot resolution root+seq at create (H-01); + market params (streamer id, metric, target/scalar range, deadline) |
| `initialize_market_tokens` | adapt markets.rs | Token-2022-only outcome mints; mint-authority PDA |
| `initialize_pool` (new) | fork raydium-cp-swap | create the YES/NO constant-product pool + LP mint; seed bounding-phase virtual liquidity |
| `add_liquidity` / `remove_liquidity` (new) | fork raydium-cp-swap | LP provides both outcome sides; LP token accounting |
| `swap` (new) | fork raydium-cp-swap | buy/sell YES or NO with `min_amount_out` slippage guard; this is the moving-odds trade |
| `mint_complete_set` / `redeem_complete_set` | adapt markets.rs mint_shares/redeem_shares | the arbitrage rail: 1 collateral <-> 1 YES + 1 NO, keeps the pool price honest vs collateral. Pre-resolution only. |
| `publish_attention_root` (new) | reuse listen_payout publisher | in-house allow-listed root publisher (G3) |
| `resolve_market` | adapt markets.rs | verify proof vs the **create-time-snapshotted** root; + dispute window before final |
| `settle` | adapt markets.rs | burn winning outcome token 1:1 for collateral; preserve solvency invariant |
| `sweep_residual` / `close_market` | adapt markets.rs | admin-gated, supply==0 guards (audit-confirmed) |
| `resolve_override` (new) | new, multisig-gated | the multisig fallback for disputed/missing data (Drift BET lifecycle) |

---

## 3. The two pricing rails (why both mint-complete-set AND swap)

This is the crux of a CPMM prediction market and the thing `markets.rs` lacks:

1. **`mint_complete_set`** (1 collateral -> 1 YES + 1 NO) and `redeem_complete_set` (the inverse) are the **fixed-par rail** — they peg the *sum* YES+NO to 1 collateral. This is markets.rs's existing behavior.
2. **`swap`** against the constant-product pool is the **price-discovery rail** — it moves the YES/NO *ratio* (the odds) without changing the sum.

Arbitrage between the two keeps the market coherent: if the pool prices YES at 0.7 + NO at 0.7 (sum 1.4 > 1), an arber mints a complete set for 1 collateral and sells both into the pool for 1.4, pushing prices back toward sum = 1. **This arb loop is what makes CPMM outcome tokens a real market and not just a fixed mint.** It is the single most important property to get right and to test.

---

## 4. Build phases (concrete, sequenced)

**Phase 0 — fork + skeleton (no funds):**
- Vendor `raydium-io/raydium-cp-swap` swap/liquidity math into a `cpmm` module under wzrd-rails. Strip to the constant-product core.
- Define `Market`, `Pool`, `OutcomePosition` state. Snapshot-root-at-create.

**Phase 1 — market lifecycle + complete-set rail:**
- `create_market`, `initialize_market_tokens` (Token-2022), `mint_complete_set` / `redeem_complete_set`. Port the solvency invariant + tests from markets.rs.

**Phase 2 — the CPMM (the moving-odds engine):**
- `initialize_pool` (+ bounding-phase seed), `add_liquidity`, `remove_liquidity`, `swap`. Fee-exempt collateral. **Test the mint/swap arbitrage loop** (the coherence property above) — this is the acceptance gate for Phase 2.

**Phase 3 — resolution + settlement:**
- `publish_attention_root` (in-house publisher + allowlist), `resolve_market` (vs snapshotted root + dispute window), `settle`, `resolve_override` (multisig). Port the audit's H-01/H-02/M-04/M-05 fixes.

**Phase 4 — hardening + interop:**
- Outcome tokens shaped for Jupiter routing. Re-run the full Plamen audit on the new program before any mainnet deploy. Switchboard On-Demand custom feed wired as the attention data source feeding `publish_attention_root`.

---

## 5. Non-negotiables (carried from the audit)

- **This is a NEW program, separately deployed and audited.** The AO program is immutable; wzrd-rails is upgradeable but this is a large surface — it gets its own audit pass before real money.
- **Pick ONE of everything before building**: one collateral mint (fee-exempt), one outcome-token standard (Token-2022), one keccak lib + node convention (rails domain-separated), one MAX_PROOF_LEN. The audit's M-04 chain (CH-3) is "two conventions -> every proof fails silently -> total fund lock" — the highest-leverage silent failure. A one-page conventions spec precedes any tree.
- **Resolution is trusted-but-bounded**, matching the Solana norm: allow-listed in-house publisher + multisig override + dispute window. Genuine dispute resistance (UMA-style) only exists by bridging to EVM — out of scope for v1, note it as the v2 trust-upgrade path.
- **Distribution > curve novelty**: design outcome tokens to be Jupiter-routable from day one. DFlow gets 98% of its volume via wallet embedding; the curve is not the adoption driver.

---

## 6. What NOT to build (explicitly out of scope for v1)

- LMSR / pm-AMM curves — no Solana precedent, research project. CPMM first.
- A native Solana optimistic/dispute oracle — does not exist; do not invent it. Trusted publisher + multisig is the norm.
- Cross-program reads of the immutable AO root (audit option (a)) — inherits the finality bug + spoof surface.
- Full Gnosis-CTF composable 1:1 conditional-token split/merge across nested markets — over-engineered for binary/threshold streamer markets; the complete-set mint/redeem rail gives the useful 90% without the nesting.
