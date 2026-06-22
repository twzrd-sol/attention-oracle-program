# Security Audit Report - wzrd-rails + attention-oracle (markets.rs port reference)

**Date**: 2026-06-21
**Auditor**: Automated Security Analysis (Plamen / Claude Opus 4.8, Core mode)
**Scope**: `programs/wzrd-rails/` (upgradeable, PRIMARY) + `programs/attention-oracle/src/instructions/markets.rs` (immutable, port reference)
**Language/Version**: Solana / Anchor 0.32.1, Token-2022
**Build Status**: wzrd-rails compiles (cargo build-sbf success). attention-oracle-token-2022 fails to compile (dead Pinocchio-era references in velocity_feed.rs) but is reference-only.
**Static Analysis Status**: Fender MCP unavailable; grep-based + manual analysis used. unified-vuln-db RAG unavailable (WebSearch fallback noted).
**On-chain verification**: CCM mint extension state verified live on mainnet (decisive for severity).

> **Canonical report (reconciled 2026-06-22).** This is the single authoritative
> wzrd-rails audit. It SUPERSEDES the two April 2026 drafts (the preliminary
> recon-only `AUDIT_REPORT.md` and `AUDIT_REPORT_PREPUSH.md`), which used a
> different, now-stale finding-ID scheme and have been removed. Those drafts'
> findings are accounted for in the "Excluded (prior-audit resolved)" section at
> the end (the `-pre` suffixed IDs). **The finding IDs here match the shipped fix
> PRs:** M-01 → PR #108 (admin cap-rotation), M-02 → PR #108 (lock_duration bound),
> M-03 → PR #109 (emission remainder-carry + StakePool realloc), L-01 → PR #108
> (Token-2022 ext allowlist), I-04 → PR #108 (PayoutWindow::space test constants).
> Re-audit (Phase 4) builds on this report.

---

## Executive Summary

This audit covers `wzrd-rails`, the upgradeable Solana program that the team intends to extend into a prediction-market settlement layer ("Polymarket/Kalshi on wzrd+dflow rails"), and `markets.rs` from the immutable attention-oracle program as the reference implementation that market logic would be ported from. The central question was not "is there a live exploit" but "what blocks a real-money streaming-attention prediction market, and is the foundation sound."

**The live `wzrd-rails` program is in good shape.** A prior audit pass found 0 Critical / 0 High and resolved its main concerns; this deeper pass (token-flow tracing, reward-accounting math, merkle-proof integrity, and on-chain verification) confirms there are **no Critical and no High severity issues in the live program**. The reward accumulator math (MasterChef-style) is conservation-correct, the four token vaults are donation-safe by construction, and the merkle claim paths are replay-safe and not forgeable. The most important on-chain check -- the CCM Token-2022 mint -- came back clean: it carries only a transfer-fee extension (no PermanentDelegate, no TransferHook), and both mint and freeze authorities are revoked, which **removes the only path to a Critical finding** on the live surface.

Three Medium issues remain in the live program, all fixable in a few lines each: an **admin-rotation bug that permanently bricks the per-window cap setter** (proven by an existing failing test, and which triggers on the first planned move to a multisig); an **unbounded lockup duration** that lets a single bad pool-initialization argument permanently lock all stakers' principal; and a **permissionless reward-crank truncation** that strands emissions at high TVL. None risk theft; all degrade availability or control.

**The prediction-market port is where the real-money risk concentrates, and it is design risk, not bug risk.** The `markets.rs` CTF construction is collateral-sound and mostly mechanical to port. But three decisions gate a safe launch: (1) **where resolution truth comes from** -- `wzrd-rails` has no on-chain attention oracle, and the reference resolves against a single-publisher merkle root whose compromise would drain markets; (2) **resolution finality** -- the reference can become permanently unresolvable due to a 4-deep root ring buffer, locking funds; and (3) **the product itself** -- `markets.rs` settles binary outcomes at par and does NOT provide moving odds, so a "long/short on future attention" index is a net-new construction, not a port. These are the items to resolve before any real money touches a ported market.

---

## Summary

| Severity | Count |
|----------|-------|
| Critical | 0 |
| High | 0 (live) / 2 (port design decisions) |
| Medium | 3 (live) + 3 (port) |
| Low | 6 (live) + 2 (port) |
| Informational | 4 (live/port) |

> Severity is split into **LIVE** (the upgradeable wzrd-rails program, real exposure today) and **PORT** (design decisions for the planned markets.rs port; the reference code is dead-in-binary, so these are launch-gating decisions rather than live bugs). The two "High" items are port design decisions (resolution source + finality), not live vulnerabilities.

### Components Audited

| Component | Path | Lines | Description |
|-----------|------|-------|-------------|
| wzrd-rails | programs/wzrd-rails/src/{lib.rs,state.rs,listen_payout.rs} | ~3,000 | Upgradeable CCM staking + merkle payout rail (PRIMARY) |
| markets.rs | programs/attention-oracle/src/instructions/markets.rs | 1,373 | CTF binary prediction-market reference (port source) |
| attention-oracle (global/merkle) | programs/attention-oracle/src/{global.rs,merkle_proof.rs} | ~1,200 | Resolution-root publisher + proof verification (port dependency) |

---

## High Findings

> Both High items are PORT design decisions for the planned prediction-market. The reference code is immutable/dead-in-binary; these are the decisions that gate a real-money launch, characterized at the severity they would carry once the port handles funds.

### [H-01] Prediction-market resolution finality: irreversible one-way + 4-deep ring-buffer race can permanently lock funds [PORT]

**Severity**: High (Impact: High / Likelihood: Medium)
**Location**: `programs/attention-oracle/src/instructions/markets.rs:648-656` (resolve gating), `constants.rs:22` (`CUMULATIVE_ROOT_HISTORY = 4`), `markets.rs:671` (resolved one-way)
**Confidence**: HIGH (code-trace with boundary values; reference code, dead-in-binary)

**Description**:
`create_market` stores `resolution_root_seq = S`. The global root config keeps only the **4 most-recent** roots in a ring buffer `roots[seq % 4]`. `resolve_market` requires the target root still be present:
```rust
require!(entry.seq == root_seq, RootTooOldOrMissing);  // markets.rs:656
```
If 4 or more newer roots are published before anyone resolves a given market, `roots[S % 4]` is overwritten and `roots[S % 4].seq != S`, so resolution reverts forever. Since `settle` and `sweep` are both gated on `resolved`, the market's CCM collateral is then **permanently locked**. Roots publish on a daily cadence, so any market not resolved within ~4 publish cycles of its target sequence locks automatically.

Separately, resolution is one-way (`resolved = true` set unconditionally, `require!(!resolved)` blocks re-resolution) with no dispute window. A valid-but-wrong upstream root settles the market irreversibly to the wrong side -- the merkle proof stops a *resolver* lying, but not a *wrong root* settling.

**Impact**:
- Permanent fund lock for any market not promptly resolved (the default outcome for slow or abandoned markets).
- Unequal YES/NO holders are fully stranded (cannot redeem -- needs equal pairs; cannot settle -- needs resolved). Equal-pair holders can still `redeem` (the only mitigant). Protocol residual/treasury sweep is permanently blocked.
- No correction path for a wrong-but-valid root.

**Recommendation**:
1. Snapshot the target root (`entry.root` + `seq`) into `MarketState` at create-time instead of re-reading the ring buffer at resolve-time. This eliminates the ring-buffer race entirely.
2. Add a resolution deadline plus an admin pro-rata recovery path for never-resolved markets.
3. Add a dispute/challenge window (or timelocked admin re-resolve) for the wrong-root case before settlement becomes final.

### [H-02] G3: wzrd-rails has no on-chain attention-resolution source -- the port's #1 design decision [PORT]

**Severity**: High (load-bearing trust decision for the entire market)
**Location**: `markets.rs:598-602, 643-667` (resolve binds the AO `GlobalRootConfig`); `wzrd-rails` -- absent entirely (grep-confirmed: zero `GlobalRootConfig`/`cumulative_total` readers)
**Confidence**: HIGH (gap confirmed by code search)

**Description**:
`markets.rs` resolves by verifying a merkle proof against the attention-oracle `GlobalRootConfig` PDA. `wzrd-rails` has no equivalent -- it cannot read attention truth on-chain. A port must choose a resolution source, and the two options carry very different risk:

- **Option (a): cross-program-read the live immutable AO `GlobalRootConfig`.** Small code, but: (1) rails MUST hard-validate the account is owned by the AO program AND is a genuine `GlobalRootConfig` -- an attacker passing a look-alike account they control would inject a fabricated root and resolve arbitrarily (the #1 footgun); (2) it inherits the H-01 ring-buffer finality bug, which rails cannot fix in code it does not own; (3) it depends on a still-live PDA of a program whose surrounding feature set was stripped.
- **Option (b): a new allow-listed attention-resolution-root publisher inside rails (RECOMMENDED).** Reuse the existing `publish_listen_payout_root` + publisher-allowlist pattern to publish into a rails-owned `AttentionRootConfig` PDA; resolve against that. Trust is a new semi-trusted in-house publisher bounded by allowlist + pause; no cross-program dependency; and H-01 can be fixed in the same self-owned construction.

**Impact**: This decision determines the trust model of the entire market. Option (a) is faster but inherits two serious problems (account-spoof + finality lock). Option (b) is more code but self-contained, upgradeable, and fixes H-01 in passing.

**Recommendation**: Choose option (b). If option (a) is chosen for speed, the AO account validation must be airtight (owner + discriminator + seed check) and the H-01 finality bug must be worked around at the rails layer. See also H-03 (publisher trust) and the chain analysis -- these three port findings interlock.

---

## Medium Findings

### [M-01] set_payout_admin rotation permanently bricks set_per_window_ccm_cap [LIVE, VERIFIED]

**Severity**: Medium
**Location**: `programs/wzrd-rails/src/lib.rs:313-329` (set_payout_admin), `1245-1266` (SetPerWindowCcmCap dual constraint), `1308-1318` (SetPayoutAdmin accounts -- cap_config absent)
**Confidence**: HIGH (1 agent confirmed, PoC: **FAIL** -- the intended behavior is broken; this is ground-truth proof of the bug)

**Description**:
The payout subsystem stores `admin` in three PDAs (`PayoutAuthorityConfig`, `PayoutCapConfig`, `PayoutVaultConfig`), all meant to be the same key. `set_payout_admin` rotates **only** `authority_config.admin` -- its accounts struct does not even include `cap_config` or `vault_config`. But `set_per_window_ccm_cap` dual-checks both:
```rust
constraint = authority_config.admin == admin.key()   // new admin passes
constraint = cap_config.admin == admin.key()          // still OLD admin -> NotAdmin
```
After a single `set_payout_admin(B)`: the new admin B fails the `cap_config` check, and the old admin A fails the `authority_config` check. **`set_per_window_ccm_cap` becomes permanently uncallable by anyone**, recoverable only by a program upgrade. `vault_config.admin` is written once at init and never read again -- a latent sibling defect.

This is a regression of the earlier hardening that made `cap_config.admin` load-bearing (the dual-check was added, but no rotation path for the second field).

**PoC Result**:
Executed `cargo test -p wzrd-rails --features localtest set_payout_admin_rotates_mutation_gate`. The test rotates the admin and asserts the new admin can set the cap. It **FAILS**:
```
panicked at core_loop.rs:1917: new payout admin can set cap: InstructionError(0, Custom(6115))
Program log: AnchorError caused by account: cap_config. Error Code: NotAdmin. Error Number: 6115.
test result: FAILED. 0 passed; 1 failed
```
This is `[POC-FAIL]`: the intended (and test-asserted) behavior does not work -- mechanical confirmation of the bug.

**Impact**:
Loss of the ability to adjust the per-window CCM cap (an economic safety bound on listen-payout emission) after the first admin rotation. Critically, rotation IS the documented response to a key compromise AND the planned go-live action (move the admin to a Squads multisig per the canary runbook) -- so the lockout triggers on the first planned governance migration, a near-certain event. No funds are stranded and claims/publish still work (they do not read `cap_config.admin`), hence Medium not High.

**Recommendation**:
Make `set_payout_admin` update all three admin fields atomically (add `cap_config` + `vault_config` as mutable accounts to the context), OR collapse to a single source of truth (read `authority_config.admin` everywhere and drop the other two admin fields/checks). The existing failing test already asserts the correct post-fix behavior, so it doubles as the regression test.

### [M-02] Unbounded lock_duration_slots at initialize_pool -> permanent principal lock [LIVE, VERIFIED]

**Severity**: Medium
**Location**: `programs/wzrd-rails/src/lib.rs:465` (stores value, no bound), `553-556` (lock_end_slot), `679-682` (unstake gate)
**Confidence**: HIGH (2 agents confirmed independently; PoC: code-trace complete with boundary values)

**Description**:
`initialize_pool` stores `lock_duration_slots: u64` verbatim with no minimum or maximum, and there is no post-init setter (grep-confirmed; the only other write is a default in a test helper). On stake:
```rust
user_stake.lock_end_slot = clock.slot.checked_add(pool.lock_duration_slots)?;  // lib.rs:553-556
```
and unstake gates on `require!(clock.slot >= user_stake.lock_end_slot, LockActive)` (lib.rs:679-682).

- `lock_duration_slots = u64::MAX` -> `checked_add` overflows -> stake reverts (self-blocking, no funds at risk).
- `lock_duration_slots ~= 1e18` -> stake **succeeds**, `lock_end_slot ~= 1e18` (~15 billion years at ~2 slots/s) -> unstake gate never satisfiable -> **all stakers' principal locked forever**. No `emergencyWithdraw` exists, so recovery is upgrade-only.

This is the **only admin-set scalar the program forgot to bound** -- `reward_rate_per_slot` (MAX_REWARD_RATE_PER_SLOT) and `per_window_cap_ccm` (MAX_PER_WINDOW_CAP_CCM) are both bounded.

**Impact**:
Day 1 ships a single global pool, so one bad or typo'd `initialize_pool` argument strands every subsequent staker's principal with no on-chain exit. Low likelihood (admin error/malice at pool creation), conditional-but-severe impact (principal lock). The upgrade authority caps the ceiling.

**PoC Result**:
Code-trace verified at exact lines (lib.rs:465 store with no bound; :553-556 checked_add; :679-682 gate; grep confirms zero `MAX_LOCK` constant anywhere). The logic is an absence-of-bound with no external state, so the trace is complete and mechanical.

**Recommendation**:
Bound `lock_duration_slots` at `initialize_pool` with a `MAX_LOCK_DURATION_SLOTS` (e.g. <= 90 days ~= 19.4M slots) and a sensible minimum, mirroring the existing `MAX_REWARD_RATE_PER_SLOT` / `MAX_PER_WINDOW_CAP_CCM` pattern. Optionally reconsider adding an emergency-withdraw escape hatch.

### [M-03] Permissionless update_pool truncates per-slot emissions to zero at high TVL [LIVE]

**Severity**: Medium
**Location**: `programs/wzrd-rails/src/state.rs:490-510` (accrue_rewards), `lib.rs:631-647` (update_pool, permissionless)
**Confidence**: HIGH (1 agent, code-trace with boundary values)

**Description**:
Reward accrual computes:
```
increment = floor(slots_elapsed * rate * 1e12 / total_staked)
```
with **no remainder carry**, while `last_update_slot` advances unconditionally. `update_pool` is permissionless. An attacker (or a naive keeper) cranking every slot forces `slots_elapsed = 1` on each call; once `total_staked > rate * 1e12`, every `increment` floors to 0 while `last_update_slot` still advances -> the `acc_reward_per_share` accumulator is **frozen** and that slot's emission is permanently burned. The floored emission is funded into `reward_vault` but never represented in the accumulator, so it strands there (no drain path -- consistent with the by-design absence of emergencyWithdraw).

With the live `reward_rate_per_slot = 1000` (canary runbook), the stall threshold is `total_staked > 1,000,000 CCM`. At 900,000 CCM, per-slot cranking burns ~10% of each window's emission; above 1M CCM it burns 100%. Dormant at launch (canary seeds ~1 CCM) but activates as TVL grows.

**Impact**: Denial-of-yield griefing (cheap, 1 tx/slot) and silent stranding of funded CCM in `reward_vault`. The system stays solvent (it under-distributes rather than over-distributes), so there is no theft -- but treasury value freezes and stakers under-earn.

**PoC Result**: Code-trace verified (floor division at state.rs:499-507; unconditional `last_update_slot` advance; permissionless `update_pool`). Not separately executed.

**Recommendation**: Carry the division remainder across accrue calls (cleanest -- makes accrual cadence-independent and also fixes the stranding), or gate accrual on a minimum slot-delta, or rate-limit/permission the standalone crank.

### [M-04] Cross-program merkle-stack incompatibility silently breaks every resolution proof if ported naively [PORT]

**Severity**: Medium (guaranteed integration break; the most dangerous because it fails silently)
**Location**: rails `listen_payout.rs:152-159` (solana_keccak_hasher, leaf + node domains) vs `attention-oracle/src/merkle_proof.rs:21-26` (sha3::Keccak256, leaf-domain-only, bare nodes); `state.rs:29` vs `markets.rs:22` (MAX_PROOF_LEN 16 vs 32)
**Confidence**: HIGH (code-trace; two distinct hash libraries and node conventions confirmed)

**Description**:
`wzrd-rails` and `attention-oracle` use **two different keccak libraries and two incompatible merkle node conventions**:
- rails: `solana_keccak_hasher`, nodes ARE domain-separated.
- AO/markets: `sha3::Keccak256` (a different crate), nodes are bare sorted-pair `keccak(a, b)` with NO node domain.

If the port merges `markets.rs` into rails without unifying the stack, a tree built with one convention and verified with the other recomputes a different root -> **every proof fails silently** (no compile error, only a generic proof mismatch). This passes any unit test that uses a single internally-consistent builder, and only breaks against cross-built proofs -- exactly the scenario in production. Additionally, `MAX_PROOF_LEN` is 16 in rails and 32 in markets/global, so a tree built for one depth produces proofs the other rejects; `CCM_DECIMALS = 9` is hardcoded in markets.rs and must match the rails Config; and `markets.rs` keys off a `ProtocolState{admin, publisher, paused, oracle_authority}` that rails' `Config{admin, ccm_mint, treasury_ccm_ata}` does not have.

**Impact**: Naive port -> every market permanently unresolvable and every reused merkle claim path broken, failing silently. This is the highest-leverage latent failure in the port.

**Recommendation**: Before building ANY tree for the port, write a one-page "rails merkle + collateral conventions" spec: ONE hash library, ONE node convention (recommend the rails domain-separated style), ONE `MAX_PROOF_LEN`, Token-2022-only outcome mints, and a `ProtocolState -> Config` field mapping. Lock a golden test vector and port against the spec.

### [M-05] Resolution root authenticated by a single semi-trusted publisher key -- the real-money trust crux [PORT]

**Severity**: Medium (port design) -- chains to High/Critical, see Chain Analysis. Tagged TRUSTED-ACTOR for the deployed immutable AO, where it is the accepted model.
**Location**: `programs/attention-oracle/src/instructions/global.rs:89-139` (publish_global_root); consumed by `markets.rs:653-667`
**Confidence**: HIGH (trust-model characterization)

**Description**:
The merkle proof guarantees a resolution value *matches the committed root* -- it does NOT guarantee the root reflects true attention data. `publish_global_root` accepts an opaque `root: [u8;32]` with no on-chain or oracle validation, gated only by `is_admin || is_publisher` (a single Ed25519 key each) plus monotonic sequencing. The `dataset_hash` is recorded and emitted but is advisory only -- nothing enforces root-to-dataset correspondence. A malicious or compromised publisher (or a publisher software bug) can commit an arbitrary root, and any signer then resolves any market to the chosen outcome (see the chain analysis), draining the funded side. There is no second source, no challenge window, and no multisig.

**Impact**: For the deployed immutable AO this is the accepted, operationally-separated trust assumption (downgraded accordingly). For the **port**, copying `publish_global_root` verbatim would put control of ALL real-money market resolutions behind a single key.

**Recommendation**: Do NOT inherit the single-key model in the port. Either (a) reference the immutable AO root cross-program with hard owner validation, or (b) make the rails publisher a multisig, add a dispute/challenge window before a root becomes resolvable, and ENFORCE (not just emit) a `dataset_hash` commitment. Decide this trust model explicitly before the port handles real funds.

---

## Low Findings

### [L-01] No Token-2022 extension allowlist on the CCM mint [LIVE, VERIFIED on-chain -- downgraded from Medium]

**Severity**: Low (defense-in-depth; downgraded from Medium after on-chain verification)
**Location**: `programs/wzrd-rails/src/lib.rs:1407, 1452, 1505, 1550, 1588, 1666` (all `ccm_mint` accounts -- none inspect extensions)
**Confidence**: HIGH (PROD-ONCHAIN verification)

**Description**:
The program validates exactly one Token-2022 extension (TransferFeeConfig, via before/after balance sampling on inbound transfers) and would silently honor any other extension on the CCM mint -- PermanentDelegate (a vault-drain primitive), TransferHook (would brick all CCM movement, since rails forwards no remaining_accounts), or DefaultAccountState=Frozen (would break first-time claim ATAs). This was initially flagged Medium-with-Critical-potential.

**On-chain verification (mainnet, CCM mint `Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM`)**: the live mint carries **only** `transferFeeConfig` (50 bps, max 5000 CCM). There is **no PermanentDelegate, no TransferHook, no DefaultAccountState**, and both **mintAuthority and freezeAuthority are revoked**. The worst-case drain/brick paths are therefore **not present on the live mint**, which removes the only path to a Critical finding on the live surface.

**Impact**: No live exposure. The residual is that the program has no on-chain assertion preventing a *future* CCM mint migration from silently introducing these extensions.

**Recommendation**: Add a config-validation assertion that the CCM mint carries no PermanentDelegate / TransferHook / DefaultAccountState (defense-in-depth for a future mint change).

### [L-02] CCM transfer-fee-config authority can strand committed payouts [LIVE, VERIFIED]

**Severity**: Low (bounded)
**Location**: `lib.rs:807-861` (claim_compensation), `945-1046` (claim_listen_payout)
**Confidence**: HIGH (PROD-ONCHAIN: authority live; CODE-TRACE: exploit path)

**Description**: All outbound payout paths send gross and let the recipient bear the live transfer fee, which is set by the external `transferFeeConfigAuthority`. On-chain verification confirms this authority is **live (not revoked)** (`vAbgvkjtVDYELfqh2xv1mbwz38WBvotTQ5hAkrPCXyP`). Between a one-shot commit (comp merkle root, or a listen window total) and a claim, that authority can raise the fee, delivering less while the replay marker is consumed. The `require!(actual_received > 0)` guard covers inbound only -- there is no outbound min-received gate.

**Impact**: Bounded -- `maximumFee` caps absolute loss at 5000 CCM/transfer, the authority is project-held, and the current fee is stable at 50 bps. A surprise fee hike could short or (at the cap) consume committed claims.

**Recommendation**: Add an outbound min-received delta gate so a surprise fee hike reverts the claim instead of silently shorting it.

### [L-03] Claim/unstake u128 overflow can brick large stakers [LIVE]

**Severity**: Low
**Location**: `state.rs:577-583` (claimable), `lib.rs:684-686` (unstake reads total_claimable before transfer)
**Confidence**: MEDIUM (code-trace with boundary values)

**Description**: A tiny-`total_staked` window at the MAX reward rate inflates the monotonic `acc_reward_per_share` to ~1.8e19 after ~18 slots. Then `amount * acc` overflows u128 for any `amount` near u64::MAX. Because `acc` is monotonic and `unstake` reads `total_claimable` *before* the principal transfer, the affected large staker can never claim, restake, or unstake -> permanent principal brick (upgrade-only recovery). Requires both a high accumulator (admin-reachable solo-stake at high rate) and a u64-scale stake.

**Recommendation**: A minimum-`total_staked` floor before emissions accrue fixes both this overflow and the previously-carried first-depositor reward-capture finding. Alternatively cap accumulator growth or lower `MAX_REWARD_RATE_PER_SLOT`.

### [L-04] set_paused can indefinitely freeze claims of already-funded windows [LIVE]

**Severity**: Low
**Location**: `lib.rs:266-277` (set_paused), `953` (claim gate)
**Confidence**: HIGH

**Description**: `claim_listen_payout` reverts if `auth_cfg.paused`, so a single `set_paused(true)` by the semi-trusted payout admin freezes all claimants of all already-committed, already-funded windows. No auto-unpause, no max-pause duration, no per-window carve-out. Within the intended emergency-halt bounds, but a compromised admin can weaponize it for an unbounded denial of already-allocated CCM. Reversible; no fund loss.

**Recommendation**: For real-money launch, consider a max-pause auto-expiry, excluding pre-pause funded windows from the claim halt, or moving the payout admin to a multisig/timelock.

### [L-05] Compensation proof length is unbounded [LIVE]

**Severity**: Low
**Location**: `lib.rs:807-825` (claim_compensation), `1096-1108` (verify_compensation_proof)
**Confidence**: HIGH

**Description**: `verify_compensation_proof` loops the entire `Vec<[u8;32]>` proof with no length guard, while `MAX_PROOF_LEN = 16` is enforced on the listen-payout path and 32 on the AO path. Practically bounded -- the claimer pays for their own transaction's compute, so there is no third-party or vault griefing; the only weaponization is the comp-root author building a deliberately deep tree, but that author already has the strictly-stronger power to simply omit a user's leaf.

**Recommendation**: Add `require!(proof.len() <= MAX_PROOF_LEN)` at the top of `claim_compensation` for consistency. Zero behavioral change for legitimate trees.

### [L-06] claim_listen_payout relies on CPI revert for underfunded vault (opaque, all-or-nothing) [LIVE]

**Severity**: Low (UX/observability)
**Location**: `lib.rs:1006-1033`
**Confidence**: HIGH

**Description**: Unlike `claim` (partial-pays and carries remainder) and `claim_compensation` (gates with a clean `CompensationUnavailable` error), `claim_listen_payout` has no `listen_payout_vault.amount >= leaf.amount_ccm` pre-check. The bookkeeping is sound and replay-safe -- the bitmap flip and `claimed_so_far` increment precede the transfer in the same instruction, so a revert rolls them back atomically. The only impact is an opaque token-program revert instead of a clean `PayoutVaultUnderfunded`, and all-or-nothing instead of partial pay.

**Recommendation**: Add a vault-balance pre-check for a clean error; optionally a partial-pay path matching the reward-claim behavior.

### [L-07] strategy.rs Kamino oracle accounts unpinned [PORT]

**Severity**: Low (port)
**Location**: `programs/attention-oracle/src/instructions/strategy.rs` (Kamino refresh_reserve CPI)
**Confidence**: MEDIUM (deferred oracle-integrity dimension)

**Description**: The Kamino program/reserve/market are address-pinned, but the four oracle accounts (Pyth/Switchboard/Scope) are bare `UncheckedAccount`s forwarded to Kamino's `refresh_reserve` -- safe only because Kamino validates oracle-to-reserve internally. Relevant only if strategy logic is ever ported.

**Recommendation**: Pin oracle addresses on `StrategyVault` at init when porting.

### [L-08] markets.rs Token-2022 fee charged on every market leg, no fee-exempt path [PORT]

**Severity**: Low (port economic)
**Location**: `markets.rs:353-363` (mint inbound), `561-571` (redeem outbound), `815-825` (settle outbound)
**Confidence**: HIGH

**Description**: Every CCM movement in a market incurs the 0.5% transfer fee with no fee-exempt path. A deposit-then-immediate-redeem loses ~2x the fee for zero activity, and a high-frequency market accrues a ~0.5-1.5% house edge to the CCM fee authority (not the market).

**Recommendation**: For the port, choose a fee-exempt collateral path (set the market vault PDA as a Token-2022 fee-exempt account, or use a non-fee collateral mint), or disclose the drag.

---

## Informational Findings

### [I-01] Compensation claim records gross amount while delivering net (off-chain over-count) [LIVE]

**Location**: `lib.rs:846-858`. `claimed.amount` and `CompensationClaimedEvent.amount` store the gross leaf amount while the user receives `amount * (1 - fee)`, with no `actual_received` companion (unlike `Staked`/`RewardPoolFunded`, which emit both). Because the comp claim crosses the on-chain/off-chain boundary (events feed indexers; the PDA is a public reconciliation source), off-chain reconcilers over-count compensation delivered by the cumulative fee. **Recommendation**: add `actual_received` to the event and store net.

### [I-02] comp_claimed replay-guard PDA seeds omit the Config key (forward-compatibility) [LIVE]

**Location**: `state.rs` -- `comp_claimed` seeds are `[b"comp_claimed", user]` only. Singleton-safe today (Config is a hard singleton, one compensation campaign ever), but a future second campaign would mis-block or cross-replay. **Recommendation**: if a second comp campaign is ever planned, add a campaign_id/config to the seed. (Note: recon's `state_variables.md` mis-documented these seeds as including config -- the code is correct.)

### [I-03] markets.rs introduces 5 new PDA seed namespaces; structs must extend append-only [PORT]

**Location**: `constants.rs:48-63`. The market seeds (`b"market"`, `b"market_vault"`, `b"market_yes"`, `b"market_no"`, `b"market_auth"`) have zero collision with rails seeds today (all market PDAs include `mint` + `market_id`). But `Config`/`StakePool`/`UserStake` have no `version`/`_reserved` field, so any field added during a port MUST append (never insert) to preserve discriminator-relative offsets. **Recommendation**: define market seed constants in rails' own state.rs, keep `mint`+`market_id` in seeds, and prefer a new MarketState-analog PDA over appending to existing structs.

### [I-04] Stale test constants for PayoutWindow::space (test hygiene, not a code bug) [LIVE]

**Location**: `programs/wzrd-rails/src/state.rs:750, 758`. Two unit tests assert `PayoutWindow::space(20) == 101` and `space(MAX) == 4194`, but the impl correctly returns 109 and 4202 (an 8-byte delta -- exactly the `claimed_so_far: u64` field added for the partial-pay fix). The impl is correct; the test constants are stale. This is positive confirmation that the partial-pay fix shipped. **Recommendation**: update the test constants to 109 / 4202 so CI is green.

---

## Port Readiness Verdict (the strategic answer)

The audit's purpose was to determine what it takes to ship a real-money streaming-attention prediction market. The answer, in three buckets:

**Mechanical (low-risk to port):** The `markets.rs` CTF 1:1-collateral construction is sound. The hypothesized "last-winner insolvency under Token-2022 fee" does NOT occur -- the vault holds post-fee collateral and mints exactly that, so it is never over-promised (verified by trace). Redeem/settle/sweep/close lifecycle guards are present and correct. The collateral math is portable as-is.

**Design decisions required (the gates):**
- **Resolution source (H-02):** rails has no on-chain attention oracle. Recommend an in-house allow-listed publisher (option b), not a cross-program read.
- **Finality (H-01):** the reference can permanently lock funds via the 4-deep ring buffer. Snapshot the target root at create-time; add a deadline and a dispute window.
- **Product shape (I-N / MR-5):** `markets.rs` settles binary outcomes at par -- it does NOT provide moving odds. A "long/short on future attention" index is a net-new CPMM/perp (already prototyped off-chain in the server), not a port. Decide binary-milestone markets vs a moving-odds index before building.

**Silent-failure hazard (M-04):** two keccak libraries and two merkle node conventions across the codebases will break every proof silently if merged naively. Unify the merkle stack before building any tree.

**Systemic (carried):** the single keypair that is simultaneously the Config admin and the upgrade authority (`2pHj...`) is a governance blocker per the team's own canary runbook; rotate to a multisig before launch.

**Minimum safe path to a real-money market:** (1) decide G3 resolution source (recommend in-house publisher), (2) fix finality (snapshot-at-create + deadline + dispute window), (3) decide product shape, (4) unify the merkle stack, (5) rotate the admin=upgrade keypair, then (6) fix the live Mediums (M-01 admin rotation, M-02 lock bound, M-03 truncation) and (7) add the Token-2022 extension allowlist (L-01).

---

## Priority Remediation Order

1. **M-01** (admin-rotation cap brick) -- LIVE, proven by failing test, triggers on first multisig migration. ~10-line fix. Do first.
2. **M-02** (unbounded lock) -- LIVE, day-1 blast radius. ~3-line bound. Do before any pool init at scale.
3. **M-03** (emission truncation) -- LIVE, activates as TVL grows. Remainder-carry fix.
4. **L-01** (Token-2022 extension allowlist) -- LIVE hardening; no current exposure but cheap insurance for a future mint change.
5. **H-01 / H-02 / M-05** (port: finality + resolution source + publisher trust) -- design decisions, resolve together before any real-money market. Interlocking (see Chain Analysis).
6. **M-04** (merkle-stack unification) -- port; must precede any tree construction.
7. **L-02..L-08, I-01..I-04** -- as scoped; L-02 (outbound fee gate) and I-04 (stale test) are quick wins.

---

## Chain Analysis (port)

The three High/Medium port findings interlock into the dominant real-money risk:

**CH-1 (the launch blocker): single-publisher arbitrary root -> faithful resolve -> settle-drain.** Because resolution is permissionless-but-merkle-faithful (anyone can resolve, but only to match the committed root), the property that makes resolution *safe* in isolation is exactly what makes a *malicious root* unstoppably executable. A compromised single publisher key (M-05) commits an arbitrary root; any signer then resolves the target market to the chosen side; `settle` drains the funded losing-side collateral. If the port ships a single-key publisher, this is **High, escalating to Critical** at scale (full pool drain, >2x attacker profit). It is fully defused by H-02 option (b) (in-house multisig publisher + dispute window + enforced dataset_hash).

**CH-2: cross-program AO-read inherits the finality bug.** Choosing H-02 option (a) imports the H-01 ring-buffer lock, which rails cannot fix in immutable AO code -> markets lock. Mitigated by option (b) + snapshot-at-create.

**CH-3: wrong merkle node convention -> all proofs fail silently -> total fund lock.** M-04 merged naively makes every resolution proof fail, the highest-leverage silent failure. Mitigated by unifying the stack before building.

No unexplored cross-class Medium+ finding pairs remain.

---

## Appendix A: Internal Audit Traceability

| Report ID | Internal Hypothesis | Chain | Verification | Severity |
|-----------|--------------------|-------|--------------|----------|
| H-01 | H-P1 (MR-3) | CH-2, CH-3 | CODE-TRACE | High (port) |
| H-02 | H-P2 (MR-4) | CH-2 | gap confirmed | High (port) |
| M-01 | H-4 (AC-1) | CH-4 | **POC-FAIL** | Medium |
| M-02 | H-2 (RM-4 + AC-2) | - | CODE-TRACE | Medium |
| M-03 | H-3 (RM-1 + RM-3) | - | CODE-TRACE | Medium |
| M-04 | H-P4 (MR-6 + MK-3) | CH-3 | CODE-TRACE | Medium (port) |
| M-05 | H-P3 (MK-7) | CH-1 | TRUSTED-ACTOR | Medium (port) |
| L-01 | H-1 (TF-2) | - | **PROD-ONCHAIN** | Low (was Medium) |
| L-02 | TF-3 | - | PROD-ONCHAIN + trace | Low |
| L-03 | H-5 (RM-2) | - | CODE-TRACE | Low |
| L-04 | H-6 (AC-3) | - | CODE-TRACE | Low |
| L-05 | H-7 (MK-2) | - | CODE-TRACE | Low |
| L-06 | H-8 (TF-6) | - | CODE-TRACE | Low |
| L-07 | H-P6 (AV-6) | - | deferred | Low (port) |
| L-08 | H-P7 (MR-2 + TF-5) | - | CODE-TRACE | Low (port) |
| I-01 | H-10 (TF-7) | - | CODE-TRACE | Info |
| I-02 | H-9 (AV-2) | - | CODE-TRACE | Info |
| I-03 | H-P8 (AC-5) | - | CODE-TRACE | Info |
| I-04 | test-health | - | executed | Info |

### Verified-correct (not findings)
CTF 1:1 solvency holds (TF-1/MR-1, last-winner-insolvency REFUTED); reward_debt re-anchor conserved; pool-update ordering correct; merkle leaf/node domain split is the correct second-preimage defense (AV-1/MK-1/MK-3); listen bitmap ordering correct (MK-4); cross-context replay bound across all three trees (MK-5); resolver is merkle-bound, cannot lie (AV-7/MK-6); one-time/monotonic guards sound (AC-6); markets lifecycle state machine sound (MR-7); rate cap enforced, admin cannot drain reward_vault.

### Excluded (prior-audit resolved, not re-reported)
M-01-pre (comp path), M-02-pre (unbounded reward_rate, now capped), M-04-pre (events added), I-02-pre, I-03-pre.
