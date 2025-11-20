# Attention Oracle — Open Core

Builder-neutral Token-2022 protocol on Solana. Minimal, verifiable on-chain reference implementation. All advanced off-chain components (aggregators, listeners, interfaces, toolkits) are developed privately and released only when production-ready.

Mainnet program ID (verified): `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`  
Built with Anchor 0.32.1 + Agave 3.0.10

## Repository Scope

This public repository contains only the verifiable on-chain surface:

- `programs/token_2022/` – the deployed Token-2022 extension program
- `attention_oracle_program/` – canonical Anchor workspace that orchestrates builds
- `oracles/x402-switchboard/` – minimal reference oracle (x402 + Switchboard) for demonstration only

Everything else lives in private repositories until mature. The public tree is intentionally minimal and permanently verifiable. No secret sauce, no moving parts, no hidden dependencies.

| Folder | Description | Last Commit | When |
| --- | --- | --- | --- |
| `attention_oracle_program` | Anchor workspace (helper scripts, CI manifests) | CI fixes: artifact paths, guard scope, IDL extraction, workspace config | 19 hours ago |
| `programs/token_2022` | Token-2022 program deployed to mainnet | (see git history) | (see git history) |

## Verification Status

Deterministic, verifiable build – green check on Solscan.  
Reproduce exactly with:

```bash
anchor build --verifiable
solana-verify build -k ~/.config/solana/id.json --library-name token_2022
```

See `VERIFY.md` for the full pipeline and GitHub Actions workflow.

## Secrets & Keys Policy

- No keys or `.env` files are ever committed.  
- All keys live in `~/.config/solana/` or local `keys/` (gitignored).  
- CI uses encrypted GitHub Secrets only.

Fork and build safely — this repo is the single source of truth for the deployed bytecode.

## Environment

Copy the template and fill the values you need (never commit this file):

```bash
cp .env.example .env
```

Required variables:

- `ANCHOR_PROVIDER_URL` — your RPC
- `ANCHOR_WALLET` — path to deploy keypair
- `AO_PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- Switchboard demo: `SB_CLUSTER`, `SB_QUEUE`, `SB_FEED`, `PORT`

## Build

```bash
cd programs/token_2022
anchor build --verifiable
```

## Toolchain notes

- Workspace Rust: `1.91.1` (dev workflow)  
- SBF target: `rustc 1.84.1-sbpf` (via Solana CLI `3.0.10`)  
- `rust-version` in crate = `1.84` to keep Anchor happy

## Test

```bash
cd programs/token_2022
anchor test
```

## Reference Oracle Demo (x402 + Switchboard)

```bash
cd oracles/x402-switchboard
npm install
npm run dev
curl http://localhost:3000/price
```

Stateless reference implementation only. Production oracles run privately.

## Canonical Production Flow

1. Private aggregators ingest off-chain events
2. Publish channel roots via `set_channel_merkle_root` (ring buffer)
3. Users claim via `claim_channel_open` / `claim_channel_open_with_receipt`
4. Long-lived reputation via passport instructions (`mint_passport_open`, `upgrade_passport_open`, …)
5. `transfer_hook` allocates dynamic fees by passport tier
6. Off-chain keepers harvest fees

Legacy epoch-based instructions are gated behind the `legacy` feature and used only for migrations.

## Security

Report vulnerabilities → security@twzrd.xyz

## License

MIT OR Apache-2.0
