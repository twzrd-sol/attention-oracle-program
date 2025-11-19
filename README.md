# Attention Oracle (Open Core)

Builder-neutral Solana Token-2022 program plus a single oracle demo. Every other service (listener, aggregator, UI, CLI, SDK) now lives in private repos while we rebuild from first principles.

## Repo Scope (Open-Core)

This repository contains only the on-chain protocol and minimal reference oracle:

- `programs/token_2022` – mainnet program GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop (Anchor 0.32.1 + Agave 3.0)
- `oracles/x402-switchboard` – reference implementation

All off-chain services (listener, aggregator, UI, gateway) are private and live in separate repos.

## Secrets & Keys Policy

- No private keys or .env values are ever committed.
- Keys live in `~/.config/solana/` or local `keys/` (gitignored).
- Reference via env only:
  ```env
  ANCHOR_WALLET=~/.config/solana/id.json
  AO_PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
  RPC_URL=https://api.mainnet-beta.solana.com
  ```
- CI uses GitHub Secrets for deployment keys – never plaintext.

Fork/modify safely. This is the verifiable source of truth for the deployed program.

## Scope

- `programs/token_2022/` — Anchor 0.32.1 program deployed as `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`.
- `oracles/x402-switchboard/` — Minimal x402 + Switchboard server that demonstrates off-chain settlement.

All other components are intentionally absent from the public tree until they are production-ready again.

## Environment

Copy the template and fill the values you need (never commit `.env`).

```bash
cp .env.example .env
```

Key fields:

- `ANCHOR_PROVIDER_URL` — RPC for Anchor builds/tests.
- `ANCHOR_WALLET` — Path to your deploy keypair.
- `AO_PROGRAM_ID` — Defaults to the public deployment.
- `SB_CLUSTER`, `SB_FEED`, `PORT` — Inputs for the oracle demo.

`dotenv` is loaded automatically where required.

## Build

```bash
cd programs/token_2022
cargo build-sbf
anchor build
```

## Build Notes (Rust 1.89 vs current SBF toolchain)

The on-chain crate pins `rust-version = "1.89"`. Today’s Solana SBF toolchain (CLI 3.0.0) still ships `rustc 1.84.1`, so `anchor build/test` fails unless you either:

1. Install a Solana toolchain that bundles rustc 1.89+ (once available, preferred), or
2. Temporarily relax the crate’s `rust-version` to `1.84` for local testing, then restore it before tagging releases.

We keep the source on 1.89 and document the mismatch instead of downgrading the program.

## Test

```bash
cd programs/token_2022
anchor test   # requires rustc 1.89 per Build Notes
```

## Oracle Demo (x402 + Switchboard)

```bash
cd oracles/x402-switchboard
npm install
cp ../../.env.example .env   # populate SB_CLUSTER/SB_FEED/PORT
npm run dev
curl http://localhost:3000/price
```

The demo is stateless and provided strictly for reference.

## Security

- Keep private keys outside the repo (e.g. `~/.config/solana/id.json`, Vault).
- `.env` files and keypairs stay gitignored.
- Report vulnerabilities to `security@twzrd.xyz`.

## License

Dual MIT / Apache-2.0 (see `LICENSE` / `LICENSE-APACHE`).
