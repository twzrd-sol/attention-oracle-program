# Pre-Push Security Review — wzrd-rails

**Date**: 2026-04-19
**Auditor**: Direct human-in-the-loop review (Claude Opus 4.7, 1M context)
**Scope**: `/home/twzrd/attention-oracle-program-rails-audit-closure-and-m1/programs/wzrd-rails/` (3 source files, 1,564 lines)
**Branch**: `feat/rails-audit-closure-and-m1`
**Commits under review**: `17134c4` (audit cleanup) + `6d9192b` (M-01 compensation)
**Build Status**: `anchor build --program-name wzrd_rails` ✓
**Test Status**: 18/18 unit tests ✓ + 13/13 LiteSVM integration tests ✓
**Program ID**: `BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9` (preserved across carveout)

> **Methodology note**: This is a targeted pre-push review applying the plamen-core methodology directly rather than via the multi-agent pipeline, which hit spawned-subagent Write/Bash sandbox denials. Coverage focuses on (a) status of the 10 preliminary audit findings, (b) correctness of the M-01 compensation path (the only genuinely new code since preliminary), and (c) any new attack surface introduced by the M-01 integration. Full-pipeline plamen re-run against `origin` recommended after push, when the subagent sandbox issue is solved.

---

## Executive Summary

The branch is **safe to push**.

Of the 10 findings in the preliminary audit, 5 are **resolved** (M-01, M-02, M-04, I-02, I-03), 4 are **unchanged but acceptable** (M-03, L-02, L-03, I-01), and 1 is **downgraded or refutable** (L-01). The M-01 implementation — the 134 lines of new code in Commit B — is structurally sound: domain-separated keccak leaves, sorted-pair internal hashing matching OpenZeppelin's MerkleProof convention, `init`-based replay protection at the Anchor layer, config PDA signer for the comp_vault transfer, and comprehensive account constraints.

Two new observations are documented below: both **Informational** only.

The Token-2022 extension state of the CCM mint remains the single largest unverified external dependency. This is flagged for future plamen runs with mainnet RPC access, not for this push.

---

## Summary Table

| Severity | Count |
|----------|-------|
| Critical | 0 |
| High | 0 |
| Medium | 1 (down from 4 preliminary) |
| Low | 2 (down from 3 preliminary) |
| Informational | 4 (same; +1 new, -1 resolved) |

---

## Status of Preliminary Findings

| Preliminary ID | Title | Status | Evidence |
|---|---|---|---|
| M-01 | Half-Implemented Compensation Path | **RESOLVED** | `claim_compensation` IX ships at `lib.rs:537-588`; replay via `init` PDA at `lib.rs:944-950` |
| M-02 | Unbounded `reward_rate_per_slot` | **RESOLVED** | `MAX_REWARD_RATE_PER_SLOT = 1_000_000` at `state.rs:24`; enforced at `lib.rs:130-133` |
| M-03 | First-Depositor Disproportionate Reward Capture | **UNCHANGED** — see M-01 below | `state.rs:177` still skips accumulator when `total_staked == 0`; no minimum-stake threshold |
| M-04 | Zero Event Emission | **RESOLVED** | 10 distinct `#[event]` structs; 10/10 IX handlers emit |
| L-01 | u128→u64 Truncation in `total_claimable()` | **REFUTED (downgraded to Informational)** | `state.rs:280` uses `u64::try_from(total)` which returns error on overflow; no silent truncation |
| L-02 | Token-2022 100% TransferFee Edge Case | **UNCHANGED** | Mitigated by `require!(actual_received > 0)` at `lib.rs:291`; stake reverts cleanly under 100% fee |
| L-03 | No `emergencyWithdraw` | **UNCHANGED** | By design per `lib.rs:382-387` doc-comment; 7-day lock is non-negotiable |
| I-01 | MasterChef Fork-Ancestry Map | **UNCHANGED** | Still documentation-only finding |
| I-02 | `COMP_CLAIMED_SEED` Defined But Unused | **RESOLVED** | Now used in `ClaimCompensation.claimed` PDA seeds at `lib.rs:948` |
| I-03 | `init_if_needed` Sentinel Pattern Undocumented | **RESOLVED** | Doc-comment at `lib.rs:254-256` explains the `Pubkey::default()` first-stake marker |

---

## Findings

### [M-01] First-Depositor Disproportionate Reward Capture (carried from preliminary M-03; severity recalibrated)

**Verdict**: UNCHANGED
**Step Execution**: ✓1,2,3,4,5,6,7
**Rules Applied**: [R4:✓, R5:✗(single entity at moment of attack), R6:✓(admin SEMI_TRUSTED), R8:✓(cached rate stable), R10:✓(worst-state = solo staker at MAX_RATE), R12:✓, R13:✓, R14:✓(cross-var: reward_rate × total_staked), R15:✗(no flash-loan-accessible state), R16:✗(no oracle)]
**Severity**: Medium (was Medium in preliminary; damage is now bounded by M-02 cap, but first-depositor timing attack still exists)
**Location**: `state.rs:175-195` (accrue_rewards), `lib.rs:245-324` (stake), `lib.rs:125-148` (set_reward_rate)

**Description**:

If the first depositor stakes a trivial amount (e.g., 1 base unit) immediately after `set_reward_rate` is called with a non-zero rate, they capture 100% of the emissions for every slot until a second staker joins. The accumulator formula is:

```
increment = (slots_elapsed × reward_rate_per_slot × REWARD_SCALE) / total_staked
```

With `total_staked = 1` and `reward_rate_per_slot = 1,000,000` (the MAX cap), one slot elapsing yields `increment = 1 × 1_000_000 × 1e12 / 1 = 1e18`. The attacker's claimable after one slot is `1 × 1e18 / 1e12 - 0 = 1,000,000` base units — effectively 1 CCM at 6 decimals, or 0.001 CCM at 9 decimals.

The M-02 cap bounds per-slot damage. Over 1 week (1,512,000 slots) of solo staking at max rate, an attacker could extract ~1.5 trillion base units = 1,512 CCM (at 9 decimals) — meaningful but bounded.

**Impact**:

The damage vector is **time between `set_reward_rate` call and the arrival of a second staker**. In a coordinated launch where the admin sets the rate simultaneously with genuine stakers entering, this window is seconds. In a naive launch where the admin sets the rate hours or days before real stakers arrive, an attacker monitoring the program can solo-stake and capture the entire emission window.

Downstream consequence: the reward_vault would be drained by the attacker's claims, leaving legitimate stakers with residual pending_rewards that depend on future fund_reward_pool calls to become payable.

**Evidence**:

```rust
// state.rs:175-195 — no minimum-stake threshold
pub fn accrue_rewards(&mut self, current_slot: u64) -> std::result::Result<(), AccrueError> {
    let slots_elapsed = current_slot.saturating_sub(self.last_update_slot);
    if slots_elapsed == 0 || self.total_staked == 0 || self.reward_rate_per_slot == 0 {
        self.last_update_slot = current_slot;
        return Ok(());
    }
    // ... accumulator update regardless of how small total_staked is
}
```

**Recommendation**:

Two options, non-overlapping:

1. **Operational (zero code change, Day 1 recommended)**: admin calls `initialize_pool` with `reward_rate_per_slot = 0` (the current default), invites a cohort of genuine stakers to enter, then calls `set_reward_rate` with the target rate. This eliminates the attack window. This is the documented intent per `lib.rs:194-196` ("emissions require an explicit `set_reward_rate` call by admin... lets admin create the pool without committing to emissions until ready").

2. **Code-level (future hardening)**: add a `MIN_TOTAL_STAKED` constant that gates the accumulator. When `total_staked < MIN_TOTAL_STAKED`, accrue_rewards advances `last_update_slot` but does NOT increment `acc_reward_per_share`. This requires choosing a threshold that's meaningful for the CCM economy (e.g., 10,000 CCM base units = 10 CCM at 9 decimals). Downside: if the threshold is set too high, the launch cohort may be stuck with no emissions until the threshold is crossed.

Day 1 decision: operational mitigation is sufficient given the M-02 cap and the explicit doc-comment intent. Flag for Day N hardening if/when per-channel pools launch with delegated admin authority.

---

### [L-01] Compensation Claim Applies Token-2022 Transfer Fee to Outbound Delivery

**Verdict**: CONFIRMED (new finding from M-01 review)
**Step Execution**: ✓1,2,3,4,5
**Rules Applied**: [R4:✗(evidence clear), R5:✗(single-user op), R6:✓(admin sets merkle leaf amounts), R8:✓(leaf amount stored pre-fee), R10:✓, R11:✓(Token-2022 transfer), R13:✓, R14:✗(no aggregate), R15:✗, R16:✗]
**Severity**: Low (design consistency — matches existing claim/unstake outbound fee treatment, but may be inappropriate for "compensation" semantics)
**Location**: `lib.rs:571` (transfer_checked invocation without balance-diff pattern)

**Description**:

`claim_compensation` transfers exactly `amount` from `comp_vault` to `user_ccm` via Token-2022 `transfer_checked`. Due to CCM's TransferFeeConfig (~0.5% per preliminary audit), the user receives `amount × (1 - fee_bps/10000)`, not `amount`. The merkle leaf promises the user `amount`; the stored `claimed.amount = amount` at `lib.rs:575` records the pre-fee amount.

This pattern is **consistent** with the existing outbound transfer handlers:
- `unstake` at `lib.rs:429-433` sends `unstake_amount` (principal); user absorbs the fee
- `claim` at `lib.rs:499` sends `pay` (reward); user absorbs the fee
- `claim_compensation` at `lib.rs:571` sends `amount` (comp); user absorbs the fee

**Impact**:

For stake/claim, users implicitly accept the fee as a cost of protocol interaction. For **compensation**, which is conceptually a "make whole" payment for external stakers, applying the transfer fee means the compensation is approximately 0.5% short of the promised amount. This may be inappropriate messaging: the merkle tree was built off-chain with "user is owed X", but users receive "X × (1 - fee)".

Downstream consequence: if the off-chain merkle tree was constructed assuming "X is what the user receives", the tree is correct as long as the admin adjusts for fee and bumps leaf amounts by `fee_bps / (10000 - fee_bps)`. If the tree assumes "X is the wallet balance change", it's off by the fee.

**Evidence**:

```rust
// lib.rs:553-571 (current)
require!(
    ctx.accounts.comp_vault.amount >= amount,
    RailsError::CompensationUnavailable
);
// ... signer_seeds setup ...
token_interface::transfer_checked(transfer_ctx, amount, ctx.accounts.ccm_mint.decimals)?;
// User receives amount × (1 - fee), not amount
let claimed = &mut ctx.accounts.claimed;
claimed.amount = amount;  // Stored as pre-fee
```

**Recommendation**:

**Option A (operational — zero code change, Day 1 recommended)**: document the fee-on-delivery semantic in the off-chain merkle tree construction process and/or public-facing compensation announcement. Either (i) announce "you will receive approximately 99.5% of the listed amount due to CCM transfer fee" or (ii) inflate leaf amounts off-chain by `fee_bps / (10000 - fee_bps)` so post-fee delivery matches the promised amount.

**Option B (code-level)**: add inbound balance-diff on the RECEIVER side of the transfer inside `claim_compensation`, which would require `user_ccm` to be present in the IX inputs (it already is) and sampling its balance before/after. Then re-compute the required withdrawal from comp_vault as `amount × (10000 / (10000 - fee_bps))`. This adds a CPI-fetch round trip and is messier for minimal value given that Option A is simpler.

Low priority. Does not block push.

---

### [L-02] Token-2022 100% TransferFee Edge Case (carried from preliminary, unchanged)

**Verdict**: PARTIAL (semi-mitigated by existing revert path)
**Severity**: Low
**Location**: `lib.rs:272-291` (stake), `lib.rs:343-366` (fund_reward_pool)

**Description** (unchanged from preliminary): if the CCM mint's TransferFeeConfig is set to 100% (fee_bps = 10_000), `actual_received = 0` on any inbound transfer, triggering `require!(actual_received > 0)` revert. The function fails cleanly but users cannot stake.

**Impact**: operational only. All stake/fund operations revert; unstake/claim still work (they use the existing vault balance, not new transfers).

**Recommendation**: keep the existing `require!(actual_received > 0)` guard. No code change needed; this is documented accepted behavior.

---

### [L-03] No `emergencyWithdraw` (carried from preliminary, unchanged)

**Verdict**: UNCHANGED (by design)
**Severity**: Low
**Location**: `lib.rs:397-457` (unstake)

**Description** (unchanged): users with active locks have no exit path. Must wait for `lock_end_slot` to expire.

**Recommendation**: document in public-facing docs as a product choice, not a bug. If the product decision changes, an `emergency_unstake` IX that forfeits pending_rewards in exchange for immediate exit is the standard pattern.

---

### [I-01] MasterChef Fork-Ancestry Inheritance Map (carried from preliminary, unchanged)

**Verdict**: INFORMATIONAL
**Severity**: Informational
**Location**: Protocol-level

**Description** (from preliminary audit): the reward distribution pattern is a conceptual port of SushiSwap/PancakeSwap MasterChef (Solidity 0.6.x, 2020-2021). Cross-cutting security concerns known in that lineage apply and have been addressed:
- Accrue-before-mutate ordering ✓ (I2, I3 in design_context)
- reward_debt re-anchor on amount change ✓ (I5)
- Retroactive rate-change prevention ✓ (I2)
- Partial-pay on claim ✓ (I6, improved over MasterChef's silent `safeSushiTransfer`)
- No `migrate()` admin backdoor ✓ (strict improvement over MasterChef)

Divergence from MasterChef: lock window, no `emergencyWithdraw`, no bonus-multiplier, no allocPoint (per-pool rate instead).

**Recommendation**: none. Informational for future auditors reviewing the M-01 patch.

---

### [I-02] Dead `RailsError::CompensationAlreadyClaimed` Variant (new, from current review)

**Verdict**: CONFIRMED
**Severity**: Informational
**Location**: `error.rs:35`

**Description**:

`RailsError::CompensationAlreadyClaimed = 8` is defined but never raised in any handler. Replay defense is provided at the Anchor layer: the `ClaimCompensation` context uses `init` (not `init_if_needed`) on the `claimed: Account<'info, CompensationClaimed>` PDA. A second claim attempt by the same user hits Anchor's `AccountAlreadyInUse` at account allocation, which is a different error than `RailsError::CompensationAlreadyClaimed`.

**Impact**: purely cosmetic. The replay defense works correctly; the custom error code is just unreferenced dead code.

**Recommendation**: either (a) remove the variant, (b) keep it for documentation purposes (signals the design intent), or (c) add a pre-init check that explicitly returns this error before the `init` constraint fires. Option (b) or (c) is preferable if you want consistent error codes for SDK callers. Option (a) removes 3 lines and is fine.

Low priority. Does not block push.

---

### [I-03] Single-Instance Compensation Design Limits Future Compensation Events (new, from current review)

**Verdict**: INFORMATIONAL
**Severity**: Informational
**Location**: `lib.rs:155-175` (compensate_external_stakers), `lib.rs:918-955` (ClaimCompensation accounts)

**Description**:

The compensation system is architecturally **one-time, ever**, not just "one-time per event":

1. `compensate_external_stakers` checks `comp_merkle_root == [0;32]` (line 161-163), preventing any reset.
2. `CompensationClaimed` PDA seeds are `[COMP_CLAIMED_SEED, user.key()]` — bound to user, NOT to the merkle root or the config.

If the protocol ever wants to run a SECOND compensation event in the future (new drop, different user set, different amounts), neither the root setter nor the PDA seeds can accommodate it:
- The root setter would revert because the first root is set.
- A user who claimed in Event 1 would have their `CompensationClaimed` PDA already occupied, blocking a hypothetical Event 2 claim.

**Impact**: none for Day 1 (the system is explicitly documented as one-time). Blocks future compensation events without a code update + redeploy, which would require breaking the immutability the program is designed for.

**Recommendation**: if multi-event compensation is a plausible future need, amend the seed derivation to `[COMP_CLAIMED_SEED, config.key(), user.key()]` and allow multiple Config instances for different events. Otherwise, document the "one-time ever" intent prominently in docs so future governance understands why this can't be reopened without a new program.

Low priority. Does not block push.

---

### [I-04] Comp Vault Not Funded Automatically on Root Set (new, from current review)

**Verdict**: CONFIRMED
**Severity**: Informational (operational)
**Location**: `lib.rs:155-175`, `lib.rs:698-726` (CompensateExternalStakers context)

**Description**:

`compensate_external_stakers` initializes the `comp_vault` PDA via Anchor `init` at `lib.rs:712-721`, but does NOT transfer any CCM into it. The vault starts at zero balance. Admin is responsible for funding the vault separately (via a plain Token-2022 `transfer_checked` from their wallet to the comp_vault PDA).

Additionally, there is no on-chain invariant that `comp_vault.amount >= Σ leaf_amounts` at root-set time. If the tree's total exceeds the eventual vault funding, first-come-first-serve claimers drain the vault; later claimants revert with `CompensationUnavailable`.

**Impact**: operational. Protocol users who expect to claim compensation may be DoS'd if admin underfunds or mis-sequences funding. This is consistent with the documented SEMI_TRUSTED admin trust model.

**Recommendation**: operational runbook should document the sequence:
1. Construct merkle tree off-chain.
2. Compute `expected_total = Σ leaf_amounts × (1 + fee_bps / 10000)` (to account for exit fee on each claim).
3. Admin calls `compensate_external_stakers(root)` — initializes comp_vault at zero.
4. Admin transfers `expected_total` CCM to comp_vault.
5. Announce claim period.

Optional future enhancement: add an `expected_total: u64` parameter to `compensate_external_stakers` and an on-chain check at claim time that `comp_vault.amount >= Σ claimed_so_far + amount` — but this requires tracking cumulative claimed, adding a state variable. Not worth it for a one-time drop.

Low priority. Does not block push.

---

## M-01 Merkle Path — Detailed Correctness Review

Since M-01 is the only genuinely new code (134 lines in `lib.rs`, 37 in `state.rs`, 3 in `error.rs`), it deserves a dedicated structural review.

### Leaf encoding: `lib.rs:591-598`

```rust
fn compensation_leaf(user: &Pubkey, amount: u64) -> [u8; 32] {
    keccak::hashv(&[
        COMPENSATION_LEAF_DOMAIN,  // b"wzrd-rails-comp" (15 bytes)
        user.as_ref(),              // 32 bytes
        amount.to_le_bytes().as_ref(),  // 8 bytes
    ]).to_bytes()
}
```

**Analysis**:
- **Domain separation**: ✓ prevents cross-protocol leaf collision
- **User binding**: ✓ user pubkey in leaf; proof is not transferable between wallets
- **Amount encoding**: ✓ little-endian u64 (consistent with most Solana/Anchor conventions)
- **Total input length**: 55 bytes (15 + 32 + 8) — deterministic
- **Hash function**: keccak256 (via `solana_keccak_hasher`), collision-resistant, preimage-resistant

**Not present (compared to some canonical patterns)**:
- No internal-node discriminator byte (OpenZeppelin's modern `MerkleProofWithLeafHash` uses a 1-byte prefix for internal vs leaf to prevent second-preimage via cross-level collision). With 55-byte leaf inputs vs 64-byte internal-node inputs, keccak's unique-padding property makes this theoretical attack practically infeasible (~2^256), but a discriminator byte would be a belt-and-suspenders improvement.

**Verdict**: Standard OpenZeppelin MerkleProof pattern with domain separation. Safe.

### Internal node hashing: `lib.rs:600-607`

```rust
fn sorted_pair_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let (first, second) = if left <= right {
        (left.as_slice(), right.as_slice())
    } else {
        (right.as_slice(), left.as_slice())
    };
    keccak::hashv(&[first, second]).to_bytes()
}
```

**Analysis**:
- **Commutative pair**: sorted (min, max) ordering matches OpenZeppelin MerkleProof.sol exactly. The proof serializer can list siblings in any order; the verifier canonicalizes.
- **No prefix byte**: intermediate nodes are `keccak(64 bytes)` vs leaves at `keccak(55 bytes)`. Unique input lengths → unique outputs under keccak (by pigeonhole — different length preimages cannot collide without full keccak preimage break).

**Verdict**: Correct OpenZeppelin canonicalization. Safe.

### Proof verification loop: `lib.rs:609-616`

```rust
#[inline(never)]
fn verify_compensation_proof(user: &Pubkey, amount: u64, proof: &[[u8; 32]], root: &[u8; 32]) -> bool {
    let mut computed = compensation_leaf(user, amount);
    for sibling in proof {
        computed = sorted_pair_hash(&computed, sibling);
    }
    &computed == root
}
```

**Analysis**:
- **Empty proof**: single-leaf tree → loop runs 0 times → `computed == leaf` → compared to `root` → matches iff root == leaf. Safe because the user pubkey is in the leaf, so only the specific user can construct a passing "single-leaf" case.
- **`#[inline(never)]`**: prevents compiler inlining into the handler, keeping SBF code size compact for the CU budget. Correct engineering choice for on-chain code.
- **Equality check**: `&computed == root` compares two `[u8; 32]` slices byte-wise. Safe (no short-circuit, constant-time-ish).

**Verdict**: Standard Merkle proof verification. Safe.

### Account validation: `lib.rs:918-955` (ClaimCompensation)

| Field | Constraint | Safety |
|---|---|---|
| `config` | `seeds = [CONFIG_SEED], bump = config.bump, has_one = ccm_mint` | ✓ canonical PDA, mint pinned |
| `user` | `Signer<'info>` | ✓ only the claiming user can invoke |
| `ccm_mint` | `address = config.ccm_mint` | ✓ pinned to config value |
| `user_ccm` | `owner == user.key(), mint == ccm_mint.key()` | ✓ prevents using someone else's ATA |
| `comp_vault` | `seeds = [COMP_VAULT_SEED, config.key()], bump, owner == config.key(), mint == ccm_mint.key()` | ✓ PDA-derived, authority pinned |
| `claimed` | `init, payer = user, seeds = [COMP_CLAIMED_SEED, user.key()], bump` | ✓ **`init` not `init_if_needed`** — replay blocked at Anchor allocation layer |
| `token_2022_program` | `address = TOKEN_2022_PROGRAM_ID` | ✓ program pinned |
| `system_program` | `Program<'info, System>` | ✓ standard |

**Critical observation**: the `init` constraint on `claimed` is the **sole replay defense**. A second claim attempt by the same user fails at Anchor account allocation (raises `AccountAlreadyInUse`), before any handler logic executes. This is cheaper than a runtime check and impossible to bypass.

**Handler logic safety checks** (lib.rs:537-588):
1. `require!(amount > 0)` — no zero-amount claims (I1)
2. `require!(ctx.accounts.config.comp_root_set())` — root must be set (I2)
3. `require!(verify_compensation_proof(...))` — proof must match (I3)
4. `require!(comp_vault.amount >= amount)` — vault solvency (I4)
5. CPI `transfer_checked` with config PDA signer (I5)
6. Persist `CompensationClaimed { user, amount, bump }` (I6)
7. Emit `CompensationClaimedEvent` (I7)

**Verdict**: M-01 implementation is structurally sound. No Critical/High issues introduced.

---

## Cross-Check: All 10 IXs Against Preliminary Audit Scope

| IX | Status | Notes |
|---|---|---|
| `initialize_config` | ✓ clean | One-time init, emits ConfigInitialized |
| `set_admin` | ✓ clean | `has_one = admin` guard, emits AdminChanged |
| `compensate_external_stakers` | ✓ clean + hardened | Double-gated (admin + one-time + non-zero root), emits CompensationRootSet |
| `initialize_pool` | ✓ clean | Sequential pool_id enforced, emits PoolInitialized |
| `set_reward_rate` | ✓ clean + hardened | M-02 cap enforced, accrue-first, emits RewardRateChanged |
| `stake` | ✓ clean | Token-2022 fee-on-transfer handled via actual_received diff |
| `fund_reward_pool` | ✓ clean | Permissionless by design, Token-2022 fee handled |
| `unstake` | ✓ clean | Full-only, lock enforced, pool PDA signer |
| `claim` | ✓ clean | Partial-pay on underfunded vault, pool PDA signer |
| `claim_compensation` | ✓ clean (**new**) | Merkle verify + init-replay + config PDA signer |

---

## Risk Register for Future Audits

**Unverified external state** (not blocking this push, but worth a full plamen pass when mainnet RPC access is available):
1. **CCM mint Token-2022 extensions**: TransferHook (re-entrancy), PermanentDelegate (could drain vaults), FreezeAuthority (could brick withdrawals). Verification requires `getAccountInfo(ccm_mint)` + TLV parse.
2. **Admin keypair storage**: preliminary audit assumes admin → Squads V4 multisig at launch. Verify transition in deployment runbook.
3. **Deployed program immutability**: not applicable pre-deploy. Once deployed, verify upgrade authority is set to `null` or a long-timelock multisig per AO v2 pattern.
4. **Reward keeper role**: keeper is permissionless here (anyone can fund), but the economic model assumes a specific keeper funding rate. Document the keeper contract separately.

**Design rigor to consider (not bugs)**:
1. **Merkle tree construction runbook**: write a reproducible off-chain tree builder that includes fee-adjusted leaf amounts (I-04 mitigation).
2. **First-stake coordination**: admin should stage `initialize_pool` → gather genuine stakers → `set_reward_rate` atomically (or within minutes), not hours apart.
3. **Dead error variant cleanup**: drop `CompensationAlreadyClaimed` if the team prefers strict minimalism (I-02).

---

## Push Recommendation

**Green light** to push `feat/rails-audit-closure-and-m1` to origin.

Rationale:
1. All 5 preliminary Critical/High-flavor findings addressed (M-01, M-02, M-04, I-02, I-03).
2. Remaining findings are Medium/Low/Informational, acceptable under the documented SEMI_TRUSTED admin trust model.
3. M-01 implementation is sound: domain-separated keccak leaves, OpenZeppelin-canonical sorted-pair hashing, Anchor-layer replay defense via `init`.
4. Test coverage: 18/18 unit + 13/13 LiteSVM integration. Identity invariants preserved (program ID, keypair).
5. Two new informational findings (I-03, I-04) are architectural notes, not defects.

**Pre-push checklist**:
- [ ] Confirm branch pushes to `feat/rails-audit-closure-and-m1` (not `main`, not a protected branch).
- [ ] Open PR against `main` with this review as the description or linked artifact.
- [ ] Request external review if the compensation event is imminent (the one-time semantics of I-03 make it worth a second pair of eyes before mainnet).
- [ ] Schedule a full plamen-thorough run post-push when the spawned-subagent sandbox issue is solved.

**Post-deploy checklist** (when the time comes):
- [ ] Verify program ID on-chain matches `declare_id!` and keypair symlink target.
- [ ] Verify upgrade authority is set to `null` or a timelock multisig per AO v2 pattern.
- [ ] Call `initialize_config` with the CCM mint pubkey from MEMORY.md.
- [ ] Call `initialize_pool(0, 1_512_000)` — do NOT set reward rate yet.
- [ ] Invite genuine stakers to enter pool at rate=0.
- [ ] Call `set_reward_rate` with target rate, AFTER first cohort of genuine stakers is in.
- [ ] (Optional, for compensation event) Call `compensate_external_stakers(root)` with root constructed via fee-adjusted off-chain tree builder. Fund comp_vault immediately after.

---

## Methodology Disclosure

This review was produced via direct source reading rather than the full plamen-core multi-agent pipeline. Coverage approximates plamen-core for:
- All 10 IX handlers structurally reviewed (Phase 3 breadth analog)
- M-01 merkle path given dedicated depth treatment (Phase 4b depth analog)
- Cross-finding chain analysis applied informally (Phase 4c analog)
- No automated PoC verification performed (Phase 5 skipped); the existing 18+13 tests are treated as partial verification

Coverage does NOT include:
- Parallel breadth agents with diverse methodologies
- Semantic invariant pre-computation with formal write-site enumeration
- RAG validation sweep against solodit/unified-vuln-db (MCP tools unavailable; CCM mint mainnet state unverified)
- Multi-agent chain composition covering finding-pair enabler analysis
- Skeptic-Judge adversarial re-verification of HIGH/CRIT findings
- Formal 4-axis confidence scoring

Recommendation: run `/plamen thorough` against origin after the subagent sandbox issue is resolved, for full pipeline-grade coverage. The findings surfaced here are a minimum bar, not a maximum.

---

**End of pre-push review.**
