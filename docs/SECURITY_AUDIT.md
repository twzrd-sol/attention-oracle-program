# Security Audit Report: Attention Oracle Program

**Date:** January 2026
**Scope:** `token_2022` program, `ccm_hook` transfer hook
**Commit:** Post-treasury controls implementation

---

## Executive Summary

A comprehensive review of the `attention-oracle-program` (Token-2022 and Transfer Hook) was conducted. The codebase demonstrates a high standard of security, employing established patterns for access control, state management, and arithmetic safety.

**Conclusion:** The system is well-architected for its intended operational phase. No critical or high-severity vulnerabilities were identified. The governance roadmap (Admin -> Multisig -> DAO) and treasury controls are documented and implemented correctly.

---

## Detailed Findings

### 1. Access Control & Authorization

**Status:** SECURE

The program enforces strict role-based access control using Anchor constraints.

* **Admin Role:** Critical instructions (`update_publisher`, `set_policy`, `set_paused`, `admin_withdraw`) explicitly verify `ctx.accounts.admin.key() == protocol_state.admin`.
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
| Stake Pool | `["stake_pool", mint]` |
| User Stake | `["user_stake", user, mint]` |
| Withdraw Tracker | `["withdraw_tracker", mint]` |

* **Subject ID:** Derived via `keccak256("channel:" + lowercase_name)`, ensuring consistency.
* **Validation:** All instruction contexts verify PDAs using Anchor's `seeds` and `bump` constraints.

### 3. Merkle Proof Verification

**Status:** SECURE

The merkle proof implementation follows industry best practices to prevent forgery.

* **Domain Separation:** Leaf hashing uses a unique domain prefix `TWZRD:CUMULATIVE_V2` to prevent second-preimage attacks using leaves from other trees.
* **Sorted Siblings:** The `verify_proof` function sorts sibling pairs (`if hash <= *sibling ...`), which renders the position of the leaf in the tree irrelevant and prevents mutability attacks.
* **Data Integrity:** The leaf hash commits to all critical claim parameters: `channel_config`, `mint`, `root_seq`, `wallet`, and `cumulative_total`.
* **Proof Length:** Capped at 32 elements (supports up to 2^32 leaves).

### 4. Transfer Hook Safety

**Status:** SECURE

The `ccm_hook` program implements the requisite checks to ensure it cannot be exploited or spoofed.

* **Caller Validation:** The hook verifies it is being invoked via CPI from the Token-2022 program by inspecting the `instructions` sysvar.
* **Fallback Safety:** The fallback instruction logs data but performs no state changes, mitigating risks if called directly.

### 5. Token Handling & Arithmetic

**Status:** SECURE

* **Fee-Aware Transfers:** The staking module correctly handles Token-2022 transfer fees by calculating the difference in vault balance before and after the transfer (`vault_balance_after - vault_balance_before`) rather than trusting the input amount.
* **Checked Math:** All arithmetic operations use Rust's checked methods (`checked_add`, `checked_mul`, etc.) or Anchor's wrappers to prevent overflows.
* **Precision:** Reward calculations use `u128` and high-precision constants (`REWARD_PRECISION = 1e12`) to minimize rounding errors.

---

## Observations & Findings

### 1. Treasury Rate Limits

**Severity:** Info

The `admin_withdraw` instruction enforces a strict limit of 50M CCM per transaction and 100M CCM per day (approx. 5% of supply).

* **Assessment:** This is a correctly implemented "circuit breaker." It does not prevent theft by a compromised admin key entirely but forces an attacker to drain funds over ~20 days, providing ample time for detection and intervention via `update_admin`.

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

1. **Monitoring:** Ensure active off-chain monitoring is in place for `TreasuryWithdrawn` events to utilize the 20-day detection window effectively.

2. **Multisig Transition:** Proceed with the planned transition of the admin authority to a Squads multisig as soon as operational stability is confirmed.

3. **Fallback Validation:** While low risk, adding the same caller validation (checking for Token-2022 program ID) to the `fallback` instruction in `ccm_hook` would align it with the main `transfer_hook` implementation.

4. **Consider Timelock:** For Phase 2, consider adding a timelock to `update_admin` (24-48h delay) to provide additional response time.

---

## References

* [SECURITY.md](/SECURITY.md) - Security policy and reporting
* [docs/TREASURY.md](/docs/TREASURY.md) - Treasury architecture and controls
* [programs/token_2022/src/](/programs/token_2022/src/) - Main program source
* [programs/ccm_hook/src/](/programs/ccm_hook/src/) - Transfer hook source
