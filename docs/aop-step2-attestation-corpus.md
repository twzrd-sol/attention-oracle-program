# AOP step 2: attestation + corpus-root upgrade (design)

**Status**: design (2026-09-18). Unblocks the implementation PR. Not source. Not a
deploy. Not a pin refresh.
**Parent**: `docs/aop-trust-rail-repositioning.md` §4 item 2 and §5 decisions 1-4.
**Seam**: `docs/trust-rail-aop-seam.md`. Evidence hierarchy unchanged
(`rebuild` > `artifact_hash` > `unreproduced`). `settlement_tx` stays
`program: none` / `verification: ledger` (decision 1) and is **not** this upgrade.
**Adapter step 1**: wzrd-final #2831, live as of 2026-09-18. Re-verify
`GET https://intel.twzrd.xyz/health` before citing a SHA.

This document freezes account layouts, instruction surfaces, PDA seeds, and the
authority model so the next PR can implement without re-litigating. It does not
add IXs to `programs/wzrd-rails`.

## 1. Why this upgrade, and what it is not

Step 1 closed "nothing reads Solana state" with zero AOP work. The remaining
holes the parent named are on-chain:

| Hole | This upgrade |
|---|---|
| Gate transcripts are unsigned self-claims | `register_attestation` PDA, kind `GATE_TRANSCRIPT` |
| V6 receipt `leaf` is a single keccak with no merkle path | `publish_corpus_root` under new domains |
| `merchant_attach` is issuer-only | same corpus root class, leaf `kind=MERCHANT_ATTACH` |
| New IXs must not inherit retired `Config.admin` | own `AttestationAuthorityConfig` PDA |

Not in this upgrade:

- Config.admin rotation (`docs/playbooks/wzrd-rails-config-admin-rotation.md`)
- Listen bootstrap (still `Config.admin` + `payer = admin`; still refuse in the seam)
- On-chain claim / redeem / CCM movement / vault
- `register_verified_moment` overload (decision 4)
- Markets v2 leaf (parent §4 item 5)
- Adapter consume of `gate_transcript` / `receipt_root` (wzrd-final follow-on)
- Mainnet send, `CONFIRM_BROADCAST`, Doppler, pin rewrite

## 2. Authority model (the load-bearing choice)

Listen init is `has_one = config.admin` with `payer = admin`. That is why the
listen rail cannot start: `Config.admin` is still retired `2pHjZL…` at 0
lamports. New trust-rail IXs **must not** appear in any accounts struct that
mentions `Config`.

### 2.1 First attester is a program constant

Decision 3: first allow-list member is the **live** V6 receipt issuer
`Ak5SQwHpuQAqU7ty7ZWX7qgF39A9yi72c22KNn8sHzvS`
(`twzrd-receipt-ed25519-v2` on `intel.twzrd.xyz/health` as of 2026-09-18).

```rust
pub const LIVE_V6_ISSUER: Pubkey =
    pubkey!("Ak5SQwHpuQAqU7ty7ZWX7qgF39A9yi72c22KNn8sHzvS");
```

Never on the list: v1 `9V6Pn19k…`, any feePayer (live `/health.fee_payer` is
gas, currently `BFK9TLC3…`; do not freeze a feePayer pubkey in this program),
retired `2pHjZL…`. Do not freeze additional members in the binary.

### 2.2 Init proves custody, does not squat

`init_attestation_authority_config` has **no `Config` account**.

| Account | Role |
|---|---|
| `attester` | `Signer`, must equal `LIVE_V6_ISSUER` (proof of custody) |
| `payer` | `Signer` + `mut`, pays rent; may be `attester` or a separate fee payer |
| `authority_config` | `init`, seeds `[ATTESTATION_AUTHORITY_CONFIG_SEED]` |
| `system_program` | |

Args: none. Admin and the initial attester are both `LIVE_V6_ISSUER`. A
caller-supplied admin would re-introduce the set-admin typo class (pubkeys
carry no checksum). First-init wins; double-init is Anchor `already in use`.

### 2.3 Two-step admin from day one

H-01 taught that 1-step `set_payout_admin` on the live binary is a footgun.
Attestation admin rotation is propose/accept from the first binary that has
it:

- `propose_attestation_admin(new_admin)` — live admin signs; writes
  `pending_admin`. Reject `Pubkey::default()`.
- `accept_attestation_admin` — `pending_admin` co-signs; copies into `admin`,
  clears pending.

No `Config.admin` emergency path. There is no pause, so there is nothing to
unpause.

### 2.4 No pause field

Parent: "mirrors `PayoutAuthorityConfig` **minus pause**" so the
H-01-class pause/claim coupling cannot recur. Attestations and corpus roots
are write-once facts. A compromised attester is handled by rotating the
allow-list, not by freezing published PDAs.

## 3. Accounts

Seeds (new; do not reuse listen seeds):

```text
ATTESTATION_AUTHORITY_CONFIG_SEED = b"attestation_authority_config"
ATTESTATION_SEED                  = b"attestation"
CORPUS_ROOT_SEED                  = b"corpus_root"
```

### 3.1 `AttestationAuthorityConfig`

PDA: `[ATTESTATION_AUTHORITY_CONFIG_SEED]`

```text
bump:                 u8
attesters:            Vec<Pubkey>   // MAX_ATTESTERS = 8, same cap as publishers
admin:                Pubkey
pending_admin:        Pubkey        // default = no rotation pending
last_published_seq:   u64           // corpus monotonicity; 0 = none yet
```

No `_reserved`. Body size excluding 8-byte discriminator:
`1 + 4 + (32 * 8) + 32 + 32 + 8 = 333`.

`attester_allowed(p)` is the listen `publisher_allowed` copy.

### 3.2 `Attestation`

PDA: `[ATTESTATION_SEED, claim_id:16]`

```text
bump:              u8
version:           u8      // 1
claim_id:          [u8; 16]
kind:              u8      // see §4.1
hashes:            [[u8; 32]; 4]
subject:           Pubkey
registered_by:     Pubkey
created_slot:      u64
created_unix_ts:   i64
```

Init-only. No update IX. Duplicate `claim_id` is `already in use`. Body = 227
bytes.

### 3.3 `CorpusRoot`

PDA: `[CORPUS_ROOT_SEED, seq.to_le_bytes()]`

```text
bump:               u8
seq:                u64
merkle_root:        [u8; 32]
leaf_count:         u32
schema_version:     u8     // 1
published_by:       Pubkey
published_at_slot:  u64
```

No `total_amount_ccm`. No `claim_bitmap`. No vault. Body = 86 bytes.
Republish of the same `seq` is `already in use` (init PDA).

## 4. Instructions

All of these omit `Config`. CI on the implementation PR must grep the new
accounts structs for `config` / `CONFIG_SEED` / `Config.admin` and fail if
present.

### 4.1 `register_attestation`

```text
register_attestation(claim_id: [u8;16], kind: u8, hashes: [[u8;32]; 4], subject: Pubkey)
```

Signer in `attesters`. Checks:

- `kind == AttestationKind::GATE_TRANSCRIPT` (value `1`). `0` reserved.
  `2+` refuse. No other kind in v1.
- `subject != Pubkey::default()`
- every `hashes[i] != [0u8; 32]`
- `claim_id` is the seed; not independently stored from a second copy that
  could drift

`GATE_TRANSCRIPT` hash slots (off-chain convention, not enforced beyond
non-zero):

| Slot | Content |
|---|---|
| 0 | intent hash |
| 1 | transcript hash |
| 2 | preflight-response hash |
| 3 | decision-token hash |

If a slot has no payload, the publisher hashes a **typed** empty string
(`keccak("twzrd.attestation.empty.v1")`) rather than writing zeros. The
program never bakes that sentinel in; zeros stay illegal.

`claim_id` is the decision-token UUID (16 bytes), matching the parent’s
`register_verified_moment` mapping. This IX is the purpose-built replacement;
do not call `register_verified_moment`.

### 4.2 `publish_corpus_root`

```text
publish_corpus_root(seq: u64, merkle_root: [u8;32], leaf_count: u32, schema_version: u8)
```

Signer in `attesters`. Checks (listen publish, minus pause / cap / CCM):

- `schema_version == CORPUS_RECEIPT_LEAF_SCHEMA_V1` (`1`)
- `seq > last_published_seq`
- `seq <= MAX_WINDOW_ID` (`99_999_999`) — same publisher-boundary guard
- `leaf_count > 0 && leaf_count <= MAX_LEAVES_PER_WINDOW` (`32_768`)
- `merkle_root != [0u8; 32]`

Then writes the PDA and sets `last_published_seq = seq`.

No on-chain verify/claim IX in this upgrade. The adapter proves a leaf
against the PDA the same way listen proofs work, with the domains below.
Adding a program-side verify later is a separate slice (it is not required
for `BindDecision` hard bind).

### 4.3 Allow-list + admin

- `set_attestation_allowlist(attesters: Vec<Pubkey>)` — live admin. Non-empty,
  unique, no default pubkey, `len <= 8`, must keep `LIVE_V6_ISSUER` as a
  member (the binary’s first attester cannot be voted off; additional members
  are free).
- `propose_attestation_admin` / `accept_attestation_admin` — §2.3.

## 5. Corpus merkle convention

Reuse the audited listen recipe with **new domain strings**. GATE A stands:
listen roots and corpus roots are not interchangeable.

```text
CORPUS_RECEIPT_LEAF_V1_DOMAIN = b"wzrd-rails:corpus-receipt-leaf:v1"
CORPUS_RECEIPT_NODE_V1_DOMAIN = b"wzrd-rails:corpus-receipt-node:v1"
MAX_PROOF_LEN                 = 16   // unchanged
```

Parent wrote `wzrd-rails:corpus-receipt-{leaf,node}:v1`. The listen pattern
spells `...-leaf:v1` / `...-node:v1` as separate constants. Same bytes either
way; implement as two constants, golden-vector them.

### 5.1 `CorpusReceiptLeafV1`

A V6 receipt `leaf` is already one keccak of a flat preimage. This wrapper
puts that digest in a domain-separated tree so a listen proof cannot verify
against a corpus root.

Canonical encoding:

```text
schema_version: u8          // 1
seq:            u64 le
leaf_index:     u32 le
kind:           u8          // 1 = RECEIPT, 2 = MERCHANT_ATTACH
signing_pubkey: 32 bytes    // must be LIVE_V6_ISSUER for v1 publishes
v6_leaf:        32 bytes    // the V6 keccak; or the attach leaf keccak
subject:        32 bytes    // scored / attached wallet
salt:           16 bytes
```

`CANONICAL_LEN = 1+8+4+1+32+32+32+16 = 126`.

`hash() = keccak(CORPUS_RECEIPT_LEAF_V1_DOMAIN || canonical_bytes)`.
Node: sorted-pair `keccak(CORPUS_RECEIPT_NODE_V1_DOMAIN || min || max)`, copy
of `listen_payout_node_hash_v1`.

`kind=RECEIPT` is fact_type `receipt_root`. `kind=MERCHANT_ATTACH` is
fact_type `merchant_attach`. One seq stream, mixed kinds in one tree.

`signing_pubkey` is hashed into the leaf so a later issuer rotation cannot
rewrite history under the old root. The program does not check it at publish
time (it never sees leaves). The adapter refuses a proof whose
`signing_pubkey` is not the live issuer **at the slot the root was
published**. Record `published_at_slot` for that reason.

## 6. H-01 is a side effect of this upgrade, not a standalone ship

Listen IXs stay in the binary (decision 2). Source already contains H-01
(#129). The step-2 upgrade therefore **deploys H-01** onto mainnet as part of
the same `solana-verify` build:

| Live pin `3128b644…` (pre-H-01) | After this upgrade |
|---|---|
| `paused` freezes claims | `paused` freezes **publish** only (listen still unbootstrapped, so dormant) |
| 1-step `set_payout_admin` | 2-step propose/accept (source) |
| no Config.admin emergency unpause | source has it; still inoperative until Config.admin rotation |

Do not ship H-01 alone. Do not advertise listen as live: config PDAs remain
absent until Config.admin rotation + bootstrap, which this design does not
do.

After a verifiable build, **one** wzrd-final change must: rotate
`HASH_RAILS_ARTIFACT`, add the new hash to `RAILS_H01_HASHES`, and only then
is `h01_publish_paused_claims_live` allowed to flip. That change is **not**
this repo and **not** this PR. Pin refresh is seam §5.1.

If `solana-verify build` matches source, rails evidence_level may move from
`artifact_hash` to `rebuild`. That is a seam pin edit in wzrd-final, same
change as the hash rotation. Do not claim `rebuild` from this design doc.

## 7. Errors

New `AttestationError` starting at `200` so it does not collide with
`RailsError` (0-19) or `ListenPayoutError` (100+).

| Code | When |
|---|---|
| `NotLiveV6Issuer` | init attester != `LIVE_V6_ISSUER` |
| `UnauthorizedAttester` | signer not on allow-list |
| `NotAdmin` | allow-list / propose signed by non-admin |
| `NoPendingAdmin` / `NotPendingAdmin` | accept path |
| `AdminPubkeyMustBeNonZero` | propose default |
| `AttesterListInvalid` | empty, default member, dups, len > 8, missing `LIVE_V6_ISSUER` |
| `UnknownAttestationKind` | kind != 1 |
| `HashMustBeNonZero` | any of the four hashes is zero |
| `SubjectMustBeNonZero` | |
| `SeqNotMonotonic` / `SeqOutOfRange` | |
| `ZeroLeafCount` / `LeafCountExceedsMax` / `ZeroMerkleRoot` / `SchemaVersionMismatch` | corpus publish |

Do not reuse `ListenPayoutError` variants. A shared pause error would imply a
pause that does not exist.

## 8. Implementation PR acceptance (next, not this)

Code PR (still no deploy):

1. State + IXs + errors as specified. No `Config` in new accounts structs.
2. `CorpusReceiptLeafV1` + node hash + golden vectors (at least one mixed-kind
   tree of 3 leaves, `MAX_PROOF_LEN` boundary, unsorted pair).
3. LiteSVM: init custody, init wrong key, double init, register happy / auth /
   zero hash / dup claim_id / bad kind, publish happy / non-monotonic / zero
   root, allow-list keeps `LIVE_V6_ISSUER`, 2-step admin, attester cannot
   update an attestation.
4. `cargo test -p wzrd-rails` green on the new tests. Do not require a full
   workspace build if AO v2 remains excluded.
5. Pointer in `GOAL.md` and parent §4 item 2: "design landed; implementation
   is PR N".

Deploy PR (later, operator go at action time):

- `solana-verify build` of wzrd-rails
- upgrade signed by `8di6hHF8…`
- init signed by `Ak5SQwHpuQA…`
- verification record append
- then the wzrd-final pin refresh

## 9. Facts to re-verify before implementing

| Fact | Last checked | Source |
|---|---|---|
| Live V6 issuer `Ak5SQwHpuQA…` / `twzrd-receipt-ed25519-v2` | 2026-09-18 `/health` | intel |
| Rails live hash `3128b644…`, pre-H-01 | parent §7 (2026-09-05 RPC) | ProgramData |
| Upgrade authority `8di6hHF8…` | parent §7 | ProgramData |
| `Config.admin` = `2pHjZL…`, 0 lamports | parent §7 | config PDA `7pwUU1hv…` |
| Listen config PDAs absent | parent §7 | RPC |
| wzrd-final step 1 (`settlement_tx`) on main | 2026-09-18 #2831 | origin/main |

If `/health.receipt_signing.trusted_pubkey` has moved when implementation
starts, **stop** and update `LIVE_V6_ISSUER` in this doc first. Shipping the
wrong first attester is a program upgrade to fix.
