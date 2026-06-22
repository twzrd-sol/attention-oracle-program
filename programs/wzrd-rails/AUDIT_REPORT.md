# Security Audit Report — wzrd-rails (PRELIMINARY)

**Date**: 2026-04-18
**Auditor**: Automated Security Analysis (Plamen Core, recon phase only)
**Scope**: `programs/wzrd-rails/` (3 files, 1203 lines)
**Language/Version**: Solana / Anchor 0.32.1 / Token-2022
**Build Status**: `anchor build --program-name wzrd_rails` — success (requires lockfile pin `borsh@1.5.7` + `proc-macro-crate@3.2.0`)
**Test Status**: 18/18 unit tests + 7/7 LiteSVM integration tests passing

> **⚠️ PRELIMINARY — RECON PHASE ONLY**
>
> This audit ran the reconnaissance phase only. The full plamen-core pipeline (depth analysis, chain analysis, PoC verification) did NOT complete due to a subagent sandbox constraint (Write tool denied for 2 of 4 recon subagents). Findings below are sourced from:
> - Recon Agent 1A (RAG meta-buffer, completed)
> - Recon Agent 1B (docs + external + fork ancestry, completed inline)
> - Recon Agent 2 (build + static analysis + tests, completed)
> - Recon Agent 3 (patterns + attack surface + templates, completed inline)
>
> **No PoC tests were written. No depth agents ran. No chain analysis. No verification.** Severities below are preliminary estimates and should be re-confirmed via a full pipeline pass with Write permissions enabled, or by manual review.

---

## Executive Summary

`wzrd-rails` is a Solana/Anchor CCM productivity rail implementing a MasterChef-style staking pool. Users stake CCM (Token-2022 with 0.5% TransferFeeConfig) into a shared pool, accrue rewards proportional to stake × time, and claim from a keeper-funded reward vault. The core code loop (initialize → stake → accrue → claim → unstake) is implemented and has 7/7 LiteSVM runtime proof. Compensation via merkle drop is designed but only half-implemented: the root-setter IX exists, but the corresponding `claim_compensation` IX does not.

Recon surfaced no Critical or High findings with obvious exploit paths. The highest-value preliminary concerns are (1) a **half-implemented compensation path** creating a stuck-funds risk if `compensate_external_stakers` is called and `comp_vault` is funded before the `claim_compensation` IX ships, (2) an **unbounded admin-settable `reward_rate_per_slot`** which under the stated SEMI_TRUSTED admin assumption allows emission-drain if admin stakes first and then raises the rate, and (3) **complete absence of event emission** across all 9 instruction handlers, impairing off-chain indexing and monitoring.

The code quality is above average for a new program: Token-2022 transfer-fee accounting is correctly handled via before/after vault reloads, the MasterChef `acc_reward_per_share` accumulator is invariant-preserving on rate changes (accrue runs before mutation), partial-pay claim is used to decouple principal withdrawal from reward-vault solvency, and `has_one` + explicit PDA constraints are consistently applied. These structural properties reduce the probability of the more severe vulnerability classes that pattern-match as "staking-protocol" concerns in RAG.

## Summary

| Severity | Count |
|----------|-------|
| Critical | 0 |
| High | 0 |
| Medium | 4 |
| Low | 3 |
| Informational | 3 |

### Components Audited

| Component | Path | Lines | Description |
|-----------|------|-------|-------------|
| wzrd-rails | `programs/wzrd-rails/src/lib.rs` | 730 | 9 instruction handlers + contexts |
| wzrd-rails | `programs/wzrd-rails/src/state.rs` | 431 | 3 state structs + accrue math + helpers |
| wzrd-rails | `programs/wzrd-rails/src/error.rs` | 42 | 10 error variants |

---

## Medium Findings

### [M-01] Half-Implemented Compensation Path — Stuck-Funds Risk [UNVERIFIED]

**Severity**: Medium
**Location**: `src/lib.rs` (compensate_external_stakers handler + CompensateExternalStakers context); `src/state.rs:16` (COMP_CLAIMED_SEED)
**Confidence**: HIGH (recon-only — not PoC-verified but the gap is a straightforward code-absence observation)

**Description**:
The protocol infrastructure for merkle-based external-staker compensation is partially deployed:
- `compensate_external_stakers(merkle_root)` IX exists: sets `config.comp_merkle_root` once, eagerly creates `comp_vault` PDA-owned Token-2022 account.
- `COMP_CLAIMED_SEED = b"comp_claimed"` is defined.
- Error codes `CompensationInvalidProof` and `CompensationAlreadyClaimed` exist.
- **However, `claim_compensation(amount, proof)` IX is NOT implemented.**

If an operator calls `compensate_external_stakers` with a real merkle root AND funds `comp_vault` with CCM before a future `claim_compensation` IX ships, those funds are stuck: the `comp_vault` PDA is owned by the `config` PDA, so no user IX can transfer out of it, and there is no admin-emergency-drain IX. Recovery requires a program upgrade (which is explicitly preserved by the HARD RULE in `project_solana_economy_plan.md`), but that is non-trivial and introduces timing risk.

**Impact**:
- If comp_vault funded: CCM deposited becomes inaccessible until a future program upgrade adds `claim_compensation`.
- No direct protocol-state corruption; other IXs continue to function.

**Recommendation**:
Either (a) implement `claim_compensation` before any production use of `compensate_external_stakers`, OR (b) add an explicit admin-gated `withdraw_comp_vault` recovery IX to allow funds to be clawed back if the claim IX is delayed. The codebase's own documentation (`session_handoff_apr18_wzrd_rails_scaffold.md`) already sequences M2/M3 (claim + tests) before any deployment, so the engineering plan already mitigates this — but in the interim, the pair (M1 shipped, M2 pending) should not be used on mainnet.

---

### [M-02] Unbounded `reward_rate_per_slot` Enables Emission Drain by Admin [UNVERIFIED]

**Severity**: Medium (adjusted from High by the SEMI_TRUSTED trust model: admin is currently a deployer keypair but targeted to migrate to Squads V4 3-of-5 multisig per the HARD RULE; exploit requires admin to violate stated trust assumptions)
**Location**: `src/lib.rs:105-116` (set_reward_rate handler)
**Confidence**: HIGH (the code has no upper bound; the exploit path is straightforward)

**Description**:
`set_reward_rate(pool_id, new_rate: u64)` accepts any `u64` value without any upper-bound validation. The admin can:
1. Stake their own CCM (via `stake`)
2. Call `set_reward_rate(0, u64::MAX)` or any large value
3. Wait N slots, the accumulator grows by `N * u64::MAX * 1e12 / total_staked`
4. Claim proportional rewards from `reward_vault`, draining it toward admin's proportional stake share

Because `set_reward_rate` correctly runs `accrue_rewards` before mutating the rate (preventing retroactive effect — a well-implemented invariant confirmed in `test_reward_rate_change_mid_period_no_retroactive_effect`), the attack only works going forward: admin cannot retroactively inflate past rewards. But going forward, the rate cap is the program's only defense, and there is none.

**Impact**:
- If admin acts maliciously: partial-to-full drain of `reward_vault` toward admin's stake share.
- Note: admin CANNOT drain `stake_vault` via this path — other stakers' principal is protected. Impact is bounded to reward-vault contents.

**Severity Adjustment**:
Per project-stated trust assumption, admin is SEMI_TRUSTED pre-Squads-migration. Under that assumption, this is Medium (abuse requires admin to violate trust). If this trust assumption is relaxed OR the deployer keypair is compromised, severity becomes High.

**Recommendation**:
Add an explicit upper-bound check: `require!(new_rate <= MAX_REWARD_RATE_PER_SLOT, RailsError::RewardRateTooHigh)`. Choose MAX_REWARD_RATE_PER_SLOT such that the worst-case annual emission is bounded (e.g., `2 * intended_annual_budget / slots_per_year`). This is a 2-line add + 1 new error variant + 1 unit test. **Small and obvious — good candidate for tomorrow's first work.**

---

### [M-03] First-Depositor Disproportionate Reward Capture When `total_staked = 0` [UNVERIFIED]

**Severity**: Medium
**Location**: `src/state.rs:117-137` (accrue_rewards) + `src/lib.rs:177-243` (stake)
**Confidence**: MEDIUM (pattern matches a known MasterChef lineage concern; not PoC-verified)

**Description**:
The `accrue_rewards` helper has a guard: if `total_staked == 0`, the accumulator does not increment but `last_update_slot` advances. This correctly prevents stakers from retroactively earning over the empty window. However, there is a different edge case: when `reward_rate_per_slot > 0` is set BEFORE any stake is entered, and then a very small stake enters, the next accrual computes:

```
increment = (slots_elapsed * rate * 1e12) / small_total_staked
```

If `small_total_staked` is 1 base unit (i.e., 0.000000001 CCM at 9 decimals), and `slots_elapsed` is non-trivial, the accumulator increments enormously, and that lone staker captures a disproportionate share of emissions.

This is distinct from the total_staked=0 guard (which prevents the increment entirely). The problematic case is `1 <= total_staked <= small number`.

**Impact**:
- First tiny staker can capture reward budget meant for a larger pool.
- Mitigated in practice if admin calls `set_reward_rate` AFTER initial stakes are entered.

**Recommendation**:
Day 1 operational fix (no code change): operator must `initialize_pool` → wait for initial stakes → THEN `set_reward_rate`. Code-level fix (more robust): add `MIN_STAKE_TO_ACCRUE` threshold in `accrue_rewards` guard, OR enforce `total_staked >= MIN_STAKE` in `set_reward_rate` when rate is non-zero. Document the operational ordering in deployment runbook.

---

### [M-04] Zero Event Emission Across All 9 Instruction Handlers [UNVERIFIED]

**Severity**: Medium (operational risk, not security — but flagged Medium because indexers/monitoring systems have no on-chain signal)
**Location**: All 9 IXs in `src/lib.rs`
**Confidence**: HIGH (complete absence of `emit!`, `emit_cpi!`, `sol_log_data`, and state-relevant `msg!` — confirmed by grep)

**Description**:
None of the 9 instructions emit events:
- `initialize_config`: silent
- `set_admin`: silent
- `set_reward_rate`: silent — admin could change economic parameter without on-chain observability
- `compensate_external_stakers`: silent — a high-consequence IX (one-time root set) has no event
- `initialize_pool`: silent
- `stake`: silent — user deposits are not indexable
- `fund_reward_pool`: silent
- `unstake`: silent
- `claim`: silent — reward payouts not indexable

**Impact**:
- Off-chain indexers must poll account state changes rather than consume events; brittle and slow.
- Monitoring systems cannot alert on admin state changes (rate changes, admin transfers, merkle root set) without polling.
- Users cannot verify their own on-chain activity via event logs.
- Not a security vulnerability in the strict sense, but degrades operational trustworthiness.

**Recommendation**:
Add `#[event]` structs for each state-changing IX and `emit!` at the end of each handler. Minimum set: `AdminChanged`, `RewardRateChanged`, `CompensationRootSet`, `PoolInitialized`, `Staked`, `RewardPoolFunded`, `Unstaked`, `Claimed`. This is ~50 lines of additive code with no risk surface change. **Small and obvious — good candidate for tomorrow's first work.**

---

## Low Findings

### [L-01] Potential u128→u64 Truncation in `total_claimable()` [UNVERIFIED]

**Severity**: Low (theoretical; unlikely under realistic emission rates)
**Location**: `src/state.rs` (total_claimable helper)
**Confidence**: MEDIUM

**Description**:
The `total_claimable` helper returns `u64` but internally computes `u128` values. If the accumulated claimable (fresh + pending) exceeds `u64::MAX` (≈ 1.8e19), the final `u64::try_from` would fail and return `AccrueError::Overflow`. This is a fail-safe (no silent corruption), but under realistic reward-rate parameters it should never trigger.

**Impact**: At realistic CCM supply and emission rates (millions of CCM/year), u64::MAX is unreachable. Only a concern if admin sets `reward_rate_per_slot` to very high values (see M-02).

**Recommendation**: No change needed given M-02 recommendation would bound rates. If M-02 is not adopted, consider returning `u128` from this helper to avoid the artificial ceiling.

---

### [L-02] Token-2022 100% TransferFee Edge Case [UNVERIFIED]

**Severity**: Low (economic, not security; requires mint-authority hostile action)
**Location**: `src/lib.rs:219-221` (stake's actual_received check)
**Confidence**: HIGH (logic observation)

**Description**:
The stake IX captures `balance_before`, performs `transfer_checked`, reloads, and computes `actual_received = balance_after - balance_before`. It then requires `actual_received > 0`. If the CCM mint-authority raises TransferFeeConfig to 100% (10000 bps), every transfer delivers zero to the destination. The user's CCM is deducted (as fee), the `require!(actual_received > 0)` fails, and the transaction reverts — so no corruption occurs. But the user's CCM is already gone via the transfer-fee path.

This is NOT wzrd-rails' fault per se — any Token-2022 consumer faces the same risk. Documenting it as a known operational risk is the right response.

**Recommendation**: Document in operational runbook that CCM mint-authority must not raise transfer fee to extreme values. Consider monitoring the CCM mint's TransferFeeConfig for unexpected changes.

---

### [L-03] No `emergencyWithdraw` — Lock-Active Users Have No Exit [UNVERIFIED]

**Severity**: Low (design choice; documented as intended)
**Location**: `src/lib.rs` unstake handler
**Confidence**: HIGH

**Description**:
Users who stake are locked for `DEFAULT_LOCK_SLOTS = 1,512,000` (7 days). There is no emergency-exit IX that allows forfeiting rewards in exchange for immediate principal withdrawal. If a user needs to exit urgently (regulatory, personal), they cannot.

**Impact**: UX friction in urgent-exit scenarios. Not a security vulnerability.

**Recommendation**: Day 1: document the 7-day lock clearly in user-facing UI. Future enhancement: add `emergency_unstake` IX that forfeits `pending_rewards` in exchange for immediate principal release, with a fee paid to `reward_vault` as a deterrent.

---

## Informational Findings

### [I-01] MasterChef EVM Fork-Ancestry Inheritance Map [UNVERIFIED]

**Severity**: Informational
**Location**: Entire program
**Confidence**: HIGH

**Description**:
Recon Agent 1B detected that wzrd-rails is a conceptual port of the SushiSwap/PancakeSwap MasterChef pattern (EVM-origin, not a Solana-native fork). Evidence: `acc_reward_per_share` (u128, 1e12-scaled), per-user `reward_debt`, `amount * acc / 1e12 - reward_debt` claim formula, `accrue_rewards ≡ updatePool`, per-pool `reward_rate_per_slot`. This is not a bug per se — it's a clear, well-understood reward-distribution pattern — but it means the 10-class MasterChef vulnerability set should be systematically reviewed:

1. ✓ Accrue-before-mutate ordering (implemented correctly)
2. ✓ reward_debt re-anchor on amount change (implemented correctly)
3. ✓ Retroactive rate change blocked (implemented correctly, runtime-proven)
4. ⚠ First-depositor / tiny-total-staked divisor attack — see M-03
5. ✓ Fee-on-transfer handling via diff-based credit (implemented correctly)
6. ✓ Reward-pool insolvency handled via partial-pay + pending_rewards
7. ⚠ No emergencyWithdraw — see L-03
8. ⚠ Rate-change front-running — admin increase → observed via mempool → pre-stake — see M-02 (partially)
9. ✓ Bump canonicalization (Anchor `bump = pool.bump` pattern)
10. ✓ init_if_needed on UserStake (safe — seeds are user-scoped)

**Recommendation**: No action. The protocol has already correctly handled 7/10 of the MasterChef lineage concerns. The remaining 3 surface as M-02, M-03, L-03.

---

### [I-02] `COMP_CLAIMED_SEED` Defined But Unused (Dead Code Until M2 Ships) [UNVERIFIED]

**Severity**: Informational
**Location**: `src/state.rs:16` (COMP_CLAIMED_SEED constant)
**Confidence**: HIGH

**Description**:
The constant `pub const COMP_CLAIMED_SEED: &[u8] = b"comp_claimed";` is defined but is not referenced by any IX handler. It's intended for the future `claim_compensation` IX's replay-defense PDA. Until that IX ships, the constant is dead code.

**Recommendation**: No action now — the constant is preparation for M2. Remove if M2 is never implemented. (Flagged only because it hints at the half-implemented gap called out in M-01.)

---

### [I-03] `init_if_needed` Sentinel Pattern Is Load-Bearing but Undocumented [UNVERIFIED]

**Severity**: Informational
**Location**: `src/lib.rs:186` (existing-vs-fresh detection in stake handler)
**Confidence**: HIGH

**Description**:
The stake handler uses `user_stake.user != Pubkey::default()` as the sentinel to distinguish "this UserStake was just freshly initialized by `init_if_needed`" from "this UserStake already existed from a prior stake." This pattern is correct (init_if_needed zeros the struct on creation; pre-existing accounts have the stored user value), but it relies on an implementation detail of Anchor's init_if_needed semantics. A future dev reading the code may not understand why the sentinel works, or may accidentally "improve" it into something incorrect.

**Recommendation**: Add a 2-line comment above the sentinel check explaining: `// init_if_needed zero-initializes a freshly-created account, so Pubkey::default() is our reliable "first stake" marker. Pre-existing UserStakes deserialize with the stored user value.` Zero risk, high clarity gain. **Small and obvious — good candidate for tomorrow's first work.**

---

## Priority Remediation Order

1. **M-04** (emit events): ~50 lines additive, zero risk surface change. Ship first.
2. **M-02** (reward_rate upper bound): ~5 lines + 1 error + 1 test. Ship second.
3. **I-03** (sentinel comment): 2-line doc add. Ship alongside M-04.
4. **M-03** (first-depositor): Document operational ordering in runbook; code-level fix optional.
5. **M-01** (compensation path): Do NOT use `compensate_external_stakers` on mainnet until `claim_compensation` IX lands (per existing project plan).
6. **L-01, L-02, L-03**: Defer to post-launch hardening pass.

---

## What This Audit Did NOT Cover

- **Depth-level analysis** (Phase 4b of the pipeline): 8+ domain-specialized agents that would trace state mutations across cross-function boundaries, probe edge cases with concrete values, and verify CPI side effects. Not run.
- **Chain analysis** (Phase 4c): finding-to-finding composition — e.g., can M-02 + M-03 compound into something worse than either alone? Not run.
- **PoC verification** (Phase 5): every finding above is marked UNVERIFIED. No integration tests were written to prove exploitability. The claimed severities are pattern-match estimates, not mechanical proof.
- **RAG validation** (Phase 4b.5): `unified-vuln-db` MCP was not registered in this session; no historical precedent search was completed. Findings may duplicate or miss patterns already known in the Solana DeFi audit corpus.
- **Fuzzing**: Trident was not installed; no invariant-fuzz campaign was run.
- **Compensation IXs (M2/M3)**: intentionally out of scope since they are not yet implemented. Will need a dedicated audit pass when they ship.

---

## Recommended Next Actions

### Tonight (stop after this)
- Nothing. Sleep.

### Tomorrow morning (all "small and obvious" per the project's do-not-open-new-branch-tonight directive)
1. **M-04 + I-03 in one commit** (emit events + sentinel comment): shortest path to shipping two finding resolutions in one review cycle
2. **M-02 in a separate commit** (reward_rate upper bound): isolated change, add one new error variant + one LiteSVM test asserting the bound fires

### Tomorrow afternoon or next session
3. **Rerun `/plamen core` with Write permissions fixed for subagents**: the full depth + verification pipeline is the real value add; recon alone missed several vulnerability classes that depth agents would surface.
4. **Then implement M2 + M3** (claim_compensation + merkle tests) as originally planned. The M-01 gap closes when these ship.

### Before mainnet
5. **Run `/plamen thorough`** (~35-95 agents, multi-pass depth, fuzzing, Skeptic-Judge adversarial verification)
6. **Install Trident and run invariant fuzz campaign** against the core loop + merkle flow
7. **External audit** (minimum 2 per the HARD RULE's launch-immutability criteria)

---

## Appendix A: Audit Pipeline Execution Trace

| Phase | Status | Notes |
|-------|--------|-------|
| Phase 1: Recon — Agent 1A (RAG) | ✓ COMPLETE | meta_buffer.md written; RAG_TOOLS_AVAILABLE=false (unified-vuln-db MCP not registered) |
| Phase 1: Recon — Agent 1B (Docs) | ⚠ DEGRADED | Analysis complete; Write tool sandbox-denied; content persisted inline by orchestrator |
| Phase 1: Recon — Agent 2 (Build) | ✓ COMPLETE | All 9 artifacts written |
| Phase 1: Recon — Agent 3 (Patterns) | ⚠ DEGRADED | Analysis complete; Write + Bash-mkdir sandbox-denied; content persisted inline by orchestrator |
| Phase 2: Instantiation | NOT RUN | Halted due to sandbox wall |
| Phase 3: Breadth | NOT RUN | |
| Phase 4a: Inventory | NOT RUN | |
| Phase 4a.5: Semantic Invariants | NOT RUN | |
| Phase 4b: Depth Loop | NOT RUN | Would have hit same sandbox wall (8+ Write-requiring agents) |
| Phase 4b.5: RAG Sweep | NOT RUN | |
| Phase 4c: Chain Analysis | NOT RUN | |
| Phase 5: Verification (PoC) | NOT RUN | |
| Phase 6: Report | DEGRADED (this document) | Written by orchestrator directly; standard pipeline tier-writers did not run |

**Pipeline Execution Time**: ~6 minutes (recon phase only)
**Agents Spawned**: 4 of ~33 planned
**Approximate API Cost**: ~$12 of the ~$84 estimated

**Scratchpad location**: private local audit workspace

---

*This preliminary report supersedes no prior audit. When the full pipeline runs successfully, replace this file with the tier-writer-generated AUDIT_REPORT.md.*
