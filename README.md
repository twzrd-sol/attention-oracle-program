# Attention Oracle (Open Core)

Builder‑neutral implementation of a Solana program for verifiable claims using Merkle proofs and Token‑2022.

Repository scope is two core components:
- Solana program (Rust, Anchor) in `programs/`
- Minimal x402 + Switchboard example in `oracles/x402-switchboard/`

UI and deployment infrastructure are intentionally out of scope.

## Environment

Safe defaults are provided via a tracked template at `.env.example` with placeholders only (no secrets). Create your local file once and keep it out of Git:

```bash
./scripts/bootstrap-env.sh     # copies .env.example -> .env if missing
# or: cp .env.example .env
```

Fill these values as needed:

- `ANCHOR_PROVIDER_URL` — Solana RPC URL (e.g. https://api.devnet.solana.com)
- `ANCHOR_WALLET` — path to your keypair (e.g. ~/.config/solana/id.json)
- `AO_PROGRAM_ID` — Attention Oracle program id (defaults to current ref)

Oracle example (optional):

- `PORT` — HTTP port for `oracles/x402-switchboard`
- `SB_CLUSTER` — devnet | mainnet-beta | testnet
- `SB_FEED` — Switchboard aggregator public key

Notes:

- `.env` files are ignored by default; `.env.example` files are tracked.
- `mocha` and the Switchboard oracle auto-load `.env` via `dotenv`.
- Anchor also respects `ANCHOR_PROVIDER_URL` and `ANCHOR_WALLET` when set.

## What It Does

- Verifiable distribution via Merkle roots committed per epoch/channel
- Token‑2022 mint support and transfer hook entrypoint
- Gas‑efficient claim bitmaps to prevent double claims

Program ID (current deployment reference): `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`

## Stack

- Language: Rust
- Program framework: Anchor ≥ 0.30
- Runtime: Solana mainnet, validator / CLI v2.3.x
- Token standard: Token‑2022 (transfer hooks)

## Build

```bash
cd programs/attention-oracle
anchor build
```

## Test

```bash
cd programs/attention-oracle
anchor test
```

## Directory Map (high level)

- `programs/attention-oracle/` — on‑chain program
- `clients/` — optional helpers and examples
- `packages/, rust-packages/` — shared libs (if present)

## Documents

- `OPEN_CORE_DOCUMENTATION_COMPLETE.md` — Open‑core scope and guidelines
- `PITCH_DECK.md` — Project overview deck
- `PROGRAMS_OVERVIEW.md` — Program ID mapping, repo scope, env/key policy

## Security & Secrets

- No private keys, .env files, or credentials are tracked. `.gitignore` blocks common secret patterns. Use environment variables and secret stores.
- Report any security concerns to the maintainers via private channels; do not open public issues for sensitive findings.

## License

Open‑core: core protocol and program are open; proprietary extensions live out of tree.
