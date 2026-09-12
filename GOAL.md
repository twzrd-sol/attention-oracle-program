# GOAL: Connect AOP to agent trust rails

**Locked**: 2026-08-11  
**Repo tip**: post-#129 H-01 on `main` (PoCs #127 + governance fix)  
**Status**: Active — seam draft in `docs/trust-rail-aop-seam.md`  
**Repositioning (2026-09-05, proposal)**: `docs/aop-trust-rail-repositioning.md` — move AOP's
role from streaming attention (listen payouts, attention-metric markets — inert on mainnet) to
backing the trust gate, buyer corpus, and facilitator rail. Not yet decided; item 6 below is
the surface it would change.

## One-liner

Wire the **Attention Oracle Program (AOP)** on-chain surface to TWZRD’s **agent trust rail** so agent commerce decisions can rest on verified attention/settlement primitives, not only off-chain intel scores.

## Why now

1. **AOP mainnet refresh is locked** — markets `7YZZ…` byte-reproducible (#124); rails `BdSv…` artifact-hash verified (#126, hash `3128b644…`); AO v2 immutable/unreproduced (reference-only).
2. **Agent trust rail v1 is frozen** — `intel.twzrd.xyz` preflight / paid trust / receipt verify are the decision API agents already call.
3. **The missing seam** is the bridge: funded attention/payout settlement on AOP (and related merkle leaves) must be safe enough that trust-rail consumers can treat on-chain claims as hard evidence.

## North-star outcome

An agent (or trust gate) can:

1. **Preflight / score** a counterparty via the frozen trust rail (unchanged).
2. **Optionally bind** that decision to AOP-backed proof of attention allocation or market resolution when those primitives are in the product path.
3. **Never** mint, claim, or settle more value than funded policy allows — on-chain invariants hold under unprivileged + semi-trusted roles.

## In-scope systems

| Layer | Component | Role |
|-------|-----------|------|
| On-chain (AOP) | `wzrd-rails` `BdSv…` | Staking / listen payout rails, merkle claim paths, caps |
| On-chain (AOP) | `wzrd-markets` `7YZZ…` | Attention-linked prediction markets, resolution, CPMM |
| On-chain (AOP) | AO v2 `GnGz…` | Immutable legacy; reference only for provenance |
| Off-chain (wzrd-final) | Trust rail | `preflight`, `/v1/intel/trust/{pubkey}`, `/v1/receipts/verify` |
| Off-chain (wzrd-final) | `attention_payout_bridge` / listen payout | Builds `PayoutAllocationLeafV1` roots for rails publish |

## Security gate (this session)

**Plamen Core** on post-refresh AOP `main` (`ca1e759`):

- Scratchpad: `.plamen-trust-rails/`
- Primary focus: settlement integrity, merkle leaf/root parity with off-chain builders, semi-trusted publisher/admin roles, claim/redeem solvency, Token-2022 surfaces agents will touch.
- Prior June audits (rails, markets, delta) are **baseline**, not ground truth — re-verify live tip.

## Non-goals (this goal slice)

- Squads migration of upgrade authority (documented decision: later).
- AO v2 source reproduction / re-deploy (immutable).
- Changing frozen trust-rail endpoint contracts without a version bump.
- Mainnet deploys or on-chain sends without separate operator go.

## Done when

1. ~~Plamen Core report~~ — `AUDIT_REPORT_TRUST_RAILS_2026-08-11.md` (local)  
2. ~~Medium+ classified + PoCs~~ — #127  
3. H-01 claim liveness + 2-step payout admin — #129 **source-complete; mainnet deploy pending** (live pin `3128b644…` is pre–H-01 — see `docs/playbooks/wzrd-rails-h01-deploy-runbook.md`)  
4. **Integration seam** — `docs/trust-rail-aop-seam.md` + checklist (PR3): trust-rail decision → AOP fact with locked evidence levels  
5. Open gaps (upgrade key, fee gross/net, markets finality residuals) listed as product constraints, not silent assumptions  
6. Follow-on: wzrd-final adapter implements `BindDecision` against the seam contract  

### Evidence hierarchy (locked for PR3+)

`markets reproduced (rebuild)` > `rails artifact_hash` > `AO unreproduced` — never flatten; hard-bind floors follow rank.  

## Operator posture

AOP code/docs changes and Plamen analysis are authorized under this goal.  
**Still require fresh go**: deploys, Doppler, production DB writes, on-chain txs, force-push.
