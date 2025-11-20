# Attention Oracle (Open Core)

Builder-neutral Solana Token-2022 program plus a single oracle demo. Every other service (listener, aggregator, UI, CLI, SDK) now lives in private repos while we rebuild from first principles.

## Repo Scope (Open-Core)

- `programs/token_2022` – mainnet program GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop (Anchor 0.32.1 + Agave 3.0.10)
- `programs/attention_oracle_program` – lightweight Merkle verifier/payout flow designed to mirror the same Program ID for clean upgrades and verification.
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

## Build Notes (Toolchains)

- Workspace Rust: `1.91.1` (fmt/lints, general dev via rustup).
- Agave SBF builder (Solana CLI `3.0.10`): uses `rustc 1.84.1-sbpf` internally.
- Program crate `rust-version`: set to `"1.84"` to match the SBF toolchain so `anchor build` stays green.

This split is expected: developers use modern stable Rust for workflow, while the SBF compiler version is managed by Agave. When the platform tools bump SBF Rust, we can raise the crate `rust-version` accordingly.

## Test

```bash
cd programs/token_2022
anchor test
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

## Canonical Architecture

The production flow is designed around the ring buffer and passports. Legacy epoch-state instructions are compiled out by default.

1. Ingest off-chain events in a private aggregator.
2. Publish channel roots via `set_channel_merkle_root` (per-channel ring buffer) or `set_merkle_root_ring` when built with the `demo` feature.
3. Users claim CCM via `claim_channel_open` (and `claim_channel_open_with_receipt` when cNFT receipts are desired).
4. Users accumulate long-lived reputation via the passport instructions (`mint_passport_open`, `upgrade_passport_open`, etc.).
5. Transfer fees are dynamically allocated by the `transfer_hook` based on passport tier, and harvested later via off-chain keepers listening for `TransferFeeEvent` / `FeesHarvested`.

Legacy epoch-state instructions (`claim`, `claim_open`, `set_merkle_root`, `set_merkle_root_open`, `claim_points_open`, and epoch-close helpers) are only compiled when the `legacy` feature is enabled and are intended for migrations and historical cleanup.

## Attention Oracle Verifier (Lite)

The `attention_oracle_program` in `programs/attention_oracle_program` mirrors the EpochRoot + Merkle verification logic with a compact claimed bitmap and CPI payout hook. It is kept aligned to Program `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` so the repo can reproduce the mainnet hash while also shipping a focused verifier for rapid integration tests.
