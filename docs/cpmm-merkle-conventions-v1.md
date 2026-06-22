# wzrd-markets Merkle / Proof Conventions v1 (THE GATE — read before any merkle code)

> **Status**: Locked. Owner: Luna (conventions) / Henry (silent-failure gate).
> **Why this file exists**: The audit's single highest-leverage silent-failure risk
> (M-04 / CH-3) is *two divergent keccak conventions coexisting in one program* — a
> one-byte drift between the prover (off-chain) and the verifier (on-chain) makes every
> proof silently unverifiable, OR (worse) a wrong-domain proof gets silently accepted.
> Phase 3 introduces merkle proof verification (`resolve_market`). This file pins the
> ONE convention it must use, byte-for-byte, BEFORE any merkle code is written.
>
> **Hard rule (Henry's gate)**: No `resolve_market` / proof-verification code merges
> until (a) this file is locked, and (b) a test proves a *wrong-domain* or *malformed*
> proof is **REJECTED, not silently accepted**. That test is the Phase 3 equivalent of
> Phase 2's arb-coherence gate.

---

## 0. The decision: adopt the audited `listen-payout v1` convention verbatim

wzrd-rails (the audited program, in this same repo at `programs/wzrd-rails/`) already
ships a hardened, golden-vector-tested merkle convention in
`programs/wzrd-rails/src/listen_payout.rs`. **wzrd-markets reuses that exact convention**
— same hash library, same sorted-pair node hashing, same proof-length cap — with its own
leaf domain string. We do NOT invent a new scheme, and we do NOT introduce a second one.

**The audit found TWO conventions already coexisting in wzrd-rails** (this is the concrete
M-04/CH-3 instance):

| Path | Leaf hash | Node hash | Domain-separated nodes? |
|------|-----------|-----------|-------------------------|
| **listen-payout** (`listen_payout_node_hash_v1`) | `keccak(LEAF_DOMAIN ‖ canonical_bytes)` | `keccak(NODE_DOMAIN ‖ sorted(L,R))` | **YES** — `...-node:v1` |
| **compensation** (`verify_compensation_proof`) | `keccak(COMPENSATION_LEAF_DOMAIN ‖ user ‖ amount)` | `keccak(sorted(L,R))` — **no node domain** | **NO** |

The listen-payout convention is the **stronger** one (node-domain separation prevents
second-preimage / leaf-as-node confusion). **wzrd-markets adopts the listen-payout
convention. It MUST NOT copy the compensation convention, and MUST NOT add a third.**

---

## 1. The locked convention (every byte pinned)

### 1.1 Hash library — ONE only
```rust
use solana_keccak_hasher as keccak;   // identical import to wzrd-rails
// keccak::hashv(&[parts...]).to_bytes()  -> [u8; 32]
```
- **keccak256**, via `solana_keccak_hasher::hashv`. No `sha2`, no `blake3`, no
  `anchor_lang::solana_program::hash` (that's sha256), no second keccak crate.
- Rationale: matches the live, audited, golden-tested rails path exactly. A second hash
  lib is the exact drift the gate exists to prevent.

### 1.2 Leaf domain — wzrd-markets-specific, distinct from rails
```rust
pub const MARKETS_RESOLUTION_LEAF_V1_DOMAIN: &[u8] =
    b"wzrd-markets:attention-resolution-leaf:v1";
```
- The leaf domain is **different** from the rails leaf domain. A rails listen-payout proof
  must NEVER verify against a markets resolution root and vice versa — different domain
  strings guarantee this.

### 1.3 Node domain — wzrd-markets-specific, distinct from the leaf domain
```rust
pub const MARKETS_RESOLUTION_NODE_V1_DOMAIN: &[u8] =
    b"wzrd-markets:attention-resolution-node:v1";
```
- The node domain MUST differ from the leaf domain (so a leaf hash can never be confused
  for an internal node hash — second-preimage defense). Mirrors the rails leaf/node split.

### 1.4 Node hashing — sorted-pair, domain-separated (copy rails exactly)
```rust
/// Sorted-pair merkle node hash for wzrd-markets resolution trees.
pub fn markets_resolution_node_hash_v1(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let (first, second) = if left <= right {
        (left.as_slice(), right.as_slice())
    } else {
        (right.as_slice(), left.as_slice())
    };
    keccak::hashv(&[MARKETS_RESOLUTION_NODE_V1_DOMAIN, first, second]).to_bytes()
}
```
- **Sorted pair**: smaller 32-byte value first (lexicographic on the raw bytes). This is
  what lets the proof omit left/right flags — the prover and verifier both sort.
- Byte-identical structure to `listen_payout_node_hash_v1` (lib only differs in the domain
  constant). Do not "improve" it.

### 1.5 Proof-length cap — reuse the rails value
```rust
pub const MARKETS_MAX_PROOF_LEN: usize = 16;   // == wzrd-rails MAX_PROOF_LEN
```
- 16 siblings ⇒ up to 2^16 = 65,536 leaves per tree. More than enough for per-streamer
  resolution; bounds compute and rejects a maliciously-long proof (DoS / compute-exhaustion).
- **Enforced as a `require!` BEFORE the fold loop** (see §3), exactly as rails does at
  `lib.rs:986-989`.

### 1.6 The resolution leaf (what a market resolves against)

Phase 3 decision — the resolution leaf commits the market's outcome for a streamer at a
window. Canonical byte layout (little-endian, fixed order — mirror the rails discipline):

```text
domain ‖
schema_version:u8                 (= 1)
market_id:u64_le                  // binds the leaf to exactly one market
streamer_ref:32_bytes             // same streamer identity commitment used at create
window_id:u64_le                  // the attention window this resolves
metric:u8                         // MarketMetric (must match market.metric)
observed_value:u64_le             // the measured metric value at resolution
outcome:u8                        // 0 = NO, 1 = YES, 2 = INVALID (see §4 of scope)
```
- `CANONICAL_LEN = 1 + 8 + 32 + 8 + 1 + 8 + 1 = 59` bytes.
- `market_id` + `metric` are bound into the leaf so a proof valid for market A under metric
  X cannot be replayed against market B or a different metric.
- Leaf hash: `keccak(MARKETS_RESOLUTION_LEAF_V1_DOMAIN ‖ canonical_bytes)`.
- **`resolve_market` MUST assert `leaf.market_id == market.market_id`,
  `leaf.streamer_ref == market.streamer_ref`, and `leaf.metric == market.metric`** after
  the proof verifies — the proof proves inclusion in the root; these asserts prove the leaf
  is for *this* market. (The audit's M-04 lesson: a verified proof against the wrong leaf is
  still wrong.)

### 1.7 Golden vectors — MANDATORY (the rails discipline, non-negotiable)

Like `listen_payout.rs`, the markets leaf module MUST ship:
1. A **golden hash** for a fixed non-zero fixture leaf (locks the byte order + domain).
2. An **all-zero vector** golden hash (determinism baseline).
3. A **field-binding test**: flipping any one field (market_id, streamer_ref, window_id,
   metric, observed_value, outcome) changes the hash.
4. A **sorted-pair test**: `node_hash(L,R) == node_hash(R,L)` and a golden node hash.

If any golden hash later changes, the byte order or a domain drifted — STOP and fix before
shipping. (This is the off-chain/on-chain mirror contract: the in-house publisher's Rust/TS
tree builder MUST produce the same hashes. If the off-chain builder lives in a shared crate,
add the mirror assertion there too.)

---

## 2. Off-chain ↔ on-chain mirror contract

The in-house publisher (Phase 3 `publish_attention_root` is on-chain; the *tree builder* is
off-chain — server or a keeper) MUST build trees with the **identical** convention:
- same keccak lib, same leaf domain, same node domain, same sorted-pair rule, same canonical
  byte layout, same `schema_version`.
- The golden vectors in §1.7 are the contract. The builder's unit tests assert the same
  golden hashes. A mismatch = silent unverifiability = the M-04 failure. The golden vector is
  what makes the mismatch LOUD (a failing test) instead of silent (an unverifiable proof in
  production).

---

## 3. The verifier shape (locked — `resolve_market` proof check)

```rust
// 1. Cap FIRST (reject over-long proof before doing any work).
require!(proof.len() <= MARKETS_MAX_PROOF_LEN, MarketsError::ProofTooLong);

// 2. Leaf hash with the LEAF domain.
let mut current = leaf.hash();   // keccak(LEAF_DOMAIN ‖ canonical_bytes)

// 3. Fold siblings with the NODE-domain sorted-pair hash.
for sibling in proof.iter() {
    current = markets_resolution_node_hash_v1(&current, sibling);
}

// 4. Compare to the SNAPSHOTTED root (H-01: the root captured at create-time, NOT a
//    live/mutable root). market.resolution_root is the create-time snapshot.
require!(current == market.resolution_root, MarketsError::InvalidMerkleProof);

// 5. Bind the leaf to THIS market (a verified proof against the wrong leaf is still wrong).
require!(leaf.market_id == market.market_id, MarketsError::LeafMarketMismatch);
require!(leaf.streamer_ref == market.streamer_ref, MarketsError::LeafStreamerMismatch);
require!(leaf.metric == market.metric, MarketsError::LeafMetricMismatch);
```

This is byte-for-byte the rails listen-payout verifier shape (`lib.rs:986-998`) plus the
leaf-binding asserts.

---

## 4. THE REJECTION TEST (Henry's gate — Phase 3 is NOT done without it)

A test that proves a **wrong-domain or malformed proof is REJECTED, not silently accepted.**
Minimum required cases (litesvm or unit, behind `localtest`):

1. **`resolve_rejects_wrong_node_domain`** — build a tree using a DIFFERENT node domain
   (e.g. the rails `...-node:v1` domain, or `...-node:v2`), produce a proof, call
   `resolve_market`. Assert it reverts `InvalidMerkleProof`. (This is the headline
   silent-failure kill switch: a proof from the *other* convention must not verify.)
2. **`resolve_rejects_wrong_leaf_domain`** — leaf hashed with the rails leaf domain instead
   of the markets leaf domain → reverts `InvalidMerkleProof`.
3. **`resolve_rejects_overlong_proof`** — `proof.len() == MARKETS_MAX_PROOF_LEN + 1` →
   reverts `ProofTooLong` (BEFORE the fold, so it's cheap and unconditional).
4. **`resolve_rejects_tampered_sibling`** — flip one byte in a valid proof's sibling →
   reverts `InvalidMerkleProof`.
5. **`resolve_rejects_leaf_for_wrong_market`** — a proof that verifies against the root but
   whose `leaf.market_id` != the market being resolved → reverts `LeafMarketMismatch`.
6. **`resolve_rejects_unsorted_or_self_proof`** — a proof that tries to pass the leaf itself
   as a sibling or relies on unsorted pairing → reverts `InvalidMerkleProof`.
7. **`resolve_accepts_valid_proof`** (the positive control) — a correctly-built proof under
   the markets convention verifies and flips `market.resolved = true` with the right outcome.

A passing positive control with NO negative controls is a false sense of security. **All of
1-7 must pass.** Cases 1 and 2 are the literal M-04/CH-3 kill switches — without them, the
gate is not satisfied.

---

## 5. Anti-drift checklist (apply before Phase 3 commit)

- [ ] Exactly ONE keccak lib in the markets resolution path (`solana_keccak_hasher`). Grep
      the new code for `sha2`, `blake3`, `sha256`, a second keccak crate — zero hits.
- [ ] Leaf domain ≠ node domain ≠ rails domains (4 distinct strings; grep to confirm).
- [ ] `MARKETS_MAX_PROOF_LEN` checked with `require!` BEFORE the fold loop.
- [ ] Root compared is `market.resolution_root` (the H-01 create-time snapshot), never a
      live config root.
- [ ] Leaf-to-market binding asserts present (market_id, streamer_ref, metric).
- [ ] Golden vectors present and passing (leaf, all-zero, field-binding, node).
- [ ] Rejection test §4 cases 1-7 all present and passing.
- [ ] No copy of the *compensation* convention (no `sorted_pair_hash` without a node domain
      in the markets path).
