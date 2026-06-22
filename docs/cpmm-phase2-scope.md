# Phase 2 Scope: CPMM Pool + Swap (the moving-odds engine)

> **Status**: Scope. Builds on Phase 1 (`claude/cpmm-markets-phase1`, tip dedd1b3).
> **Branch**: `claude/cpmm-markets-phase2`
> **Goal**: the constant-product pool over YES/NO outcome tokens + liquidity + swap. This is where ODDS MOVE. **Acceptance gate = the mint/swap arbitrage coherence loop.**

---

## 0. The mechanism (read this first — it's the crux)

A CPMM **prediction** market is not a normal AMM. The pool holds **YES and NO outcome-token reserves** (not USDC). The invariant is `yes_reserve * no_reserve = k`. The implied probability of YES = `no_reserve / (yes_reserve + no_reserve)`.

**The pool trades YES <-> NO directly** (outcome-token-for-outcome-token). USDC never enters the pool. This is the Gnosis/Hedgehog model and it is what makes the arb loop provable.

**"Buy YES with USDC" is a COMPOSED operation** (the two rails working together):
1. `mint_complete_set(N USDC)` -> N YES + N NO (Phase 1 rail).
2. `swap` the N NO into the pool -> more YES out.
3. Trader keeps (N YES + swap-out YES). Pool: NO reserve up, YES reserve down -> YES price up.

**"Sell YES for USDC"** is the inverse: swap YES -> NO in the pool until you hold equal YES+NO, then `redeem_complete_set` -> USDC.

Phase 2 builds the **pool + the raw YES<->NO swap + liquidity**. The USDC-denominated buy/sell *wrappers* (mint-then-swap, swap-then-redeem) can be thin convenience IXs in Phase 2 OR left to the client/SDK composing the two rails — **decision: implement the raw `swap` (YES<->NO) in Phase 2; the USDC wrappers are optional convenience and may defer to SDK.** The arb gate is proven on the raw primitives.

---

## 1. Instructions

| Instruction | Purpose |
|-------------|---------|
| `initialize_pool` | Create the Pool PDA + LP mint + the pool's YES/NO token accounts. Seed bounding-phase virtual liquidity for cold-start. |
| `add_liquidity` | LP deposits YES + NO in the current ratio -> receives LP tokens. |
| `remove_liquidity` | Burn LP tokens -> withdraw YES + NO pro-rata. |
| `swap` | Swap YES->NO or NO->YES against the constant-product curve, with `min_amount_out` slippage guard. The moving-odds primitive. |

---

## 2. `initialize_pool` + the bounding phase (cold-start)

**The problem**: a brand-new market on a streamer nobody has bet on has zero liquidity. The first trade against an empty pool either reverts or gets a nonsense price. **The fix** (Path Protocol's documented design, scope §4): seed a **virtual-liquidity floor** so the first trades price against a sane baseline.

- `initialize_pool(virtual_liquidity: u64)`: create the Pool PDA `[POOL_SEED, market.key()]`, the LP mint `[LP_MINT_SEED, market.key()]` (Token-2022, the pool PDA is mint authority), and the pool's YES/NO token accounts (owned by the pool PDA).
- Set `bounding_phase_active = true`, `virtual_liquidity = V`, `yes_reserve = 0`, `no_reserve = 0`, `lp_supply = 0`.
- **Bounding-phase math**: while `bounding_phase_active`, swap pricing uses `(yes_reserve + V)` and `(no_reserve + V)` as the effective reserves — so the FIRST trade sees a 50/50 implied price (V/2V = 0.5) and moves smoothly from there, instead of dividing by zero. V is a fixed floor, NOT real tokens (the pool doesn't hold V of anything — it's added to the curve inputs only).
- **Transition**: when real liquidity arrives (`add_liquidity` pushes real reserves above a threshold, or an explicit `graduate` step), set `bounding_phase_active = false` and pricing uses the real reserves only. **Decision: transition when `add_liquidity` first brings both real reserves >= V** (the virtual floor is now dominated by real liquidity). Document the exact threshold; keep it simple and testable.
- **CRITICAL accounting rule**: the virtual liquidity must NEVER be withdrawable and must NEVER let the pool pay out tokens it doesn't hold. The pool's token-account balances are the hard ceiling on any transfer-out; V only shifts the *price*, never the *payout solvency*. This is the #1 thing the Rails/Orchestration specialists flagged — get it exactly right or the arb loop shows phantom profit.

---

## 3. `add_liquidity` / `remove_liquidity`

- `add_liquidity(max_yes, max_no, min_lp)`: deposit YES + NO. First LP sets the ratio; subsequent LPs must match the current `yes_reserve:no_reserve` ratio (deposit is bounded by the scarcer side, refund/ignore the excess or require exact — **decision: compute the required NO for the given YES at the current ratio (and vice versa), take the min, mint LP proportional to the share added**, mirroring Raydium's `lp_tokens_to_trading_tokens` inverse). Mint LP tokens, update `yes_reserve`/`no_reserve`/`lp_supply`.
- `remove_liquidity(lp_amount, min_yes, min_no)`: burn LP, return YES + NO pro-rata via `lp_tokens_to_trading_tokens(lp_amount, lp_supply, yes_reserve, no_reserve, Floor)`. Floor rounding (LP gets slightly less, pool keeps dust — never overpay). Update reserves/supply.
- Use the vendored `lp_tokens_to_trading_tokens` (already hardened to `Option`). LP token = Token-2022, pool PDA is mint authority.

---

## 4. `swap` (the moving-odds primitive)

- `swap(amount_in, min_amount_out, direction)` where direction = YesToNo | NoToYes.
- Pull `amount_in` of the input outcome token from the trader into the pool's input reserve account.
- Compute `amount_out = swap_base_input_without_fees(amount_in, effective_input_reserve, effective_output_reserve)` where `effective_*` includes the bounding-phase virtual floor IF `bounding_phase_active`.
- **Slippage**: require `amount_out >= min_amount_out` (SlippageExceeded else).
- **Solvency**: require `amount_out <= pool's real output-token-account balance` (the pool can only pay what it holds — the virtual floor does NOT add payable tokens). Transfer `amount_out` out (pool PDA signs).
- Update `yes_reserve`/`no_reserve` (the REAL reserves; reserves track actual token-account balances).
- **Fee**: Phase 2 = fee-free swap (matches the fee-exempt-collateral decision; a swap fee / LP fee is a later economic-tuning decision, not needed to prove coherence). Document that fee=0 for v1.
- Emit Swapped (amount_in, amount_out, new implied price).
- Rounding: the curve floors output (trader gets <= exact, pool keeps dust) — this is the audited Raydium behavior and the reason `curve_value_does_not_decrease_from_swap` holds. Preserve it.

---

## 5. THE ACCEPTANCE GATE — arb-coherence loop

This is the one test with real economic risk. It must pass or Phase 2 is NOT done.

**The coherence property**: a trader cannot extract free USDC by cycling the two rails. Concretely, the round-trip `mint_complete_set(N) -> swap both sides through the pool -> redeem_complete_set` must return the trader **<= N USDC** (they can only lose to slippage/dust, never gain). And the pool's constant-product invariant `k` must never DECREASE across a swap.

Required tests (litesvm, behind localtest):
1. **`arb_coherence_no_free_usdc` (THE GATE)**: set up a pool with real liquidity. A trader mints a complete set for N USDC, sells both YES and NO into the pool (or does a round-trip swap), redeems back to USDC. Assert final USDC <= N (no free money). Assert pool `k_after >= k_before` across each swap.
2. **`swap_moves_price`**: a swap of YES->NO measurably increases the implied price of NO (and decreases YES). Assert the price moved in the right direction by the curve-predicted amount.
3. **`bounding_phase_first_trade_sane`**: the FIRST swap on a fresh pool (bounding phase) gets a sane price near 0.5 (not a revert, not a divide-by-zero, not 0 or infinity).
4. **`bounding_phase_solvency`**: prove the pool CANNOT pay out more tokens than it holds even under the virtual floor — attempt a swap that would require paying out more than the real reserve; assert it reverts (the virtual floor shifts price, not payout capacity). **This is the phantom-profit guard the specialists flagged.**
5. **`add_remove_liquidity_roundtrip`**: add liquidity, remove it, LP gets back <= what they put in (dust to the pool), reserves/supply consistent.
6. **`swap_slippage_guard`**: a swap with `min_amount_out` higher than achievable reverts (SlippageExceeded).
7. **`remove_liquidity_never_overpays`** + **`single_side_heavy_swap`** (large swap that nearly drains one side — assert it reverts or bounds correctly, never pays out more than held).
8. Boundary: zero-amount swap/add/remove rejected; swap on an uninitialized pool rejected; swap after resolved rejected (markets stop trading at resolution).
9. **Curve proptests still pass** (Phase 0/1 regression).

---

## 6. State (Phase 0 left room)

- `Pool` struct already has every field (yes_reserve, no_reserve, lp_mint, lp_supply, bounding_phase_active, virtual_liquidity, _reserved). Phase 2 populates them.
- No new accounts beyond the pool's YES/NO token accounts + LP mint (created by `initialize_pool`).
- Add a `swap_paused` or rely on `market.resolved` to halt trading post-resolution — **decision: gate swap/add on `!market.resolved`** (trading stops at resolution; remove_liquidity may still be allowed so LPs can exit — decide and document).

---

## 7. Errors (add to MarketsError)

`PoolAlreadyExists`, `PoolNotInitialized`, `SlippageExceeded`, `InsufficientPoolLiquidity`, `RatioMismatch`, `ZeroLiquidity`, `BoundingPhaseViolation`, `MarketTradingHalted` (swap after resolved).

---

## 8. Non-negotiables (carried)

- **Pool PDA signer-seed correctness**: the seeds + bump used to sign swap/remove transfers OUT must be byte-identical to the seeds at pool init. The Rails + Sebastian flagged this — verify it explicitly (store bump on the Pool, use it consistently).
- **Virtual liquidity shifts PRICE, never PAYOUT solvency.** The pool's real token-account balance is the hard ceiling on every transfer-out. Test #4 proves this.
- **Curve floors output** (pool keeps dust) — preserve the audited Raydium rounding; it's why `k` never decreases.
- **Fee = 0 for v1** (matches fee-exempt collateral; swap fee is later tuning).
- No merkle/resolution code (Phase 3). But Henry's note: the one-keccak/node-domain/MAX_PROOF_LEN conventions one-pager should be written THIS phase (Luna owns it) so Phase 3 doesn't hit the silent-failure trap.

---

## 9. Out of scope for Phase 2

- Resolution / settle / publish_attention_root / multisig (Phase 3).
- Swap fees / LP fee economics (later tuning).
- Jupiter routing metadata (Phase 4).
- The USDC buy/sell convenience wrappers MAY defer to SDK (the raw swap + the two rails are sufficient to prove coherence).

---

## 10. Verification checklist (the independent gate before commit)

1. `cargo build -p wzrd-markets` (host) + `cargo-build-sbf` — both clean.
2. **The arb-coherence gate test passes** (test #1) — final USDC <= N, k never decreases.
3. The bounding-phase solvency guard passes (test #4) — no phantom payout.
4. swap_moves_price, slippage guard, add/remove roundtrip all pass.
5. Curve proptests regression-pass.
6. Pool PDA signer seeds verified byte-identical init-vs-use.
7. No prod unwraps in the new swap/liquidity paths.
