# Security Audit Report: Attention Oracle Program

**Date:** January 2026
**Scope:** `token_2022` program
**Commit:** Post-treasury controls implementation

---

## Executive Summary

A comprehensive review of the `attention-oracle-program` Token-2022 program was conducted. The codebase demonstrates a high standard of security, employing established patterns for access control, state management, and arithmetic safety.

**Conclusion:** The system is well-architected for its intended operational phase. No critical or high-severity vulnerabilities were identified. The governance roadmap (Admin -> Multisig -> DAO) and treasury controls are documented and implemented correctly.

---

## Detailed Findings

### 1. Access Control & Authorization

**Status:** SECURE

The program enforces strict role-based access control using Anchor constraints.

* **Admin Role:** Critical instructions (`update_publisher`, `set_paused`, `update_admin`, `harvest_fees`) explicitly verify `ctx.accounts.admin.key() == protocol_state.admin`.
* **Publisher Role:** Separation of duties is implemented. Daily operations like `publish_cumulative_root` can be delegated to a publisher key, keeping the admin cold.
* **Admin Transfer:** `update_admin` prevents lockout by validating the new key is not `Pubkey::default()`.
* **Permissionless Init:** `InitializeMintOpen` allows any user to create a protocol instance for their own mint, correctly setting the caller as admin.

### 2. PDA Derivation & Validation

**Status:** SECURE

Program Derived Addresses (PDAs) use deterministic seeds and canonical bumps to prevent collision and usurpation.

| PDA | Seeds |
|-----|-------|
| Protocol State | `["protocol", mint]` |
| Fee Config | `["protocol", mint, "fee_config"]` |
| Channel Config | `["channel_cfg_v2", mint, subject_id]` |
| Claim State | `["claim_state_v2", channel_config, wallet]` |

* **Subject ID:** Derived via `keccak256("channel:" + lowercase_name)`, ensuring consistency.
* **Validation:** All instruction contexts verify PDAs using Anchor's `seeds` and `bump` constraints.

### 3. Merkle Proof Verification

**Status:** SECURE

The merkle proof implementation follows industry best practices to prevent forgery.

* **Domain Separation:** Leaf hashing uses a unique domain prefix `TWZRD:CUMULATIVE_V2` to prevent second-preimage attacks using leaves from other trees.
* **Sorted Siblings:** The `verify_proof` function sorts sibling pairs (`if hash <= *sibling ...`), which renders the position of the leaf in the tree irrelevant and prevents mutability attacks.
* **Data Integrity:** The leaf hash commits to all critical claim parameters: `channel_config`, `mint`, `root_seq`, `wallet`, and `cumulative_total`.
* **Proof Length:** Capped at 32 elements (supports up to 2^32 leaves).

### 4. Token Handling & Arithmetic

**Status:** SECURE

* **Token-2022 Fees:** Transfer fees are enforced by the native Token-2022 Transfer Fee Extension. The on-chain program provides harvesting of withheld fees.
* **Checked Math:** All arithmetic operations use Rust's checked methods (`checked_add`, `checked_mul`, etc.) or Anchor's wrappers to prevent overflows.
* **Precision:** Reward calculations use `u128` where needed to minimize rounding and overflow risk.

---

## Observations & Findings

### 1. Treasury Outflows

**Severity:** Info

The current public program interface does **not** expose an `admin_withdraw` instruction. Treasury outflows occur via cumulative claims only.

* **Assessment:** This reduces the number of explicit "treasury drain" paths in the program interface. Note that the program is upgradeable; the upgrade authority remains the primary governance/control surface.

### 2. Admin Authority

**Severity:** Low

The `update_admin` instruction executes immediately without a timelock.

* **Context:** This allows for rapid response to key compromise.
* **Mitigation:** The project roadmap explicitly plans a transition to Multisig (Phase 2) and DAO governance (Phase 3).

### 3. Publisher Compromise Risk

**Severity:** Info

A compromised publisher key can publish malicious merkle roots.

* **Impact:** Users can only claim amounts included in the proof (no arbitrary minting). Worst case is inflation via fabricated `cumulative_total` values.
* **Mitigation:** Pause mechanism halts claims during incident response. Publisher key can be rotated by admin.

### 4. Claim Receipts

**Severity:** Low

Claims emit events but do not store a specific "receipt" account on-chain.

* **Impact:** Disputes regarding off-chain indexing must rely on replaying historical blocks to find the `CumulativeRewardsClaimed` event. This is a standard gas-optimization trade-off.

### 5. Dependency Management

**Severity:** Info

Several low-severity alerts related to dev-dependencies (`ed25519-dalek`, `curve25519-dalek`, `atty`) have been dismissed as they are transitive dependencies of pinned upstream crates (Solana SDK 2.3.x / Anchor 0.32.x) and are not used in the on-chain program runtime.

---

## Vulnerability Checklist

| Vulnerability | Status | Notes |
|--------------|--------|-------|
| Missing signer check | SAFE | All admin/publisher functions require `Signer` |
| Missing owner check | SAFE | Anchor's `Account<>` validates owner |
| Account substitution | SAFE | PDAs validated via seeds, constraints enforce expected accounts |
| Arithmetic overflow | SAFE | All math uses checked operations |
| Type confusion | SAFE | Anchor discriminators prevent |
| Reinitialization | SAFE | `is_initialized` flag + `version == 0` checks |
| Closing accounts | N/A | No account closing implemented |
| Missing rent exemption | SAFE | `init` constraint handles rent |
| PDA bump canonicalization | SAFE | Bump stored and validated |
| CPI privilege escalation | SAFE | Signer seeds properly scoped |

---

## Recommendations

1. **Monitoring:** Ensure active off-chain monitoring is in place for cumulative claims (`CumulativeRewardsClaimed`) and fee harvesting (`FeesHarvested`). Consider alerting on unusual claim volume or unexpected harvesting activity.

2. **Multisig Transition:** Proceed with the planned transition of the admin authority to a Squads multisig as soon as operational stability is confirmed.

3. **Verified Releases:** Publish a verifiable build + tag for each mainnet deployment and keep `docs/LIVE_STATUS.md` up to date to reduce ambiguity for integrators.

4. **Consider Timelock:** For Phase 2, consider adding a timelock to `update_admin` (24-48h delay) to provide additional response time.

---

## References

* [SECURITY.md](/SECURITY.md) - Security policy and reporting
* [docs/TREASURY.md](/docs/TREASURY.md) - Treasury architecture and controls
* [programs/token_2022/src/](/programs/token_2022/src/) - Main program source
