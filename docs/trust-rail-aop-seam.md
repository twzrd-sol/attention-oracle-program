# Trust-rail ↔ AOP integration seam

**Status**: Draft interface (PR3)  
**Locked with**: H-01 remediation on `main` (`01b7b77`+) · Plamen Core 2026-08-11 · GOAL.md  
**Audience**: trust-rail / intel consumers, payout bridge, agent gate authors

This document is the **contract** for optionally binding TWZRD agent trust-rail
decisions to AOP on-chain settlement primitives **without over-claiming**
verification strength.

It does **not** change frozen trust-rail HTTP/MCP endpoints. It defines how
consumers may *attach* AOP evidence to those decisions.

---

## 1. Evidence levels (locked — do not flatten)

Priority for **consumer hard-bind eligibility** and honest labeling:

| Rank | Level | Program | ID | Meaning |
|-----:|-------|---------|----|---------|
| **1 (strongest)** | `rebuild` | **wzrd-markets** | `7YZZxQC9JrWwoo1W1fYgxZs6rnbW17iF72mi65vU93sy` | Live binary **byte-reproduced** from committed source (#124). Highest integrity pin. |
| **2** | `artifact_hash` | **wzrd-rails** | `BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9` | Live binary matches preserved deploy artifact (`solana-verify`); **not** a docker verifiable-source rebuild. See `docs/wzrd-rails-mainnet-verification.md`. |
| **3 (weakest)** | `unreproduced` | **AO v2** | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | Immutable; on-chain hash not reproduced from public tree. **Reference only — never hard-bind.** |

### Why this weighting (and why not to adjust)

1. **Honesty > marketing.** Flattening rails to `rebuild` would misstate #126’s mechanical proof and invite agents to treat chronological source inference as bit-for-bit rebuild.
2. **Hard bind follows rank.** A commerce decision that requires the strongest on-chain attestation should prefer markets paths (`rebuild`) over rails (`artifact_hash`). Soft signals may use rails freely with labeled level.
3. **AO is not a third peer.** Demoting AO is not optional: unreproduced source means no agent-hard evidence path. Do not invent a fourth “legacy” hard tier.
4. **Shared upgrade authority does not equal shared evidence level.** Both upgradeable programs share `2pHjZL…`; that is a **role risk** (see §5), not a reason to average or merge evidence levels.

**Product rule:** every AOP-backed attestation field MUST include `evidence_level` ∈ {`rebuild`, `artifact_hash`, `unreproduced`}. Consumers MUST refuse hard bind when `evidence_level == unreproduced`.

---

## 2. Seam architecture

```
┌─────────────────────────────────────────────────────────────┐
│ Agent / gate                                                │
│  1) Free preflight  2) ROI-gated paid trust  3) receipt     │  ← frozen trust rail
└───────────────────────────┬─────────────────────────────────┘
                            │ optional attach
                            ▼
┌─────────────────────────────────────────────────────────────┐
│ AOP evidence adapter (this seam)                            │
│  · pin program hashes by evidence_level                     │
│  · verify leaf/domain/proof or resolution finality          │
│  · apply net-fee + pause/liveness policy                    │
│  · emit BindDecision { strength, evidence_level, reason }   │
└───────────────────────────┬─────────────────────────────────┘
          ┌─────────────────┼─────────────────┐
          ▼                 ▼                 ▼
   wzrd-markets        wzrd-rails           AO v2
   (rebuild)         (artifact_hash)    (unreproduced)
```

Trust-rail scores and AOP evidence are **orthogonal**:

| Plane | Answers |
|-------|---------|
| Trust rail | “Should I pay *this counterparty*?” |
| AOP seam | “Is *this on-chain settlement fact* funded, live, and honestly labeled?” |

A high trust score does **not** upgrade `artifact_hash` → `rebuild`. A perfect merkle claim does **not** replace preflight wash detection.

---

## 3. Canonical bind targets

### 3.1 Listen payout entitlement (rails · `artifact_hash`)

| Field | Value |
|-------|--------|
| Program | `BdSv…` · level `artifact_hash` |
| Off-chain builder | `attention_payout_bridge` / `PayoutAllocationLeafV1` (wzrd-final) |
| Publish IX | `publish_listen_payout_root` |
| Claim IX | `claim_listen_payout` |
| Domain | Listen-payout merkle domains only (never compensation plain, never markets) |

**Post–H-01 liveness (critical):**

| Flag / action | Effect on hard bind |
|---------------|---------------------|
| `PayoutAuthorityConfig.paused == true` | **Does not** block hard bind for *already published* windows — claims remain live |
| `paused == true` | **Does** block soft/hard bind for *new* unpublished windows (publish frozen) |
| Config.admin emergency unpause | Ops recovery only; not required for claim liveness of published windows |
| 2-step payout admin | Reduces rotation footgun; still SEMI_TRUSTED for allowlist/cap/pause-publish |

### 3.2 Markets resolution / settle (markets · `rebuild`)

| Field | Value |
|-------|--------|
| Program | `7YZZ…` · level `rebuild` |
| Path | create-time `resolution_root` → resolve / override → settle / redeem |
| Hard bind prefers | YES/NO settled winner after unlock; not mid-dispute |
| Soft / refuse | Unbounded override postpone; INVALID asymmetric inventory; admin==resolver brick |

### 3.3 AO v2 (· `unreproduced`)

No hard bind. Optional soft provenance / historical reference only.

---

## 4. `BindDecision` interface (consumer contract)

Suggested JSON shape for adapters (language-agnostic):

```json
{
  "strength": "hard" | "soft" | "refuse",
  "evidence_level": "rebuild" | "artifact_hash" | "unreproduced",
  "program_id": "<base58>",
  "program_hash": "<hex solana-verify executable hash>",
  "fact_type": "listen_claim" | "listen_window" | "market_settled" | "market_price" | "other",
  "fact_ref": { "window_id": 0, "leaf_index": 0, "market_id": null },
  "net_amount": { "raw": "0", "mint": "<pubkey>", "fee_adjusted": true },
  "checks": [
    { "id": "hash_pin", "ok": true },
    { "id": "domain", "ok": true },
    { "id": "claim_live", "ok": true },
    { "id": "vault_prefund", "ok": true }
  ],
  "reason": "human-readable summary",
  "as_of_slot": 0
}
```

### Strength matrix

| Strength | When |
|----------|------|
| **hard** | Pin matches declared `evidence_level` floor for that program · all fact-type checks pass · fee-adjusted net · not refuse-class role/finality brick |
| **soft** | Fact exists but underfund, unlock open, dispute active, or fee uncertain |
| **refuse** | Hash mismatch · wrong domain · `unreproduced` for commerce · leaf/proof fail · known permanent brick path required |

**Floor rule:** hard bind on rails requires at least `artifact_hash` pin; hard bind on markets requires `rebuild` pin. Never hard-bind AO.

---

## 5. Shared constraints (all AOP facts)

1. **Pin refresh:** After any rails/markets upgrade, re-verify hash, update pin, invalidate prior hard binds until re-pin.  
2. **Upgrade authority:** Single key `2pHjZL…` — product FULLY_TRUSTED today; treat total rewrite as catastrophic until Squads (non-goal for this seam).  
3. **TransferFee:** Live CCM 50 bps — **never** treat event gross as wallet net for settlement math.  
4. **Domain separation:** Rails listen ↔ markets resolution roots are not interchangeable (GATE A).  
5. **Leaf parity:** `PayoutAllocationLeafV1` off-chain builders must stay golden-equal to on-chain verifier.  

Full operational checklist: `docs/agent-trust-integration-checklist.md`.

---

## 6. Out of scope (explicit)

- Changing `intel.twzrd.xyz` preflight / trust / receipt verify schemas without a trust-rail version bump.  
- Mainnet program deploys (separate operator go + verification record append).  
- Squads migration of upgrade authority.  
- AO source reproduction.  
- Implementing the adapter inside wzrd-final (follow-on; this repo owns the **contract**).

---

## 7. Acceptance for this PR

- [x] Evidence hierarchy locked as `rebuild > artifact_hash > unreproduced`  
- [x] Post–H-01 claim liveness documented (pause ≠ claim freeze)  
- [x] BindDecision + strength matrix defined  
- [x] Cross-link checklist for operators  

Next (wzrd-final / intel): implement adapter against this contract; pin hashes in config; CI golden leaf parity.
