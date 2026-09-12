# Trust-rail ↔ AOP integration seam

**Status**: Draft interface (PR3)  
**Locked with**: H-01 remediation on `main` (`01b7b77`+) · Plamen Core 2026-08-11 · GOAL.md  
**Audience**: trust-rail / intel consumers, payout bridge, agent gate authors

This document is the **contract** for optionally binding TWZRD agent trust-rail
decisions to AOP on-chain settlement primitives **without over-claiming**
verification strength.

It does **not** change frozen trust-rail HTTP/MCP endpoints. It defines how
consumers may *attach* AOP evidence to those decisions.

> **Repositioning proposal (2026-09-05).** `docs/aop-trust-rail-repositioning.md`
> proposes retiring the listen-payout bind target (§3.1) in favour of fact types
> that back the trust gate, buyer corpus, and facilitator rail directly
> (`settlement_tx`, `gate_transcript`, `receipt_root`, `merchant_attach`). Until
> that proposal is decided, this contract stands as written; §3.1's bootstrap
> gate already yields `refuse` for every listen fact.

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
4. **Shared upgrade authority does not equal shared evidence level.** Both upgradeable programs share a single upgrade key (`8di6hHF8…` since the 2026-08-24 rotation); that is a **role risk** (see §5), not a reason to average or merge evidence levels.

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

> **The rails leg is contract surface, not available evidence today.** Its only
> canonical bind target (§3.1, listen payout) has no accounts on mainnet — that
> rail has never been bootstrapped — so the adapter currently has exactly one
> bindable fact family (markets) plus AO reference-only provenance. Evidence
> level is unaffected: rails is still `artifact_hash`, it simply has nothing to
> bind yet.

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

**Bootstrap gate (blocking — precedes the deployment gate below):**

> **The listen-payout rail has never been bootstrapped on mainnet.** All three
> of its config PDAs are absent — `getAccountInfo` returns null for each
> (verified 2026-09-03):
>
> ```
> authority_config  D6vVhAmEGBpNA9NhAfTMgQAraeG1as2SbvCLNmesoNgP
> cap_config        Ebxykho1xsxio3eQr8v4yViukTUrrVAJxYesAGsoHjCz
> vault_config      B4mUpjzUDY82TG4mNxECEWewmbkPcLpqgATnvP3ma2rk
> ```
>
> There is therefore no authority config, no cap config, no vault config, no
> published window, and no payout vault to fund. Listen-payout facts are
> currently unavailable at **any** strength — not `hard`, not `soft`. Adapters
> MUST emit `refuse` for `fact_type` ∈ {`listen_claim`, `listen_window`} while
> these accounts are absent, and MUST resolve that absence against live RPC
> rather than assuming bootstrap has since happened because this document
> exists. This gate is strictly stronger than the pin gate below: the pin gate
> decides *which* listen semantics apply, this one decides whether any listen
> fact exists at all.
>
> **Dependency chain.** Bootstrapping means `init_payout_authority_config`,
> `init_payout_cap_config`, `init_payout_vault_config`. In the **deployed**
> binary all three are gated on rails `Config.admin`, and all three declare
> `payer = admin`, so the admin key must itself be funded — a separate fee payer
> does not cover an allocation bound to `admin` in the context. `Config.admin`
> still reads the retired `2pHjZL…` (see the split note below), which holds 0
> lamports. The listen bind target is therefore blocked behind the Config admin
> rotation: `docs/playbooks/wzrd-rails-config-admin-rotation.md`.

**Post–H-01 liveness (critical — deployment-gated):**

> **Deployment gate.** The semantics in the table below exist in source
> (`03acd2c`, #129) but are **not yet live on mainnet**. The current
> `artifact_hash` pin `3128b644…` (deployed 2026-06-22, RPC re-verified
> 2026-08-13) predates H-01: on that binary `claim_listen_payout` reverts
> while `paused == true`, payout-admin rotation is 1-step, and there is no
> Config.admin emergency unpause. Against pin `3128b644…`, consumers MUST
> treat `paused == true` as a claim freeze and refuse hard bind on any
> listen fact while paused. The table applies **only** to a rails binary
> hash-verified from post-`03acd2c` source. Sequencing: deploy H-01
> (`docs/playbooks/wzrd-rails-h01-deploy-runbook.md`) → re-verify hash →
> refresh pin (§5.1) → only then enable the pause-tolerant rules below.

| Flag / action (post–H-01 binary only) | Effect on hard bind |
|---------------|---------------------|
| `PayoutAuthorityConfig.paused == true` | **Does not** block hard bind for *already published* windows — claims remain live |
| `paused == true` | **Does** block soft/hard bind for *new* unpublished windows (publish frozen) |
| Config.admin emergency unpause | Ops recovery only; not required for claim liveness of published windows |
| 2-step payout admin | Reduces rotation footgun; still SEMI_TRUSTED for allowlist/cap/pause-publish |

> **Config.admin split (2026-09-02).** The BPF upgrade authority rotated to
> `8di6hHF8…` on 2026-08-24, but rails `Config.admin` (config PDA
> `7pwUU1hv…`) still reads `2pHjZL…` on mainnet. The emergency-unpause
> escape above requires a live `Config.admin` signer: if `2pHjZL…` is
> retired for signing, the escape is inoperative until `set_admin` rotates
> it (which itself needs `2pHjZL…` to sign — otherwise recovery is a
> program upgrade under `8di6hHF8…`). Whether the split is deliberate is an
> open operator decision — see the canary playbook note. Do not count the
> escape as a mitigating control until the split is resolved.

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

### Check definitions and freshness

- **Listen checks presuppose bootstrap.** `claim_live` and `vault_prefund` both
  assume the rail's config PDAs and payout vault exist on mainnet. They do not
  (§3.1), so listen fact types short-circuit to `refuse` before either check is
  evaluated. The definitions below describe the post-bootstrap steady state.
- **`vault_prefund` is off-chain adapter arithmetic, not an on-chain check.**
  Pass criteria: live listen-payout vault ATA balance ≥
  `total_amount_ccm − claimed_so_far` for the window. The program has no
  solvency gate — an underfunded claim simply fails at the transfer CPI.
  Fee-adjust the expected wallet net per §5.3 before comparing.
- **`claim_live` is pin-dependent.** On a post–H-01 pin, published-window
  claims stay live even when `paused == true`; on pin `3128b644…` this check
  MUST fail while paused (see §3.1 deployment gate).
- **Staleness.** Every BindDecision carries `as_of_slot`. A hard bind MUST be
  re-derived before acting when it is older than the consumer's freshness
  bound (RECOMMENDED default: 300 slots ≈ 2 minutes). Pause flips, vault
  drains, and pin invalidations between bind and act are otherwise invisible
  to the consumer.

---

## 5. Shared constraints (all AOP facts)

1. **Pin refresh:** After any rails/markets upgrade, re-verify hash, update pin, invalidate prior hard binds until re-pin.  
2. **Upgrade authority:** Single key `8di6hHF8…` (rotated from `2pHjZL…` 2026-08-24; RPC re-verified 2026-09-02) — product FULLY_TRUSTED today; treat total rewrite as catastrophic until Squads (non-goal for this seam).  
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
- [x] Post–H-01 claim liveness documented (pause ≠ claim freeze) — **deployment-gated**: live pin `3128b644…` remains pre–H-01 until the deploy + re-pin lands  
- [x] Listen-payout bind target documented as **unavailable at any strength** — all three config PDAs absent on mainnet, bootstrap gated on `Config.admin` (§3.1)  
- [x] BindDecision + strength matrix defined  
- [x] Cross-link checklist for operators  

Next (wzrd-final / intel): implement adapter against this contract; pin hashes in config; CI golden leaf parity.
