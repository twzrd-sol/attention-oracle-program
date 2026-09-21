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
| `evidence-ledger` | `BzBAYJxUtJp6mUkJPjEYjd8vdb2FUGnAfB5X9LqrQ72W` | Devnet only. This continuation. |

Devnet deploy tx: `3b3NkCMp995aeui59ocQFMJ9GjAeS9ZZAfxGukeKXeYRcYgBb2yVu9dcMwY8sTFKyAWEaD1JfXmqXpM1zFc5PTX4` (2026-09-04). Mainnet is an operator decision.

## Where the pieces live

Program source now lives at `programs/evidence-ledger` in this repo (copied
2026-09-21: `src/`, `tests/`, `Cargo.toml`, `Cargo.lock`, `README.md`). No
`target/`, no keypair. It is excluded from this workspace so its own release
profile (`opt-level` 3) stays the one that builds. The wzrd-final tree is the
pre-cut original and the tree the devnet binary was built from. Do not edit
both.

| Piece | Home | Why |
|---|---|---|
| Program source, instructions, LiteSVM tests | `programs/evidence-ledger` here | Public Solana surface. One program id. |
| HTTP signed tree head, `log_anchor_publisher`, hourly anchor timer | `wzrd-final` | They read the intel log and the prod DB. |
| Preflight, receipts, wash | `wzrd-final` | Decision and evidence production. Not the chain. |
| Listen settlement, `SETTLEMENT_JOBS_ENABLED`, AO oracle key | neither | Do not flip that fleet to feed this ledger. |

Until the cut, the wzrd-final tree is the source that was deployed to devnet.
Do not keep two edited copies.

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

## Cut

Done 2026-09-21 as a working-tree copy. Not committed.

1. Source is in `programs/evidence-ledger`. No `target/`, no keypair.
2. Excluded from the workspace. Not wired to `GnGz…` or `BdSv…`.
3. `cargo test --release` from that crate is the check that the copy builds.
4. The wzrd-final publisher stays on devnet program id `BzBAYJxUtJp6mUkJPjEYjd8vdb2FUGnAfB5X9LqrQ72W`.
5. Commit only when asked. No mainnet deploy in the cut.

`security.txt` inside the program still names the wzrd-final source URL. That
string is a fixed 367-byte section. Changing it changes the binary, so it stays
until a named source-string bump.
