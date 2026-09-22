# Agent ledger

This is the continuation of Attention Oracle Program. New Solana work for agents
lands here. It does not land in `wzrd-final`, and it does not reopen Listen
settlement.

`GOAL.md` (2026-08-11) remains the record of the trust-rail seam. It is not the
brief for this program.

## What this is

An append-only ledger of signed evidence. A published root means one signer
attested one head before one slot. Another agent checks inclusion and decides
whether to cite that head or pay for a fresh one.

The ambition is a shared proof surface for agents on the web: any issuer, any
leaf domain, one program, inclusion proofs a stranger can verify. The program
does not become that by announcing it. It becomes that when a foreign wallet
pays because it read someone else's leaf.

A root is not a vouch, a score, or the truth of a page. The page stays off-chain.
The leaf names what was paid for.

## Lineage

| Program | Id | Status |
|---|---|---|
| Attention Oracle (`token_2022`) | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | Immutable on mainnet. Do not redeploy. |
| `wzrd-rails` | `BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9` | Upgradeable. Listen payout era. Not this ledger. |
| `wzrd-markets` | (see repo docs) | Attention markets. Not this ledger. |
| `evidence-ledger` | `BzBAYJxUtJp6mUkJPjEYjd8vdb2FUGnAfB5X9LqrQ72W` | Mainnet. This continuation. |

Mainnet: slot `444956973` (2026-09-07 02:56 UTC), deploy tx `2H3Jnx1uTKTNatYpJnUb2p9WGqLrPxGEi8DsZ8c2BpBtssc8kKvPzhrPCcroVH63ZRTsGwNyee7JdLeWHUP6EKfL`, upgrade authority `4HxZL3SAjBcXXk4VJMRs6JSP3T1cW95TkvJY1Ra5GD1B`, programdata `9e2Q1rDEmvqAFQ9Quh8tvzDAjW8xRxnYTspXvZCdD6CQ`. ELF sha256 `8d690a9ffa7aef50091789cc10c7b425b4375633cc5ed7031dcdd04515375f68` matches `cargo build-sbf` of commit `43ff827` (checked 2026-09-21). Devnet deploy tx `3b3NkCMp995aeui59ocQFMJ9GjAeS9ZZAfxGukeKXeYRcYgBb2yVu9dcMwY8sTFKyAWEaD1JfXmqXpM1zFc5PTX4` (2026-09-04) is the same program id, earlier.

## Where the pieces live

Program source now lives at `programs/evidence-ledger` in this repo (copied
2026-09-21: `src/`, `tests/`, `Cargo.toml`, `Cargo.lock`, `README.md`). No
`target/`, no keypair. It is excluded from this workspace so its own release
profile (`opt-level` 3) stays the one that builds. The mainnet ELF matches
commit `43ff827`. Do not edit the wzrd-final copy in parallel.

| Piece | Home | Why |
|---|---|---|
| Program source, instructions, LiteSVM tests | `programs/evidence-ledger` here | Public Solana surface. One program id. |
| HTTP signed tree head, `log_anchor_publisher`, hourly anchor timer | `wzrd-final` | They read the intel log and the prod DB. |
| Preflight, receipts, wash | `wzrd-final` | Decision and evidence production. Not the chain. |
| Listen settlement, `SETTLEMENT_JOBS_ENABLED`, AO oracle key | neither | Do not flip that fleet to feed this ledger. |

This repo is the source of truth for the program. The mainnet binary matches
`43ff827`. Do not keep a second edited copy in wzrd-final.

## Instructions that exist

`init_ledger`, `set_authority`, `publish_root` (scheme 1), `anchor_head`
(scheme 2, prior ed25519 of the trusted signer), `verify_inclusion` (no signer,
CPI-callable). No claim, mint, transfer, or scoring. `claimable` stays false.
A signer change is a new `log_id`, not a rewritten head.

## Leaf this ledger is for

Payer, resource hash, USDC amount, slot. Agents coordinate by reading leaves.
An agent acts on its own when it holds a Solana USDC key and a ceiling. The
program records the payment. It does not fetch the page and it does not hold
the goal.

Settlement is USDC. A token, if one is ever added, is capacity only. Leaves are
signed evidence, not raw transaction counts.

## Stripe / SPT leaf domain (no-permission spike)

Separate leaf domain on the same program. Proof construction is local and
fail-open. It does not call Stripe, Link, Muse, Shared Payment Tokens, trust
refuse, or any RPC. Spend and settlement stay out of this path.

| Field | Binding |
|---|---|
| `log_id` | `stripe.agentic.spt.v1` |
| Domain tag | `TWZRD:STRIPE_SPT_LEAF_V1` |
| Payer | `keccak256("stripe:customer:" \|\| customer_ref_utf8)` |
| Resource hash | `keccak256("stripe:pi:" \|\| payment_intent_id \|\| 0x00 \|\| resource_id_utf8)` |
| Amount | `u64` little-endian, USD cents (currency fixed by the domain tag) |
| Slot | `u64` little-endian stand-in: Stripe `created` unix seconds until an on-chain slot is available |

Leaf digest:

`leaf = keccak256(domain \|\| payer \|\| resource_hash \|\| amount_le \|\| slot_le)`

Same on-chain shape as the USDC leaf: four fields, one 32-byte digest, inclusion
against a published root. Fixture and LiteSVM coverage live under
`programs/evidence-ledger`. The dry-run publisher is on wzrd-final worktree
branch `grok/stripe-spt-leaf-dry-run`
(`packages/twzrd-agent-intel/src/twzrd_agent_intel/stripe_spt_leaf_dry_run.py`).
It never waits on an external refuse.

## Reader paid-fetch leaf (`reader.fetch.v1`)

Not an SPT. Not `intel.twzrd.xyz/v6`. Not initialized. The encoder is
host/test only (`leaf_fetch.rs`, `cfg` test/localtest) so the mainnet binary
stays the deployed ELF.

| Field | Binding |
|---|---|
| `log_id` | `reader.fetch.v1` |
| Domain tag | `TWZRD:READER_FETCH_LEAF_V1` |
| Payer | 32-byte Solana pubkey, or 12 zero bytes plus a 20-byte Base address |
| Resource hash | `sha256` of the delivered markdown. `sha256` of the URL is the unpaid request id only. |
| Amount | `u64` little-endian USDC base units. Live prices are 5000 and 50000. |
| Time | one `u8` clock plus one `u64`. Clock `1` is `slot` (Solana settlement slot). Clock `2` is `block_number` (Base `eth_getTransactionReceipt.blockNumber`). A Base payment has no Solana slot. |

`leaf = keccak256(domain || payer || resource_hash || amount_le || clock || time_le)`

Golden fixture: `programs/evidence-ledger/tests/fixtures/reader_fetch_v1.json`.
No `init_ledger`. The hourly publisher stays on `intel.twzrd.xyz/v6`.

## Cut

Done 2026-09-21. Program source is in-tree at `programs/evidence-ledger` (commit `43ff827`).

1. Source is in `programs/evidence-ledger`. No `target/`, no keypair.
2. Excluded from the workspace. Not wired to `GnGz…` or `BdSv…`.
3. `cargo test --release` from that crate is the check that the copy builds.
4. The same program id is live on mainnet. The wzrd-final publisher was documented against devnet; this note does not retarget it.
5. Further commits only when asked. The 2026-09-21 cut did not deploy. Mainnet was already live.

`security.txt` inside the program still names the wzrd-final source URL. That
string is a fixed 367-byte section. Changing it changes the binary, so it stays
until a named source-string bump.
