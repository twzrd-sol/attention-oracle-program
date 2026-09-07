# AOP repositioning: from streaming attention to the trust rail

**Status**: proposal (2026-09-05); operator decisions recorded 2026-09-06 (§5).
Nothing here is deployed. Admin rotation and any on-chain send still need explicit go.
**Supersedes in intent**: the listen-payout bind target in `docs/trust-rail-aop-seam.md` §3.1.
**Does not change**: the evidence-level hierarchy, the H-01 deployment gate, the Config-admin
split note, or any on-chain state.

## 1. The decision being proposed

Reposition the Attention Oracle Program surface (wzrd-rails, wzrd-markets, AO v2) **away from
streaming attention** — listen-payout entitlements and attention-metric markets — and **toward
backing the three trust-rail surfaces that are live and revenue-bearing** in `twzrd-trust`:

| Surface | What it is today | The gap an on-chain program closes |
|---|---|---|
| **Trust gate** (`twzrd-x402-gate` AutoGate) | Buyer-side pre-sign checkpoint. Emits an integrator-signed decision token; its adoption and block "proofs" are **unsigned self-assertions** whose invariants (`signer_invocations`, `actual_spend_usdc`, `onchain_settlements`) are caller-supplied integers. `readiness_card.proof` is a declared field **no client ever consumes**. | A server-side join the gate's own code says is missing (`notExternalRunProof`). An anchored transcript turns a self-claim into an attestation without touching any existing schema. |
| **Buyer corpus** (`intel.twzrd.xyz`) | Preflight, merchant_card, wash flags, V6 receipts signed by live issuer `Ak5SQwHpuQA…` (`twzrd-receipt-ed25519-v2`; not v1 `9V6Pn19k…`). A V6 `leaf` is a **single keccak hash of a flat preimage** — no merkle path, no root, no depth. | Batching receipt leaves into periodically published on-chain roots makes any receipt independently verifiable against chain, using the audited keccak convention rails already ships. |
| **Payment facilitator rail** (`/supported → /verify → /settle`) | TWZRD cosigns as feePayer `4LkEFjJd…`, then best-effort attaches a V6 receipt + `merchant_attach`. Every `settlement_tx` in the system is a base58 string **taken on trust from the issuer or scraped from payer output**; `delivery-capture.js` says so and names chain-side verification as an un-shipped phase. | Settlement verification is a ledger read, not a program. The seam's adapter can do it today. What a program adds is anchoring the *attach* facts (track-record leaf, demand snapshot) so they are not issuer-only. |

The streaming-attention side, by contrast, is inert on mainnet and the README already calls it
experimental (`390beda`, 2026-08-25):

- the listen-payout rail was **never bootstrapped** (all three config PDAs absent);
- its H-01 governance fix is **not deployed** (live hash `3128b644…`, pre-H-01);
- every instruction that could bootstrap it is gated on `Config.admin`, which is the **retired,
  unfunded `2pHjZL…`** (`docs/playbooks/wzrd-rails-config-admin-rotation.md`);
- wzrd-markets resolves on an attention-shaped leaf (`streamer_ref`, `metric`, `observed_value`).

## 2. What AOP has on mainnet that serves the new position

Each row checked against the **deployed** binaries (rails `d441724`, markets `425e115`), not
current source.

| Primitive | Program · evidence | Serves | Usable today? |
|---|---|---|---|
| `register_verified_moment` — PDA `[b"verified_moment", claim_id:16]` committing four 32-byte hashes + recipient + authority + slot/ts | rails · `artifact_hash` | **Trust gate** transcript anchoring. A decision token's `decisionId` is a UUID (16 bytes) — it *is* a `claim_id`. `intentHash`, transcript hash, preflight-response hash, block-proof hash fill the four slots. | **Structurally yes, operationally no.** It is gated on `Config.admin` (retired key), and its fields are commemorative-shaped (`asset_id`, `collection`, `og_gng_program` must be non-zero / a fixed program). Overloading works as a proof of concept; a purpose-built attestation account is the right shape. |
| Listen-payout merkle convention (`wzrd-rails:listen-payout-allocation-{leaf,node}:v1`, sorted-pair, node-domain-separated, `MAX_PROOF_LEN` 16) | rails · `artifact_hash` | **Buyer corpus** receipt roots. The convention is audited and golden-vector-tested; a corpus-root publish IX reuses it verbatim with its own domain strings, exactly as markets did. | **No** — the publish IX is listen-specific (window cap, claim bitmap, CCM vault). Needs an upgrade. |
| Pool 0 — 3,116,139.77 CCM staked, solvent | rails · `artifact_hash` | Nothing in the trust rail. `twzrd-trust` has **no staking, bonding, or slashing concept anywhere**, and the stake is internal WZRD wallets the June upgrade plan already wrote off. | Keep as-is. Not a trust signal; do not advertise it as one. |
| wzrd-markets resolve → settle | markets · `rebuild` (strongest pin) | Corpus-fact markets later ("did `pay_to` X clear N settlements by window W"). | **Not without a v2 leaf domain** — `streamer_ref`/`metric` are attention semantics. Explicit non-goal for this slice. |
| AO v2 `publish_global_root` / `claim_global_v2`, `root_seq`, `verify_root_inputs` | AO · `unreproduced` | The legacy earn/claim lane already has a merkle-root vocabulary — stranded on an immutable, unreproduced binary. | Reference only. Never hard-bind. |
| Solana ledger itself (a USDC transfer with signature S landed at slot N) | none · **stronger than any program pin** | **Facilitator rail** settlement verification. | **Yes, today, adapter-side.** No AOP change required. |

The one structural fact that makes this tractable: **the BPF upgrade authority `8di6hHF8…` is
live, funded, and rotated; the Config admin is not.** New trust-rail instructions can be gated on
their own authority PDA instead of `Config.admin`, so they do not inherit the retired key's blast
radius. A rails upgrade is signed by `8di6…`; nothing about it needs `2pHj…` to exist.

## 3. Proposed fact types for the seam

Additions to `BindDecision.fact_type` (seam §4). Existing values are unchanged; `listen_*` is
retired below.

| `fact_type` | Verified how | Evidence | Strength ceiling |
|---|---|---|---|
| `settlement_tx` | Adapter reads the signature at `finalized`: USDC (`EPjFWdd5…`) transfer to the stated `payTo`, amount matches the intent, feePayer as declared. No program involved. | **program-independent** (`program: none`, `verification: ledger`). Not a fourth `evidence_level` (§5.1). `BindDecision.evidence_level` is omitted / N/A. | `hard` |
| `gate_transcript` | A verified-moment (or successor attestation) PDA exists whose hashes recompute from the decision token, intent, and preflight response the consumer holds. | rails pin (`artifact_hash` today; `rebuild` after a verifiable upgrade) | `hard` once the anchoring IX is not Config-admin-gated; `soft` while it is |
| `receipt_root` | V6 `leaf` proves against a published corpus root under the rails corpus domain; `signing_pubkey` is the **live** issuer (`Ak5SQwHpuQA…` / `twzrd-receipt-ed25519-v2` as of 2026-09-06 `/health`; not v1 `9V6Pn19k…`). | rails pin | `hard` after upgrade; unavailable before |
| `merchant_attach` | `merchant_attach` track-record leaf proves against the same root class. | rails pin | as above |
| `listen_claim`, `listen_window` | — | — | **retired**: `refuse`, permanently, unless the rail is bootstrapped by an explicit operator decision |
| `market_settled`, `market_price` | unchanged | markets `rebuild` | unchanged; attention-shaped until a v2 leaf |

## 4. Sequencing

Ordered so each step is independently shippable and the biggest hole closes first.

0. **Docs (this PR).** This proposal; seam §3.1 and §6 pointers; GOAL.md pointer; §5
   decisions recorded. No on-chain change, no consumer change.
1. **Ledger-verified settlement, adapter-side (next implementation).** Implement `settlement_tx` in the wzrd-final
   adapter and populate `readiness_card.proof` / `offline_receipt_verification` with the result.
   Closes "nothing reads Solana state" with zero AOP work and zero AOP risk. Also gives
   `delivery-capture.js` the server-side settlement check it documents as pending.
2. **rails v-next: attestation + corpus roots.** One upgrade, one verifiable build, signed by
   `8di6…`:
   - `register_attestation(claim_id:16, kind:u8, hashes:[[u8;32];4], subject:Pubkey)` gated on a
     new `AttestationAuthorityConfig` PDA (allow-list, mirrors `PayoutAuthorityConfig` minus pause),
     **not** on `Config.admin`;
   - `publish_corpus_root(seq, root, leaf_count, schema)` under new domains
     `wzrd-rails:corpus-receipt-{leaf,node}:v1`, same convention, same `MAX_PROOF_LEN`;
   - no CCM movement, no vault, no claim path — attestation only, so the H-01-class pause/claim
     coupling cannot recur.
   Deploy via `solana-verify build` so rails moves to `rebuild` and the seam's two-tier
   hard-bind asymmetry disappears. Pin refresh per seam §5.1.
3. **Config admin rotation** (`docs/playbooks/wzrd-rails-config-admin-rotation.md`) — still
   required, but demoted from "prerequisite for the trust rail" to "hygiene": it unblocks
   `set_reward_rate`, `initialize_pool`, `compensate_external_stakers`, `realloc_stake_pool`,
   `register_verified_moment`, and the listen bootstraps. None of those is on the path above.
4. **H-01 deploy** (`docs/playbooks/wzrd-rails-h01-deploy-runbook.md`) — folded into step 2's
   upgrade if the listen rail is kept in the binary; **optional** if it is retired. Do not ship
   it as a standalone deploy any more.
5. **Corpus-fact markets** — v2 resolution leaf domain, separate scope doc. Not before step 2
   has a live corpus root to resolve against.

## 5. Operator decisions (recorded 2026-09-06)

Nothing in this section is a deploy, a `CONFIRM_BROADCAST`, or an H-01 ship.

1. **Do not add a `ledger` evidence level above `rebuild`.** Seam §1 stays the locked
   three-value program-provenance enum (`rebuild` > `artifact_hash` > `unreproduced`).
   A Solana USDC transfer is not AOP-backed, so it must not pick a program rank.
   Mixing "how we know the program is real" with "this fact uses no program" is the
   wrong axis. `settlement_tx` is recorded in the fact table as `program: none`,
   `verification: ledger`; `BindDecision.evidence_level` is omitted / N/A for that
   fact. Consumers of the three-tier enum are unchanged.

2. **Retire listen payouts in the seam only.** `listen_claim` / `listen_window` stay
   `refuse` until an explicit Liquid Attention product call. Removing IXs from the
   rails binary is an upgrade (a deploy) and is not implied here. Dead on mainnet
   (never bootstrapped + retired `Config.admin`) is not the same as deleted.

3. **Who signs: not the feePayer, not `Config.admin`.** First member of a new
   `AttestationAuthorityConfig` allow-list is the **live** V6 receipt issuer
   (`Ak5SQwHpuQA…` / `twzrd-receipt-ed25519-v2` on `intel.twzrd.xyz/health` as of
   2026-09-06). The draft's `9V6Pn19k…` is the v1 key — do not use it. FeePayer
   `4LkEFjJd…` is gas, never an attestation key. Retired `2pHjZL…` is never on the
   list. Do not freeze the rest of the allow-list, and do not implement the PDA,
   until the step-2 upgrade is designed.

4. **No `register_verified_moment` proof-of-concept before step 2.** That IX is
   `Config.admin`-gated, so a "cheap demo" *is* the admin rotation. Overloading
   commemorative fields (`asset_id`, `og_gng_program`) is not a gate transcript.
   Skip. Next implementation is sequencing step 1 (adapter-side `settlement_tx`
   in wzrd-final), not this IX and not a rotation.

## 6. What this retires, keeps, and explicitly does not claim

- **Retired as a bind target**: listen-payout entitlements (`listen_claim`, `listen_window`).
  The IXs stay in the binary until an explicit Liquid Attention kill (decision 2).
- **Kept, unchanged**: markets as an attention-metric product; pool 0 stake; AO v2 as
  reference-only provenance; the evidence hierarchy and every gate in the seam.
- **Not claimed**: that any of the new fact types exist on chain today. Only `settlement_tx`
  is available now, and it is adapter-side. Everything with a rails pin is behind step 2.
- **Not touched**: `twzrd-trust` schemas. `readiness_card.proof` is already on the wire and
  unconsumed; the V6 receipt keeps its fields and signer; decision tokens stay
  integrator-signed. The on-chain layer attaches *underneath* them.

## 7. Facts this proposal rests on (re-verify before acting)

| Fact | Checked | Source |
|---|---|---|
| Rails live hash `3128b644…`, slot 428118420, pre-H-01 | 2026-09-05 RPC | `ftWybjbY…` ProgramData |
| Upgrade authority `8di6hHF8…` (both programs), funded 0.178 SOL | 2026-09-03 RPC | ProgramData headers |
| `Config.admin` = `2pHjZL…`, 0 lamports | 2026-09-05 RPC | `7pwUU1hv…` bytes 8..40 |
| Listen-payout config PDAs absent | 2026-09-03 RPC | `D6vVhAmE…`, `Ebxykho1…`, `B4mUpjzU…` |
| `register_verified_moment` present in deployed rails | git | `d441724:programs/wzrd-rails/src/lib.rs` |
| Gate proofs unsigned; `onchain_settlements` caller-supplied; `readiness_card.proof` unconsumed | code read | `twzrd-x402-gate/dist/{block-proof,adoption-proof}.js`, `types.d.ts` |
| V6 receipt = single keccak leaf, no proof path; live issuer `Ak5SQwHpuQA…` (v2), not v1 `9V6Pn19k…` | 2026-09-06 `/health` + code | intel `receipt_signing.key_id=twzrd-receipt-ed25519-v2` |
| No Solana RPC read anywhere in `twzrd-trust`; `delivery-capture.js` names it as pending | code read | `twzrd-x402-gate/dist/delivery-capture.js` |
| Markets leaf `{market_id, streamer_ref, window_id, metric, observed_value, outcome}` | source | `programs/wzrd-markets/src/resolution.rs` |
