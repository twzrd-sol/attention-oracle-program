# Attention Oracle (Open Core)

Builder‑neutral, first‑principles implementation of a modern Solana program for verifiable claims using Merkle proofs and Token‑2022. No IP, no secrets, no hype.

— Keep: open‑core docs
— Remove: internal checklists, ops runbooks, incident logs, and environment files (scrubbed from history)

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
- Framework: Anchor ≥ 0.30
- Cluster toolchain: Solana ≥ 1.18

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

## Documents (kept)

- `OPEN_CORE_DOCUMENTATION_COMPLETE.md` — Open‑core scope and guidelines
- `OPEN_CORE_EXCELLENCE_PLAN.md` — Quality bars and contribution expectations

Pitch materials are maintained off‑repo and shared on request. All other internal docs were removed and scrubbed from git history.

## Security & Secrets

- No private keys, .env files, or credentials are tracked. `.gitignore` blocks them; history has been rewritten to purge past leaks.
- Report any security concerns to the maintainers via private channels; do not open public issues for sensitive findings.

## License

Open‑core: core protocol and program are open; proprietary extensions live out of tree.
