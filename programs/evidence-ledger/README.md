# evidence-ledger (Pinocchio)

Append-only on-chain registry of merkle roots over TWZRD's signed evidence
leaves. A published entry means one thing: *TWZRD's log key signed this head,
and it was anchored before slot S.* It is not a vouch, an eligibility flag, or
an unlock. The program has no claim, mint, transfer, or scoring instruction and
no per-seller account, by construction. See `LEDGER.md` in this repo for lineage
and cut boundaries.

Program id: `BzBAYJxUtJp6mUkJPjEYjd8vdb2FUGnAfB5X9LqrQ72W` (keypair in `target/deploy`, not committed).

## Accounts

| Account | PDA | Size | Notes |
|---|---|---|---|
| `Ledger` | `["ledger", keccak256(log_id)]` | 128 | one per log / leaf domain; `scheme`, `authority`, immutable `trusted_signer`, strictly-increasing cursor |
| `RootEntry` | `["root", ledger, seq_le]` | 152 | one per anchored head; never rewritten (a correction is a later seq) |

Schemes: `1` = sorted-pair keccak batch root, `2` = RFC 9162 keccak log.
The off-chain merkle helpers and RFC 9162 log live in the sibling `wzrd-final`
tree (ingestion), not in this repo. Layouts and discriminators
(`sha256("account:<Name>")[..8]`) are in `src/state.rs`.

## Instructions (`sha256("global:<name>")[..8]` discriminators)

- `init_ledger(authority, trusted_signer, scheme, log_id)` — the chain hashes `log_id` into the seed.
- `set_authority(new)` — who may spend to publish. The trusted signer has no rotation path on
  purpose: the log spec says a key change means a new `log_id`, never a re-signed head.
- `publish_root(seq, root, manifest_hash, leaf_count)` — scheme 1 only.
- `anchor_head(tree_size, timestamp, root, log_id)` — scheme 2. Requires the immediately
  preceding instruction to be one ed25519 precompile call whose (pubkey, message) is exactly
  (`trusted_signer`, the STH preimage rebuilt on-chain). The program never signs, so the
  anchoring key cannot manufacture a head the log key never issued.
- `verify_inclusion(seq, leaf, proof)` — no signer, no writes; CPI-callable. Scheme 2 takes
  `(index, path)` per RFC 9162.

Errors: `6000` Unauthorized, `6001` AlreadyInitialized, `6002` SequenceMismatch, `6003` InvalidProof,
`6004` InvalidPubkey, `6005` WrongScheme, `6006` MissingSignatureCheck, `6007` SignatureBindingMismatch.

## Build and test

```bash
cd programs/evidence-ledger
cargo test --release                                   # unit: keccak, layouts, both merkle schemes (Python reference vectors)
cargo build-sbf                                         # target/deploy/evidence_ledger.so
cargo test --release --features localtest --test litesvm_ledger   # LiteSVM end to end, incl. the signed head
```

Golden leaves pinned on-chain in the LiteSVM suite: the signed V6 receipt example, the V6
full/none reputation-block vectors, and the three decision-outcome attestation vectors.

## Off-chain half

Ingestion and publishing stay in the sibling `wzrd-final` tree: the HTTP signed
tree head, the Anchor publisher that builds `[ed25519, anchor_head]` transactions,
and the hourly anchor timer. This repo holds the on-chain program only. See
`LEDGER.md` for the boundary. Mainnet is already deployed; a further upgrade is an operator decision.

## Deployments

| Cluster | Program | Deploy tx | Notes |
|---|---|---|---|
| devnet | `BzBAYJxUtJp6mUkJPjEYjd8vdb2FUGnAfB5X9LqrQ72W` | `3b3NkCMp995aeui59ocQFMJ9GjAeS9ZZAfxGukeKXeYRcYgBb2yVu9dcMwY8sTFKyAWEaD1JfXmqXpM1zFc5PTX4` | 2026-09-04; upgrade authority is a throwaway devnet key |
| mainnet | `BzBAYJxUtJp6mUkJPjEYjd8vdb2FUGnAfB5X9LqrQ72W` | `2H3Jnx1uTKTNatYpJnUb2p9WGqLrPxGEi8DsZ8c2BpBtssc8kKvPzhrPCcroVH63ZRTsGwNyee7JdLeWHUP6EKfL` | 2026-09-07 02:56 UTC, slot `444956973`. Upgrade authority `4HxZL3SAjBcXXk4VJMRs6JSP3T1cW95TkvJY1Ra5GD1B`. Programdata `9e2Q1rDEmvqAFQ9Quh8tvzDAjW8xRxnYTspXvZCdD6CQ`. ELF sha256 `8d690a9ffa7aef50091789cc10c7b425b4375633cc5ed7031dcdd04515375f68` matches `cargo build-sbf` of commit `43ff827` (2026-09-21). |

First anchored head on devnet (the live `GET /v1/log/sth` head, signed by `Ak5SQwHpuQAqU7ty7ZWX7qgF39A9yi72c22KNn8sHzvS`):

- ledger `8EpPmDkQQ9uqMQH166kx7nu4wT415bobWcWfyiZDvgv3` — `log_id intel.twzrd.xyz/v6`, scheme 2, trusted signer = the v2 key
- `anchor_head` tx `5tgSkxJmMpgPLnGJxXAV3eyPvE75J5nVNJdeABLTCnNhzHzrPq9L3n3DRC1dYDoC1BV7tQCEjJEUNyc3pxbvymB4`, slot 492922104
- root entry `yAKKnCafUxus8hT81wXKcs3PZLpyncn1KNEwY2r9Yce` — tree_size 1, root `0x811e1fee65f06c5cfcfee8f338e933c1d3dd261c4c09b8f2793b62bea7ea6db4`, signed timestamp 1788450541

Independently re-read through the public devnet RPC: the transaction invoked `Ed25519SigVerify…` then this program; both PDAs are program-owned. This is what a `log_anchors` row (migration 0212) records.

## Running the Commit level

The anchor CLI and systemd timer live in `wzrd-final`, not here. From that tree,
`python -m twzrd_agent_intel.anchor_log_head` reads the current signed tree head,
reads the on-chain ledger, anchors only when `tree_size` grew, and records the
`log_anchors` row. The hourly Battleship unit is optional and operator-owned.

Cost on mainnet at rent-exempt minimums: 0.001773 SOL per anchored head (permanent, 152-byte account),
0.001621 SOL once per ledger, 0.288 SOL once for the program. Because a run is idempotent, spend tracks
the number of distinct heads, not wall-clock: an hourly timer anchors at most once per hour and only when
the log grew.
