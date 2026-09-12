# Agent Trust Rail Integration Checklist

**Date**: 2026-08-11  
**Programs tip**: post-#129 H-01 on `main`  
**Goal**: Connect AOP on-chain settlement primitives to TWZRD agent trust rails without over-claiming evidence strength.  
**Seam contract**: `docs/trust-rail-aop-seam.md`  
**Related audit**: `AUDIT_REPORT_TRUST_RAILS_2026-08-11.md` (local Plamen Core)

---

## 1. Systems map (decision → on-chain evidence)

| Trust-rail step | Off-chain API / builder | On-chain program | Instruction / account | Safe as hard evidence? |
|-----------------|-------------------------|------------------|----------------------|------------------------|
| Preflight counterparty | `intel.twzrd.xyz` readiness | — | — | Yes (off-chain score; unchanged) |
| Paid trust score | `GET /v1/intel/trust/{pubkey}` | — | — | Yes (signed receipt; not AOP) |
| Receipt verify | `/v1/receipts/verify` | — | — | Yes (trust-rail crypto only) |
| **Optional bind: listen payout entitlement** | `attention_payout_bridge` → `PayoutAllocationLeafV1` | **wzrd-rails** `BdSv…` (`artifact_hash`) | `publish_listen_payout_root` + `claim_listen_payout` | **Conditional** — §2 |
| **Optional bind: stake / reward claim** | — | **wzrd-rails** | `stake` / `claim` / `unstake` | **Conditional** — fee net + vault funding |
| **Optional bind: market resolution** | attention root publisher | **wzrd-markets** `7YZZ…` (`rebuild`) | resolve → settle / redeem | **Conditional** — §3 |
| **Optional bind: complete-set / CPMM price** | — | **wzrd-markets** | mint / swap / LP | **Soft** — first-LP ratio residual |
| AO v2 vault/claim | legacy | **AO** `GnGz…` (`unreproduced`) | immutable | **No hard bind** |

---

## 2. Listen payout — safe consumption constraints

**Blocking precondition — the rail is not bootstrapped on mainnet.** All three
listen-payout config PDAs are absent; `getAccountInfo` returns null for each
(verified 2026-09-03):

```
authority_config  D6vVhAmEGBpNA9NhAfTMgQAraeG1as2SbvCLNmesoNgP
cap_config        Ebxykho1xsxio3eQr8v4yViukTUrrVAJxYesAGsoHjCz
vault_config      B4mUpjzUDY82TG4mNxECEWewmbkPcLpqgATnvP3ma2rk
```

No authority config, no cap config, no vault config — therefore no published
windows and no payout vault to fund. Every row of the table below presupposes
accounts that do not exist, so listen facts are unavailable at **any** strength
today: emit `refuse` for `fact_type` ∈ {`listen_claim`, `listen_window`}, and
re-resolve the absence against live RPC rather than assuming bootstrap has since
happened because this document exists. Bootstrap means
`init_payout_authority_config`, `init_payout_cap_config`,
`init_payout_vault_config` — in the **deployed** binary all three are gated on
rails `Config.admin` and all three declare `payer = admin`, and `Config.admin`
still reads the retired `2pHjZL…` (§5, §7 gap 3,
`docs/playbooks/wzrd-rails-config-admin-rotation.md`). This gate precedes the
H-01 pin gate below: the pin decides *which* listen semantics apply, this decides
whether any listen fact exists at all.

**Once the rail is bootstrapped**, before treating a listen leaf as **hard**
funded entitlement:

| Check | Pass criteria | If fail |
|-------|---------------|---------|
| Program hash | Executable hash matches pinned rails artifact (or newer re-verified pin) | Refuse hard bind; re-verify |
| Evidence level | Consumer labels `artifact_hash` (not `rebuild`) | Mislabel → refuse honesty gate |
| Leaf layout | `PayoutAllocationLeafV1` golden parity with on-chain verifier | Refuse |
| Domain | Listen-payout domain only — never compensation or markets domains | Refuse |
| **Publish pause (H-01 — pin-dependent)** | **Post–H-01 pin only:** new windows may be blocked if `paused`; claims on published windows stay live | On a post–H-01 pin, do **not** refuse published claims solely because `paused == true`. On the current live pin `3128b644…` (pre–H-01), `claim_listen_payout` reverts while paused — treat `paused == true` as a claim freeze and refuse hard bind |
| Prefund | Listen vault ATA balance ≥ remaining unclaimed class | Soft / partial FCFS |
| Cap | Window total ≤ per-window cap at publish | On-chain enforced |
| Proof | Valid listen-payout node convention; `MAX_PROOF_LEN` | Fail closed |
| Bitmap | Leaf index unclaimed | Fail closed |
| Hard cap | `claimed_so_far + amount ≤ total_amount_ccm` | On-chain enforced |
| Wallet match | Claimer == `leaf.wallet_pubkey` | Fail closed |
| **Net receipt** | Expected wallet net under live TransferFeeConfig; do **not** use event gross | Adjust amount |
| Publisher | Allow-listed SEMI_TRUSTED — fairness not on-chain | Score publisher separately |
| Payout admin | 2-step rotation + Config.admin unpause-only (H-01) | Ops residual; claims still live for published windows |

---

## 3. Markets resolution / settle — safe consumption constraints

| Check | Pass criteria | If fail |
|-------|---------------|---------|
| Program hash | Matches **rebuild** pin | Refuse |
| Evidence level | Consumer labels `rebuild` | Mislabel → refuse honesty gate |
| Create snapshot | Resolution root+seq snapshotted at `create_market` | Design OK; wrong root → never-resolve |
| Outcome | YES/NO settled after unlock | INVALID → §3b |
| Unlock | `clock_slot > settle_unlock_slot` (strict) | Soft until unlock |
| Override storm | No indefinite no-op override cycle | Soft / delay (finality DoS residual) |
| Admin≠resolver | Roles still separated | Override brick if equal |
| Collateral | Fee-exempt USDC as live policy | Latent underpay if regressed |

### 3b. INVALID markets

| Check | Pass criteria |
|-------|---------------|
| Inventory | Holder has **both** YES and NO for redeem; single-side stranded on-chain |
| Grace | Before grace end, admin cannot force-sweep residual |
| Post-grace | Admin may void full vault residual — do not assume salvage |
| Do not | Treat INVALID as automatic pro-rata USDC for asymmetric books |

---

## 4. Evidence levels (product honesty)

| Program | Live evidence level | Trust-rail field |
|---------|---------------------|------------------|
| wzrd-markets | Byte-reproduced from source | `evidence_level = "rebuild"` |
| wzrd-rails | Artifact-hash-verified only; live pin `3128b644…` is **pre–H-01** | `evidence_level = "artifact_hash"` |
| AO v2 | Unreproduced / immutable | `evidence_level = "unreproduced"` → **no hard bind** |

**Priority mapping (locked):** `rebuild` > `artifact_hash` > `unreproduced`.

Never advertise rails as source-rebuild verified without actual solana-verify source reproduction.

After **any** upgrade of rails or markets:

1. Re-fetch ProgramData executable hash.  
2. Re-run rebuild or artifact compare.  
3. Invalidate prior hard binds until pins update.  
4. Re-check leaf golden parity for rails.

---

## 5. Role trust assumptions

| Role | Trust class | Can affect agent evidence |
|------|-------------|---------------------------|
| Upgrade authority `8di6hHF8…` (rotated 2026-08-24) | FULLY_TRUSTED (product) | Total rewrite both programs |
| Markets admin / resolver | FULLY_TRUSTED intent | Finality DoS, INVALID, override brick |
| Rails Config.admin | FULLY_TRUSTED | **Nine** IX answer to it in the *deployed* binary, not four: `set_admin`, `set_reward_rate`, `initialize_pool`, `compensate_external_stakers`, `realloc_stake_pool`, `register_verified_moment`, plus the three `init_payout_*` bootstraps — so this key also gates standing up the listen-payout rail at all (§2). Also **unpause-only** on listen pause (H-01). Still `2pHjZL…` on-chain — **not** rotated with the 2026-08-24 BPF authority rotation; that key holds 0 lamports while `init_payout_*` declares `payer = admin`, so the un-bootstrapped rail stays blocked until it rotates (`docs/playbooks/wzrd-rails-config-admin-rotation.md`). The unpause escape likewise depends on this key's liveness (operator decision pending, see seam §3.1) |
| Payout admin | **SEMI_TRUSTED** | Publish pause, allowlist, cap; 2-step rotation (H-01) |
| Listen / attention publisher | **SEMI_TRUSTED** | Root content within caps |
| Unprivileged user | Permissionless | First LP ratio, dust stake grief windows |

H-01 behaviors above (2-step rotation, unpause-only Config.admin escape) describe post–H-01 source; they are live on-chain only after the H-01 deploy (§7 gap 4).

---

## 6. Decision matrix for agent gate

| Desired bind strength | Requirements |
|----------------------|--------------|
| **Hard** | Pin OK for program’s evidence floor · fact-type checks pass · fee net adjusted · not refuse-class finality brick · published listen claims OK even if publish-paused (H-01, **post–H-01 pin only** — on `3128b644…` paused ⇒ refuse) |
| **Soft** | Fact exists but underfund, unlock open, dispute active, or fee uncertain |
| **Refuse** | Hash mismatch, wrong domain, AO-only evidence, admin==resolver when override required, INVALID asymmetric without rebalance plan |

---

## 7. Open product gaps (explicit constraints)

1. Shared single upgrade key — Squads later; pin hashes now.  
2. Rails not source-reproduced — label `artifact_hash`.  
3. Listen-payout rail **never bootstrapped on mainnet** — all three config PDAs absent (§2), so no listen fact binds at any strength. Its `init_payout_*` bootstraps are gated on the retired `2pHjZL…` `Config.admin`, so clearing this needs the Config admin rotation (`docs/playbooks/wzrd-rails-config-admin-rotation.md`) and then the bootstrap.  
4. Payout pause freezes claims — **fixed in source (H-01 #129) but NOT yet deployed**; live binary `3128b644…` still freezes claims under pause. Cleared only by the H-01 deploy + pin refresh (`docs/playbooks/wzrd-rails-h01-deploy-runbook.md`).  
5. Outbound gross attestation under live 50 bps TransferFee.  
6. Unbounded override × extend finality delay (markets).  
7. INVALID asymmetric + grace full sweep.  
8. Publisher fairness not on-chain.  

---

## 8. Minimal operator runbook for trust-rail go-live

1. Pin executable hashes for `BdSv…` (`artifact_hash`) and `7YZZ…` (`rebuild`) in trust-rail config.  
2. Wire leaf builder CI to rails goldens on every deploy.  
3. Index vault balances, `settle_unlock_slot`, last override slot; treat `paused` as **publish** freeze only.  
4. Apply TransferFee schedule when computing claimable net.  
5. Block hard bind if any Refuse condition.  
6. Document SEMI_TRUSTED payout admin + evidence levels in agent-facing risk copy.  

---

**Sources**: Plamen Core 2026-08-11, GOAL.md, `docs/wzrd-rails-mainnet-verification.md`, `docs/trust-rail-aop-seam.md`, H-01 #129.
