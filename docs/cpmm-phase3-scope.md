# Phase 3 Scope: Resolution + Settlement (the payoff layer)

> **Status**: Scope. Builds on Phase 2 (`claude/cpmm-markets-phase2`, tip cc12453).
> **Branch**: `claude/cpmm-markets-phase3`
> **Goal**: a market that actually *settles*. Publish an attention root, resolve a market
> against the create-time-snapshotted root via a merkle proof, run a dispute window, then
> burn the winning outcome 1:1 for USDC. Plus the never-resolved fallback and a multisig
> override.
> **Hard gate #1**: `docs/cpmm-merkle-conventions-v1.md` is LOCKED FIRST. No merkle code
> before it. The §4 rejection test (wrong-domain proof REJECTED) is the Phase 3 equivalent
> of Phase 2's arb gate.

---

## 0. The four locked parameters (decided, not guessed)

| Parameter | Decision | Encoding |
|-----------|----------|----------|
| **Dispute window default** | `54_000` slots (~6h) | `MarketsConfig.default_dispute_window_slots: u64`. Per-market `dispute_window_slots` snapshotted at create (Phase 1 H-01). Admin may **extend once** per market via `extend_dispute_window`. |
| **resolve_override multisig** | Config-stored M-of-N (no hardcode) | `MarketsConfig.resolver_multisig` (member set) + `resolver_threshold: u8`. Set at init. **Member set MUST be disjoint from `admin`** (validated at init). |
| **Never-resolved fallback** | Complete-set redeem stays open after deadline | After `resolve_deadline_slot`, `redeem_complete_set` remains callable (it already requires `!resolved`; a never-resolved market is `!resolved` forever). Single-side holders buy the cheap side via the Phase 2 pool. Preserves MR-1 exactly. |
| **Outcome encoding** | Binary + INVALID | `Outcome` u8: `No = 0`, `Yes = 1`, `Invalid = 2`. INVALID routes to the redeem path (both sides 1:1) — no separate mechanism. |

---

## 1. Instructions

| Instruction | Auth | Purpose |
|-------------|------|---------|
| `publish_attention_root` | allow-listed publisher | Publish a daily/window attention merkle root + window metadata on-chain (mirrors rails `publish_listen_payout_root`). The off-chain builder uses the v1 convention. |
| `resolve_market` | allow-listed publisher (or permissionless w/ valid proof — decision below) | Verify a resolution-leaf merkle proof against the market's **create-time snapshotted** root, set `outcome` + `resolved_at_slot`, start the dispute window. |
| `extend_dispute_window` | admin | Extend a market's dispute window ONCE (bounded). Defense for a contested resolution. |
| `settle` | permissionless (holder calls for themselves) | After the dispute window closes, burn the caller's winning-outcome tokens 1:1 for USDC. Losing/invalid handled per §4. |
| `resolve_override` | multisig (M-of-N, disjoint from admin) | Emergency: override a wrong/contested resolution within the dispute window. Sets/changes `outcome`, emits an event, restarts a short re-dispute window. |
| `sweep_residual` | admin | After settle is fully drained (all winning supply burned), sweep dust collateral. Guard: `winning_supply == 0`. |
| `close_market` | admin | Reclaim the Market/Pool account rent after full settlement + sweep. Guard: supplies == 0, vault drained to dust. |

**`resolve_market` auth decision**: gate on the **allow-listed publisher** for v1 (the
publisher is the entity that published the root, and resolution interpretation — which leaf
is the canonical outcome leaf — is a curated step for v1). Permissionless resolution (anyone
with a valid proof can trigger) is a later decision; document this as a Phase-3 trust choice.
The dispute window + multisig override are the checks on publisher error.

---

## 2. `publish_attention_root` (the in-house publisher)

Mirrors wzrd-rails `publish_listen_payout_root` (audited, H-01-hardened):
- **Auth**: `require!(publisher in config.publisher_allowlist)` — reuse the Phase-0
  `publisher_allowlist` on `MarketsConfig`. Allow-list validated like rails
  `validate_payout_publishers` (non-empty, <= MAX, no `Pubkey::default()`, no dups).
- Publish an `AttentionRoot` account keyed by `[ATTENTION_ROOT_SEED, window_id.to_le_bytes()]`:
  `merkle_root: [u8;32]`, `window_id: u64`, `leaf_count: u32`, `schema_version: u8`,
  `published_at_slot: u64`, `publisher: Pubkey`.
- `require!(merkle_root != [0u8;32], MarketsError::ZeroResolutionRoot)`.
- One root per window (`init` the PDA; re-publish of the same window_id fails — the account
  already exists). A correction goes through a new window or the multisig override.
- Emit `AttentionRootPublished`.

**Relationship to create-time snapshot (H-01)**: `create_market` (Phase 1) already
snapshotted `resolution_root` + `resolution_root_seq` at create. `publish_attention_root` is
how that root came to exist *before* the market was created (the publisher publishes; then a
market is created bound to that published root). `resolve_market` verifies against
`market.resolution_root` (the snapshot), NOT against a live `AttentionRoot` account that
could change — this is the finality guarantee. The `AttentionRoot` account exists for
discoverability/indexers and to let the publisher commit `leaf_count`; the *binding* root for
resolution is the snapshot on the Market.

---

## 3. `resolve_market` (proof verification — the merkle gate)

Uses the locked convention (`docs/cpmm-merkle-conventions-v1.md` §3) verbatim:
1. `require!(!market.resolved, MarketsError::MarketAlreadyResolved)`.
2. `require!(clock.slot <= market.resolve_deadline_slot, MarketsError::ResolutionDeadlinePassed)`
   — cannot resolve after the never-resolved fallback has effectively taken over.
3. Cap check: `require!(proof.len() <= MARKETS_MAX_PROOF_LEN, ProofTooLong)`.
4. `leaf.hash()` (markets leaf domain) → fold via `markets_resolution_node_hash_v1` →
   `require!(current == market.resolution_root, InvalidMerkleProof)`.
5. Leaf-to-market binding: `leaf.market_id == market.market_id`,
   `leaf.streamer_ref == market.streamer_ref`, `leaf.metric == market.metric`.
6. Set `market.outcome = leaf.outcome`, `market.resolved = true`,
   `market.resolved_at_slot = clock.slot`,
   `market.settle_unlock_slot = clock.slot + market.dispute_window_slots`.
7. Emit `MarketResolved { market_id, outcome, observed_value, resolved_at_slot, settle_unlock_slot }`.

**INVALID outcome**: if `leaf.outcome == Invalid`, set `market.resolved = true`,
`market.outcome = Invalid`. `settle` then refuses (INVALID has no winning side); holders use
`redeem_complete_set` (still gated `!won`/ via the invalid branch — see §4).

---

## 4. `settle` (burn winning side 1:1 — preserve MR-1)

- `require!(market.resolved, MarketsError::MarketNotResolved)`.
- `require!(clock.slot >= market.settle_unlock_slot, MarketsError::DisputeWindowOpen)` — the
  dispute window must have elapsed.
- **INVALID branch**: `require!(market.outcome != Invalid, MarketsError::MarketInvalidUseRedeem)`
  — INVALID markets settle via `redeem_complete_set` (both sides), not `settle`.
- Binary branch: the winning mint = `yes_mint` if `outcome == Yes` else `no_mint`.
  - Burn `amount` of the caller's winning-outcome tokens.
  - Transfer `amount` USDC from the vault to the caller (vault-authority PDA signs).
  - The losing-outcome tokens are worth 0 (never redeemable post-resolution); they simply
    remain (or can be burned by the holder with no payout — not required).
- **MR-1 preservation**: before resolution `vault == yes_supply == no_supply`. After
  resolution, only the winning side is redeemable. The invariant that protects solvency:
  **`vault.amount >= winning_mint.supply`** must hold at all times post-resolution (every
  winning token can be settled 1:1; the vault is never short). Because complete-set minting
  kept `vault == winning_supply` at resolution and settle burns 1 winning + removes 1 USDC in
  lockstep, the invariant is maintained. The losing supply does NOT have a USDC claim, so the
  vault has exactly enough for the winning side. **Test this explicitly (the Phase 3 gate).**
- **Never-resolved**: if the deadline passes with `!resolved`, `redeem_complete_set` is the
  recovery path (unchanged from Phase 1 — both sides 1:1). Single-side holders rebalance via
  the pool. No new code; a test proves redeem still works post-deadline on an unresolved
  market.
- Emit `Settled { market_id, winner, amount, settler }`.

---

## 5. `resolve_override` (multisig, disjoint from admin)

- **Auth**: M-of-N over `config.resolver_multisig` with `config.resolver_threshold`. The
  instruction takes the signing members as `remaining_accounts` (or explicit signer slots);
  `require!` that >= threshold distinct members from the set signed, AND that none of them is
  the `admin` (disjointness already enforced at init, re-assert defensively).
- **Window**: callable only while the dispute window is open OR within a bounded grace after
  resolution (`clock.slot <= market.settle_unlock_slot`). Override after settle has begun is a
  separate, harder problem (funds already moved) — **out of scope; document that override is a
  pre-settle remedy only.**
- Sets `market.outcome` to the corrected value, emits `ResolutionOverridden { market_id,
  old_outcome, new_outcome, signers_count }`, and **restarts** a short re-dispute window
  (`settle_unlock_slot = clock.slot + min(dispute_window_slots, OVERRIDE_REDISPUTE_SLOTS)`).
- Override can also set `Invalid` (escape hatch for a market that cannot be honestly resolved).

---

## 6. `sweep_residual` / `close_market` (cleanup, admin, supply==0 guards)

- `sweep_residual`: `require!(winning_supply == 0)` (everyone settled) → transfer remaining
  vault dust to treasury. For INVALID markets: `require!(yes_supply == 0 && no_supply == 0)`
  (everyone redeemed).
- `close_market`: `require!` supplies == 0 and vault drained to <= dust threshold → close the
  Market (and Pool, if empty) accounts, return rent to admin/recipient.
- These are housekeeping; the guards prevent closing a market with live obligations.

---

## 7. State additions (Phase 0/1 left room)

- `Market`: add `outcome: u8` (Outcome enum-as-u8), `resolved_at_slot: u64`,
  `settle_unlock_slot: u64`. (`resolved: bool`, `resolution_root`, `resolution_root_seq`,
  `resolve_deadline_slot`, `dispute_window_slots` already exist from Phase 1.) Carve from
  `Market._reserved` if present; else this is the one place a `Market` LEN bump is acceptable
  — but Phase 1 reserved room, so **carve from reserve, no realloc** (confirm during build).
- `MarketsConfig`: add `default_dispute_window_slots: u64`, `resolver_threshold: u8`
  (`resolver_multisig` member set + `publisher_allowlist` already exist from Phase 0). Carve
  from `_reserved` (Phase 0 left 56-64 bytes); **no LEN change / no realloc** — confirm the
  reserve has room for 8 + 1 bytes (it does).
- New accounts: `AttentionRoot` PDA (per window). No per-settle account (settle is stateless
  beyond burning + transferring; replay is naturally prevented because burned tokens can't be
  re-burned).

---

## 8. Errors (add to MarketsError)

`ProofTooLong`, `InvalidMerkleProof`, `LeafMarketMismatch`, `LeafStreamerMismatch`,
`LeafMetricMismatch`, `MarketAlreadyResolved`, `MarketNotResolved`,
`ResolutionDeadlinePassed`, `DisputeWindowOpen`, `MarketInvalidUseRedeem`,
`UnauthorizedPublisher`, `ZeroResolutionRoot`, `MultisigThresholdNotMet`,
`OverrideWindowClosed`, `MultisigMemberIsAdmin`, `WindowAlreadyPublished`,
`SupplyNotZero`, `DisputeAlreadyExtended`.

---

## 9. THE ACCEPTANCE GATES (must pass or Phase 3 is NOT done)

**Gate A — the merkle rejection test** (`docs/cpmm-merkle-conventions-v1.md` §4, cases 1-7):
wrong-node-domain proof REJECTED, wrong-leaf-domain REJECTED, overlong REJECTED, tampered
sibling REJECTED, leaf-for-wrong-market REJECTED, self/unsorted proof REJECTED, valid proof
ACCEPTED. **Cases 1-2 are the M-04/CH-3 silent-failure kill switches.**

**Gate B — post-resolution solvency** (`settle_preserves_vault_solvency`): after a real
resolve→dispute-elapse→settle cycle, assert `vault.amount >= winning_supply` holds across
every partial settle; full settle drains vault to exactly 0 with winning_supply 0. No
over-pay, no short vault.

**Gate C — never-resolved recovery** (`unresolved_market_redeems_after_deadline`): a market
past `resolve_deadline_slot` with `!resolved` — `redeem_complete_set` returns full collateral
1:1; MR-1 baseline restored.

Plus required functional/boundary tests:
1. `publish_attention_root` happy path + non-publisher rejected + zero-root rejected +
   double-publish-same-window rejected.
2. `resolve_market` happy path (sets outcome, starts window) + resolve-after-deadline
   rejected + resolve-already-resolved rejected.
3. Dispute window: `settle` before `settle_unlock_slot` reverts `DisputeWindowOpen`; after, succeeds.
4. `extend_dispute_window`: extends once; second extend reverts `DisputeAlreadyExtended`.
5. `resolve_override`: < threshold signers reverts; admin-as-signer reverts; valid M-of-N
   overrides + restarts re-dispute; override-to-INVALID works; override after settle-window
   closed reverts.
6. INVALID outcome: `settle` reverts `MarketInvalidUseRedeem`; `redeem_complete_set` works.
7. `sweep_residual` / `close_market`: blocked while supply > 0; succeeds when drained.
8. Multisig member set disjoint from admin enforced at init.
9. Curve proptests + Phase 1 complete-set gate + Phase 2 arb gate all regression-pass.

---

## 10. Non-negotiables (carried + new)

- **Gate #1 first**: conventions one-pager locked before any merkle code. DONE — this scope
  references it.
- **Proof verifies against the create-time snapshot** (`market.resolution_root`), never a
  live mutable root. (H-01.)
- **ONE keccak convention** (the audited listen-payout v1, adopted verbatim with markets
  domains). No second/third convention. (M-04/CH-3.)
- **Leaf bound to the market** (market_id + streamer_ref + metric asserted post-verify).
- **MR-1 solvency preserved** through settle (`vault >= winning_supply` always).
- **Multisig disjoint from admin** (no single point can both resolve and override).
- **Dispute window real** (settle blocked until it elapses; override is the in-window remedy).
- Vault-authority PDA signer seeds for settle transfers byte-identical to Phase 1 init.
- Fee-free (USDC collateral fee-exempt; outcome mints fee-free) — unchanged.

---

## 11. Out of scope for Phase 3

- Permissionless resolution (anyone-with-proof triggers) — v1 is publisher-gated.
- Override AFTER settle has moved funds (clawback) — override is pre-settle only.
- Jupiter routing metadata, Switchboard On-Demand wiring — Phase 4.
- Multi-outcome (>2 + invalid) / scalar markets — binary + invalid only.
- The off-chain tree builder implementation — Phase 3 ships the on-chain verifier + the
  convention + golden vectors; the builder is a server/keeper task that conforms to the
  golden vectors (the mirror contract in conventions §2).

---

## 12. Verification checklist (the independent gate before commit)

1. `cargo build -p wzrd-markets` (host) + `cargo-build-sbf` — both clean.
2. **Gate A** (merkle rejection, §4 cases 1-7) passes — esp. wrong-domain REJECTED.
3. **Gate B** (post-resolution solvency `vault >= winning_supply`) passes.
4. **Gate C** (never-resolved redeem after deadline) passes.
5. Functional/boundary tests §9.1-§9.8 pass.
6. Golden vectors for the resolution leaf present + passing (mirror contract).
7. Curve proptests + Phase 1 + Phase 2 gates regression-pass.
8. Anti-drift checklist (conventions §5) all checked.
9. No prod unwraps in the new resolution/settle paths.
10. Multisig disjoint-from-admin enforced; vault signer seeds verified.
