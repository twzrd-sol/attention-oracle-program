# Security Audit Report: Attention Oracle Protocol

**Date:** January 2026 (updated February 2026)
**Scope:** `token_2022` (Oracle) + `channel_vault` (Vault) programs
**Commit:** `cff6981` (initial) / `482828b` (post-launch improvements)
**Framework:** Anchor 0.32.1 / Solana SDK 2.3.x

---

## Executive Summary

A comprehensive review of both on-chain programs in the Attention Oracle Protocol was conducted. The audit covers the Oracle program (`token_2022`) — cumulative claims, channel staking, governance, and transfer-fee management — and the Vault program (`channel_vault`) — an ERC4626-style liquid staking wrapper with compound, redeem, and emergency reserve mechanics.

**Conclusion:** No critical or high-severity vulnerabilities were identified. Two medium-severity items remain open; one has been mitigated via Squads multisig (see [Open Items](#open-items)). Both programs enforce strict access control, checked arithmetic, PDA validation, and pause/shutdown controls. Admin governance has transitioned from single-key to Squads v4 multisig (Phase 2 complete).

**Test coverage:** 89 tests across 3 test suites (36 vault + 31 staking + 22 cumulative), all passing.

---

## Program Overview

### Oracle (`token_2022`)

| Area | Description |
|------|-------------|
| Cumulative Claims | Merkle-proof-based reward distribution with domain-separated leaf hashing |
| Channel Staking | MasterChef-style yield with soulbound NFT receipts and boost multipliers |
| Governance | Fee harvesting from Token-2022 withheld transfer fees |
| Admin | Publisher delegation, pause/unpause, admin transfer, reward rate management |

### Vault (`channel_vault`)

| Area | Description |
|------|-------------|
| Deposits | CCM deposits with transfer-fee accounting, vLOFI share minting |
| Compounding | Permissionless crank: stakes pending deposits into Oracle, rolls over expired locks |
| Redemption | Queued withdrawal (no penalty) or instant redeem (20% penalty) |
| Emergency | Admin emergency unstake, reserve funded by penalties (5% NAV cap) |
| Lifecycle | Initialize, pause/resume, sync oracle position, close vault |

---

## Oracle Program — Detailed Findings

### 1. Access Control & Authorization

**Status:** SECURE

| Role | Enforcement | Instructions |
|------|-------------|-------------|
| Admin | `admin.key() == protocol_state.admin` | `update_publisher`, `set_paused`, `update_admin`, `harvest_fees` |
| Admin (staking) | `admin.key() == ADMIN_AUTHORITY` | `set_reward_rate`, `admin_shutdown_pool`, `migrate_user_stake`, `migrate_stake_pool` |
| Publisher | `publisher.key() == protocol_state.publisher` | `publish_cumulative_root` |
| Permissionless | No authority check | `InitializeMintOpen`, `claim_cumulative`, `stake_channel`, `unstake_channel`, `claim_channel_rewards`, `emergency_unstake_channel` |

* **Admin Transfer:** `update_admin` prevents lockout by validating the new key is not `Pubkey::default()`.
* **Publisher Delegation:** Daily operations can be delegated to a publisher key, keeping the admin cold.
* **Permissionless Init:** `InitializeMintOpen` allows any user to create a protocol instance for their own mint, correctly setting the caller as admin.

### 2. PDA Derivation & Validation

**Status:** SECURE

| PDA | Seeds | Program |
|-----|-------|---------|
| Protocol State | `["protocol", mint]` | Oracle |
| Fee Config | `["protocol", mint, "fee_config"]` | Oracle |
| Channel Config | `["channel_cfg_v2", mint, subject_id]` | Oracle |
| Claim State | `["claim_state_v2", channel_config, wallet]` | Oracle |
| Stake Pool | `["channel_pool", channel_config]` | Oracle |
| User Stake | `["channel_user", channel_config, user]` | Oracle |
| NFT Mint | `["stake_nft", stake_pool, user]` | Oracle |
| Stake Vault | `["stake_vault", stake_pool]` | Oracle |

* **Subject ID:** Derived via `keccak256("channel:" + lowercase_name)`, ensuring consistency.
* **Validation:** All instruction contexts verify PDAs using Anchor's `seeds` and `bump` constraints. Bumps are stored on first init and re-verified on every access.

### 3. Merkle Proof Verification

**Status:** SECURE

* **Domain Separation:** Leaf hashing uses `TWZRD:CUMULATIVE_V2` prefix to prevent second-preimage attacks.
* **Sorted Siblings:** `verify_proof` sorts sibling pairs, rendering leaf position irrelevant.
* **Data Integrity:** Leaf commits to `channel_config`, `mint`, `root_seq`, `wallet`, and `cumulative_total`.
* **Proof Length:** Capped at 32 elements (2^32 leaves).
* **Deduplication:** Single `keccak_hashv` implementation in `merkle_proof.rs`, imported by `channel.rs`.

### 4. Staking Security

**Status:** SECURE

* **MasterChef Accumulator:** `acc_reward_per_share` updated lazily per interaction via `update_pool_rewards()`. Called before any pool total modification.
* **Soulbound NFT Receipts:** Token-2022 `NonTransferable` extension. Minted on stake, burned on unstake. Prevents stake double-counting.
* **Boost Multiplier:** Lock duration maps to `multiplier_bps`; weighted stake = `amount * multiplier_bps / BOOST_PRECISION`.
* **Principal Protection:** `claim_channel_rewards` enforces `vault_balance - total_staked >= pending`. Claims cannot consume staked principal.
* **Pending Rewards Block:** `unstake_channel` blocks exit if user has claimable rewards (exception: pool shutdown or underfunded).
* **Emergency Unstake:** 20% penalty, lock-active required. Pool shutdown waives lock and penalty.

### 5. Pause & Shutdown Controls

**Status:** SECURE

* **Protocol Pause:** `protocol_state.paused` halts `stake_channel`, claims, and publishing. Admin can still publish during pause.
* **Pool Shutdown:** `admin_shutdown_pool` sets `is_shutdown = true`, zeroes `reward_per_slot`, waives lock requirements. Users can exit penalty-free.

### 6. Token Handling & Arithmetic

**Status:** SECURE

* **Token-2022 Fees:** 0.5% transfer fee enforced by Token-2022 extension. On-chain program provides fee harvesting (batched, 30 accounts per tx).
* **Checked Math:** All arithmetic uses `checked_add`, `checked_sub`, `checked_mul`, `checked_div`. No unchecked operations in critical paths.
* **Precision:** Reward calculations use `u128` intermediate values (`REWARD_PRECISION = 1e18`, `BOOST_PRECISION = 10_000`).

---

## Vault Program — Detailed Findings

### 7. Access Control & Authorization

**Status:** SECURE

| Role | Enforcement | Instructions |
|------|-------------|-------------|
| Admin | `admin.key() == vault.admin` | `pause`, `resume`, `update_admin`, `sync_oracle_position`, `close_vault`, `set_vlofi_metadata`, `admin_emergency_unstake` |
| User | Signer + PDA ownership | `deposit`, `request_withdraw`, `complete_withdraw`, `instant_redeem` |
| Permissionless | No authority check | `compound` (any funded wallet can crank) |

### 8. PDA Derivation & Validation

**Status:** SECURE

| PDA | Seeds | Program |
|-----|-------|---------|
| Vault | `["vault", channel_config]` | Vault |
| vLOFI Mint | `["vlofi", vault]` | Vault |
| CCM Buffer | `["vault_ccm", vault]` | Vault |
| Oracle Position | `["vault_oracle", vault]` | Vault |
| Withdraw Request | `["withdraw", vault, user, request_id]` | Vault |
| Metadata | `["metadata", metadata_program, vlofi_mint]` | Metaplex |

All PDAs chain from `channel_config`, making every account deterministically derivable from a single channel config pubkey + CCM mint.

### 9. Share Pricing & Inflation Protection

**Status:** SECURE

* **Virtual Offset:** `VIRTUAL_ASSETS = 1e9`, `VIRTUAL_SHARES = 1e9`. Prevents first-depositor inflation attack by ensuring non-zero share price at initialization.
* **NAV Calculation:** `net_assets = total_staked + pending_deposits + emergency_reserve - pending_withdrawals`. All components tracked independently.
* **Exchange Rate:** `(net_assets + VIRTUAL_ASSETS) * 1e9 / (total_shares + VIRTUAL_SHARES)`. Monotonically increasing under normal operation.

### 10. Transfer-Fee Accounting

**Status:** SECURE

* **Deposits:** Buffer balance captured before and after Token-2022 transfer. `actual_received = after - before`. Shares minted on actual amount, not requested amount.
* **Compound (unstake):** Buffer balance captured before and after Oracle CPI unstake. Actual received measured to prevent phantom inflation from transfer-fee discrepancy.

### 11. Pause Enforcement

**Status:** SECURE

Pause constraint (`!vault.paused @ VaultError::VaultPaused`) enforced on all 4 user-facing instructions:

| Instruction | Pauseable | Rationale |
|------------|-----------|-----------|
| `deposit` | Yes | Prevent new deposits during incident |
| `request_withdraw` | Yes | Prevent new queue entries during incident |
| `instant_redeem` | Yes | Prevent exits during incident |
| `compound` | Yes | Prevent Oracle interaction during incident |
| `complete_withdraw` | No | Users with approved requests can always exit |
| `admin_emergency_unstake` | No | Admin emergency action |
| `close_vault` | No | Admin lifecycle action |

### 12. Withdrawal & Redemption

**Status:** SECURE

* **Queued Withdrawal:** `request_withdraw` creates a `WithdrawRequest` PDA with unique `request_id` seed (prevents duplicates). `complete_withdraw` validates queue period elapsed. Full value, no penalty.
* **Instant Redeem:** 20% penalty. Requires active Oracle position (locked stake). Penalty funds emergency reserve (up to 5% NAV cap). Only buffer-backed portion of penalty is moved to reserve.
* **Pending Reservation:** `pending_withdrawals` reserved separately. Compound respects reservation: `stakeable = pending_deposits.saturating_sub(pending_withdrawals)`.

### 13. Emergency Reserve

**Status:** SECURE

* **Cap:** 5% of NAV (`RESERVE_CAP_BPS = 500`).
* **Funded by:** 20% instant-redeem penalties.
* **Included in NAV:** Reserve is part of `net_assets()`, so share price reflects reserve balance.
* **Used by:** `complete_withdraw` draws from buffer first, then reserve if buffer insufficient.

### 14. Account Closing

**Status:** SECURE

`close_vault` enforces 7 guards before allowing account closure:

1. `vault.total_shares == 0`
2. `vault.total_staked == 0`
3. `vault.pending_deposits == 0`
4. `vault.pending_withdrawals == 0`
5. `vault_ccm_buffer.amount == 0`
6. `vlofi_mint.supply == 0`
7. `vault_oracle_position.is_active == false`

Rent reclaimed to admin. All associated accounts (buffer, vLOFI mint, oracle position) closed in same transaction.

### 15. CPI to Oracle

**Status:** SECURE

All CPIs from vault to Oracle use properly scoped vault PDA signer seeds:

```
signer_seeds = &[VAULT_SEED, channel_config_key.as_ref(), &[vault_bump]]
```

Used for: `claim_channel_rewards`, `unstake_channel`, `stake_channel`. No privilege escalation possible — the vault PDA can only sign for its own channel.

---

## Post-Launch Improvements

The following security and decentralization improvements were implemented after initial mainnet deployment:

### 16. Keeper Bounty for Compound

**Commit:** `086fb35`
**Status:** IMPLEMENTED

A permissionless keeper bounty incentivizes decentralized operation of the compound crank:

| Parameter | Value | Notes |
|-----------|-------|-------|
| Bounty Rate | 0.10% (10 bps) | Applied to claimed rewards only |
| Source | Rewards | Never deducted from principal |
| Payout | Immediate | Transferred to caller's ATA in same tx |

* **Decentralization:** Anyone can profitably run a compound keeper, reducing reliance on protocol-operated infrastructure.
* **Principal Protection:** Bounty is calculated strictly on `rewards_claimed`, never touching depositor principal or pending deposits.
* **Event Emission:** `CompoundBountyPaid` event logged with vault, caller, amount, and timestamp.

### 17. Complete Withdraw Slippage Protection

**Commit:** `0a791de`
**Status:** IMPLEMENTED

Added `min_ccm_amount` parameter to `complete_withdraw` to protect users from transfer-fee-related slippage:

```rust
pub fn complete_withdraw(
    ctx: Context<CompleteWithdraw>,
    request_id: u64,
    min_ccm_amount: u64,  // slippage protection
) -> Result<()>
```

* **Transfer Fee Handling:** CCM has a 0.5% transfer fee. Users now specify the minimum acceptable amount after fees.
* **Post-Transfer Verification:** Actual received amount is measured by comparing user balance before/after transfer.
* **Revert on Slippage:** Transaction fails with `VaultError::SlippageExceeded` if `actual_received < min_ccm_amount`.
* **Consistency:** Aligns with existing slippage protection in `request_withdraw` and `instant_redeem`.

### 18. Pool Consolidation (Legacy Pool Shutdown)

**Commit:** `482828b`
**Status:** COMPLETED

Shutdown of 12 legacy staking pools to consolidate into new lock-tier architecture:

| Category | Pools Shutdown | New Structure |
|----------|---------------|---------------|
| Lofi Vault | 4 pools (3h, 6h, 9h, 12h) | Consolidated to 3h + 12h tiers |
| TWZRD | 1 pool (twzrd-247-6h) | Migrated to new tier structure |
| Audio | 7 pools (various) | Consolidated to standard tiers |
| **Total** | **12 pools** | |

* **Shutdown Reason:** "Consolidating to new lock-tier pools (3h + 12h)"
* **Execution:** Via Squads multisig proposals with batched `admin_shutdown_pool` instructions.
* **User Impact:** Lock requirements waived for exit; users can withdraw penalty-free from shutdown pools.
* **Cleanup:** Pool configs removed from `channels.ts` keeper registry.

### 19. Buffer Authority Validation in Upgrade Script

**Commit:** `a57ed03`
**Status:** IMPLEMENTED

Enhanced program upgrade script with strict buffer authority validation:

```typescript
// Trusted authorities for buffer ownership
const trustedAuthorities = [
  vaultPda,                              // Squads vault PDA (ideal)
  ...keypairs.map((kp) => kp.publicKey), // Local multisig member keys
];
```

* **Validation:** Script rejects buffers with untrusted authorities before creating upgrade proposals.
* **Guidance:** Provides clear instructions for transferring buffer authority to Squads vault if needed.
* **Defense-in-Depth:** Prevents accidental or malicious use of compromised buffers in upgrade flow.

---

## Vulnerability Checklist

| Vulnerability | Oracle | Vault | Notes |
|--------------|--------|-------|-------|
| Missing signer check | SAFE | SAFE | All admin/authority functions require `Signer` with constraint checks |
| Missing owner check | SAFE | SAFE | Anchor's `Account<>` validates program ownership |
| Account substitution | SAFE | SAFE | PDAs validated via seeds; constraints enforce expected accounts |
| Arithmetic overflow | SAFE | SAFE | All math uses checked operations; `u128` for precision-sensitive paths |
| Type confusion | SAFE | SAFE | Anchor 8-byte discriminators on all accounts |
| Reinitialization | SAFE | SAFE | `init` constraints; migration functions check account size |
| Closing accounts | SAFE | SAFE | 7-guard close pattern (vault); rent returned to user/admin |
| Missing rent exemption | SAFE | SAFE | `init` constraint handles rent allocation |
| PDA bump canonicalization | SAFE | SAFE | Bumps stored on init, re-verified on every access |
| CPI privilege escalation | SAFE | SAFE | Signer seeds scoped to exact PDA; no cross-channel signing |
| Pause bypass | SAFE | SAFE | All user-facing instructions check pause state; admin/exit paths exempt by design |

---

## Open Items

### 1. Reward Rate Underfunding Check

**Severity:** Medium

`set_reward_rate` allows admin to set a reward rate that exceeds what the treasury can fund. No on-chain validation that `reward_per_slot * expected_duration <= treasury_balance`.

* **Impact:** Pool accumulates reward debt that cannot be paid out. Users blocked from unstaking due to pending rewards check.
* **Mitigation:** Pool shutdown waives pending rewards block. Off-chain monitoring should alert on treasury balance vs. committed rate.

### 2. Emergency Unstake Reward Forfeiture

**Severity:** Medium

`admin_emergency_unstake` (vault) unstakes from Oracle without claiming pending rewards first. Accrued yield is forfeited.

* **Impact:** Users lose unclaimed rewards if admin triggers emergency unstake.
* **Mitigation:** Emergency unstake is a catastrophic-scenario tool. Under normal operation, compound crank claims rewards before re-staking. Admin should claim rewards manually before emergency unstake when possible.

### 3. Immediate Admin Transfer

**Severity:** Medium | **Status:** Mitigated via Squads Multisig configuration

`update_admin` executes immediately without a timelock. A compromised admin key can transfer authority in a single transaction.

* **Impact:** Attacker with admin key gains full control immediately.
* **Mitigation:** Operational controls via Squads Multisig. On-chain timelock would require program upgrade.

#### Squads Multisig Configuration (Recommended)

The following Squads configuration provides equivalent protection to an on-chain timelock:

1. **Enable Time Delay on Vault**
   - Navigate to Squads UI > Settings > Time Lock
   - Set execution delay to **24-48 hours** for all transactions
   - This creates a window for team members to review and potentially reject malicious proposals

2. **Configure Approval Threshold**
   - Set threshold to **3/5** (or higher) for admin operations
   - Ensures no single compromised key can execute `update_admin`
   - Add trusted team members as vault members with appropriate permissions

3. **Squads UI Configuration Steps**
   - Create or select your multisig vault at [v4.squads.so](https://v4.squads.so)
   - Go to **Settings** > **Multisig Settings**
   - Under **Threshold**, set to 3 of 5 signers minimum
   - Under **Time Lock**, enable and set delay to 24-48 hours
   - Transfer program admin authority to the Squads vault address
   - Test with a non-critical transaction before transferring admin

**Note:** On-chain timelock enforcement (two-step transfer with delay) is planned for a future program upgrade but requires redeployment. The Squads operational controls provide equivalent security for the current deployment.

---

## Recent Hardening

The following fixes were applied in the post-audit hardening pass (commit `cff6981`):

| Fix | File | Description |
|-----|------|-------------|
| Pause on RequestWithdraw | `redeem.rs` | Added `!vault.paused` constraint to close gap in pause enforcement |
| SyncOraclePosition correctness | `admin.rs` | Made vault `mut`; syncs `oracle_user_stake` key and corrects `total_staked` drift |
| CloseVault guard | `close.rs` | Added `total_staked == 0` constraint to prevent closing vault with active Oracle stake |
| Compound phantom inflation | `compound.rs` | Measures actual unstake return via buffer snapshots instead of trusting `position.stake_amount` |
| Instant redeem penalty accounting | `redeem.rs` | Only moves buffer-backed portion of penalty to reserve, preventing accounting mismatch |
| Dead code removal | `events.rs`, `channel.rs`, `merkle_proof.rs` | Removed 8 unused event structs; deduplicated `keccak_hashv` into single public function |

---

## Observations

### Treasury Outflows

**Severity:** Info

The Oracle program does not expose an `admin_withdraw` instruction. Treasury outflows occur via cumulative claims only. The program is upgradeable; the upgrade authority remains the primary governance surface.

### Publisher Compromise Risk

**Severity:** Info

A compromised publisher key can publish malicious merkle roots. Impact limited to inflation via fabricated `cumulative_total` values (no arbitrary minting). Pause mechanism halts claims during incident response. Publisher key rotatable by admin.

### Claim Receipts

**Severity:** Info

Claims emit `CumulativeRewardsClaimed` events but do not store receipt accounts on-chain. Dispute resolution relies on historical block replay. Standard gas-optimization trade-off.

### Dependency Management

**Severity:** Info

Low-severity alerts related to transitive dependencies (`ed25519-dalek`, `curve25519-dalek`, `atty`) are pinned by Solana SDK 2.3.x / Anchor 0.32.x and are not used in on-chain runtime.

---

## Test Coverage

| Suite | File | Tests | Coverage |
|-------|------|-------|----------|
| Vault Logic | `programs/channel-vault/tests/vault_logic.rs` | 36 | Deposits, withdrawals, instant redeem, compound, emergency, close, admin, share math, edge cases |
| Staking | `programs/token_2022/tests/litesvm_staking.rs` | 31 | Stake/unstake, rewards, boost, migration, shutdown, fee accounting, math invariants |
| Cumulative | `programs/token_2022/tests/litesvm_cumulative.rs` | 22 | Root publishing, merkle proofs, claims, fee harvesting, pause, admin |
| **Total** | | **89** | |

All 89 tests pass on commit `cff6981`.

---

## Recommendations

1. ~~**Multisig Transition:**~~ **COMPLETED** (commit `4e30207`) — Admin authority migrated to Squads v4 multisig. See Open Item #3 for operational timelock configuration.

2. **Timelock on Admin Transfer:** Configure Squads time delay (24-48h) for operational timelock. On-chain timelock would require program upgrade.

3. **Reward Rate Validation:** Add off-chain monitoring to alert when `reward_per_slot * slots_remaining > treasury_balance`. Consider an on-chain check in a future program upgrade.

4. **Monitoring:** Deploy alerting for:
   - Unusual cumulative claim volume or large single claims
   - Fee harvesting frequency and amounts
   - Compound crank cadence (gaps indicate keeper issues)
   - Emergency reserve levels approaching 5% cap
   - Treasury balance vs. committed reward rate

5. **Verified Builds:** Publish verifiable builds and tags for each mainnet deployment.

---

## References

* [SECURITY.md](/SECURITY.md) — Security policy and responsible disclosure
* [docs/TREASURY.md](/docs/TREASURY.md) — Treasury architecture and controls
* [programs/token_2022/src/](/programs/token_2022/src/) — Oracle program source
* [programs/channel-vault/src/](/programs/channel-vault/src/) — Vault program source
* [.well-known/security.txt](/.well-known/security.txt) — Security contact
