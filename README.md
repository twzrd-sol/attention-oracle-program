# Attention Oracle (Open Core)

Builder-neutral implementation of a Solana Token‑2022 program plus the helpers that surround it.

## Environment

Use the tracked `.env.example` to create a local `.env` (never commit `.env`).
```bash
./scripts/bootstrap-env.sh  # copies .env.example -> .env if missing
# or: cp .env.example .env
```
Fill these values as needed:

- `ANCHOR_PROVIDER_URL` — Anchor RPC URL (e.g. `https://api.devnet.solana.com`).
- `AO_RPC_URL` — Optional override for CLI/SDK RPC (falls back to Anchor URL).
- `ANCHOR_WALLET` — Path to your deploy keypair (e.g. `~/.config/solana/id.json`).
- `AO_PROGRAM_ID` — Defaults to the public program `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` (v0.2.1-clean).
- `SB_CLUSTER`, `SB_FEED`, `PORT` — For the optional `oracles/x402-switchboard` reference implementation.

`dotenv` is loaded automatically in tests and the oracle demo.

## Architecture

```
programs/token_2022/     # Active Solana Token-2022 program (GnGz...)
sdk/                     # TypeScript + Rust clients that share the same IDL
cli/                     # Admin CLI wired to AO_PROGRAM_ID and AO_RPC_URL
oracles/x402-switchboard/ # Minimal Switchboard + x402 demo (env-driven)
docs/                    # Public docs (open-core scope + pitch deck outline)
```

## Build

```bash
cd programs/token_2022
cargo build-sbf
anchor build
```

## Test

```bash
cd programs/token_2022
anchor test
```

## Documentation

- `docs/OPEN_CORE_DOCS.md` — Open-core scope, contribution notes, and governance guidance.
- `docs/PITCH_DECK.md` — Public-safe pitch deck outline without proprietary links or claims.

## Security & Secrets

- Private keys stay local (e.g. `~/.config/solana/id.json`, HashiCorp Vault, or a secure key manager). Paths alone are referenced in code.
- `.env` files and `.json` keypairs are ignored by Git.
- Report vulnerabilities via `security@twzrd.xyz`; see `docs/OPEN_CORE_DOCS.md` for the disclosure policy.

## License

Dual MIT / Apache-2.0 (see `LICENSE` / `LICENSE-APACHE`).
