# Attention Oracle (Open Core)

Builder‑neutral implementation of a Solana program for verifiable claims using Merkle proofs and Token‑2022.

Repository scope is two core components:
- Solana program (Rust, Anchor) in `programs/`
- Minimal x402 + Switchboard example in `oracles/x402-switchboard/`

UI and deployment infrastructure are intentionally out of scope.

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

## Security & Secrets

- No private keys, .env files, or credentials are tracked. `.gitignore` blocks common secret patterns. Use environment variables and secret stores.
- Report any security concerns to the maintainers via private channels; do not open public issues for sensitive findings.

## License

Open‑core: core protocol and program are open; proprietary extensions live out of tree.
