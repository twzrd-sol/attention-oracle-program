# Attention Oracle — Open Core (Token‑2022 Program)

Brand‑neutral, production‑grade Anchor program implementing Token‑2022 claim
verification and a transfer‑hook entrypoint. No secrets or third‑party API keys
in this repository.

## Requirements
- Rust 1.76+ and Cargo
- Anchor CLI 0.30+
- Solana toolchain (for local work)

## Build
- Localnet/devnet: `anchor build`
- Program ID: `declare_id!` uses a placeholder; replace with your own when deploying.

## Contents
- `programs/token-2022/` — Anchor program and modules (claims, governance,
  ring‑buffer state, points). Module names and crate name are brand‑neutral.

## Security & Secrets
- No `.env` or provider URLs with query parameters are committed.
- `.gitignore` prevents common key formats and wallet JSON files from entering history.
- Do not hardcode RPC keys, JWTs, cookies, or API secrets in code or configs.

## License
MIT — see `LICENSE`.
