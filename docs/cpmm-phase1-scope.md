# Phase 1 Scope: Market Lifecycle + Complete-Set Rail (wzrd-markets)

> **Status**: Scope. Builds on Phase 0 (`claude/cpmm-markets-phase0`).
> **Branch**: `claude/cpmm-markets-phase1`
> **Goal**: the market lifecycle (create + token init) + the **fixed-par complete-set rail** (1 USDC <-> 1 YES + 1 NO), with the audit-verified solvency invariant ported. NO CPMM pool / swap yet — that's Phase 2.

---

## What Phase 1 delivers

| Instruction | Purpose |
|-------------|---------|
| `create_market` | Open a market on a streamer. Snapshots the resolution root + seq AT CREATE-TIME (audit H-01). Populates the Phase-0 `Market` struct. |
| `initialize_market_tokens` | Create the YES + NO Token-2022 mints (fee-free) + the USDC collateral vault, all PDA-owned. Sets `tokens_initialized`. |
| `mint_complete_set` | Deposit N USDC -> mint exactly N YES + N NO (the fixed-par rail; the audit-verified MR-1 solvency pattern). |
| `redeem_complete_set` | Burn N YES + N NO -> return N USDC (the inverse; pre-resolution only). |

This gives a **testable complete-set roundtrip** and the solvency proof that anchors everything Phase 2+ builds on.

---

## The solvency invariant (the heart of Phase 1)

Ported from `markets.rs::mint_shares` (audit MR-1, verified sound). The invariant:

> **At all times before resolution: `vault.amount == yes_mint.supply == no_mint.supply`.**

Mechanism:
- `mint_complete_set(amount)`: snapshot `vault_before` -> transfer USDC in -> `vault.reload()` -> `net_received = vault_after - vault_before` -> mint exactly `net_received` YES AND `net_received` NO.
- Because collateral is **USDC (fee-exempt, locked Phase-0 decision)**, `net_received == amount` always. **We keep the before/after sampling anyway** — it is the defense-in-depth the audit endorsed, costs nothing, and protects against any future collateral change. Do NOT shortcut it to `amount`.
- `redeem_complete_set(amount)`: burn `amount` YES AND `amount` NO from the redeemer -> transfer `amount` USDC out. Guard: pre-resolution only (`!resolved`).

**Acceptance gate**: a litesvm test that mints a complete set, asserts the 3-way equality, redeems it, and asserts the vault + supplies return to baseline. Plus boundary cases (amount=0 rejected, redeem more than held rejected, redeem after resolved rejected).

---

## `create_market` parameters

```
create_market(
    market_id: u64,            // caller passes; enforced == config.next_market_id (sequential, no gaps)
    streamer_ref: [u8; 32],    // hash/id of the streamer (off-chain identity commitment)
    metric: u8,                // MarketMetric enum-as-u8 (see below)
    target: u64,               // threshold for threshold markets (e.g. avg-viewers >= target)
    resolution_root: [u8; 32], // SNAPSHOTTED here (H-01) — the attention root this market resolves against
    resolution_root_seq: u64,  // snapshotted alongside (H-01)
    resolve_deadline_slot: u64,// hard finality deadline (H-01) — must be > created_slot
    dispute_window_slots: u64, // challenge window after resolution before settle is final (H-01)
)
```

- **Authority**: who can create a market? For Phase 1, gate on `config.admin` OR a creator allowlist. Keep it admin-gated for v1 (markets are curated; permissionless creation is a later decision). Document this as a Phase-1 trust choice.
- **market_id sequencing**: add a `next_market_id: u64` counter to `MarketsConfig` (or derive from a count) so market_ids are sequential and the PDA seed is collision-free. The audit (AC-5) said keep market_id in the seed — Phase 0 already does (`[MARKET_SEED, market_id.to_le_bytes()]`).
- **MarketMetric enum** (u8): define a small enum — e.g. `AvgViewers = 0`, `PeakViewers = 1`, `HoursWatched = 2`, `EngagementScore = 3`. Phase 1 only needs the value stored; resolution interpretation is Phase 3.
- **Resolution root at create**: this is the H-01 fix. The market binds to the root that exists NOW. If a real attention root isn't available yet at create time, allow `resolution_root = [0;32]` + a flag, but the cleaner v1 is to require a non-zero snapshot (the publisher must have published before a market opens). Pick: **require non-zero resolution_root** (simpler finality story); document that market creation depends on a published attention root.

---

## `initialize_market_tokens`

- Create `yes_mint` + `no_mint` as **Token-2022 mints, fee-free** (no TransferFeeConfig), 6 decimals (match USDC so 1 USDC <-> 1 share is clean), mint authority = a per-market PDA `[MINT_AUTH_SEED, market_id]`.
- Create `vault` as a USDC token account owned by a market PDA (the market or a vault-authority PDA).
- Set `market.tokens_initialized = true`. Guard: callable once (`!tokens_initialized`), only after `create_market`.
- The mint-authority PDA signs `mint_to` / `burn` in the complete-set rail.

---

## State changes (minimal — Phase 0 left room)

- `MarketsConfig`: add `next_market_id: u64` (carve from `_reserved` if a u64 fits, else append — Phase 0 gave a 64-byte reserve, so carve from it: no LEN change, no realloc). Update the LEN comment but NOT the LEN value if carving from reserve.
- `Market`: already has every field (Phase 0). Phase 1 just populates them.
- No new accounts beyond the per-market mints/vault (created by `initialize_market_tokens`).

---

## Errors (add to MarketsError)

`ZeroAmount`, `MarketAlreadyHasTokens`, `TokensNotInitialized`, `MarketResolved` (redeem-after-resolve), `InsufficientOutcomeBalance`, `InvalidMarketId` (non-sequential), `DeadlineInPast`, `ZeroResolutionRoot`.

---

## Out of scope for Phase 1 (explicit)

- The CPMM pool, `add_liquidity`, `swap` — Phase 2. (Complete-set rail alone gives fixed-par mint/redeem; it does NOT move odds.)
- `publish_attention_root`, `resolve_market`, `settle`, `resolve_override` — Phase 3.
- Permissionless market creation — v1 is admin-gated.
- Any merkle verification — Phase 3 (and the one-keccak-convention rule applies then).

---

## Verification checklist (the independent gate before commit)

1. `cargo build -p wzrd-markets` (host) + `cargo-build-sbf` — both clean.
2. Curve proptests still pass (Phase 0 regression).
3. Complete-set roundtrip test: mint -> assert `vault == yes_supply == no_supply == N` -> redeem -> assert back to baseline.
4. Boundary tests: amount=0 rejected; redeem > held rejected; redeem after resolved rejected; double `initialize_market_tokens` rejected; non-sequential market_id rejected.
5. PDA seed test: market / mint / vault / mint-auth PDAs derive as expected and are collision-free.
6. The before/after sampling is present in `mint_complete_set` (not shortcut to `amount`).
