# Verifiable Distribution Protocol (Token‑2022)

On‑chain Merkle claim verification and Token‑2022 integration (Anchor).

A general‑purpose, production‑grade Anchor program for settling off‑chain,
verifiable events on‑chain. The protocol provides a secure and gas‑efficient
mechanism for token distribution via Merkle proofs, with first‑class support
for the Token‑2022 standard.

This repository contains the brand‑neutral, open‑core on‑chain program. No
secrets or third‑party API keys are included.

## Requirements
- Rust 1.76+ and Cargo
- Anchor CLI 0.30+
- Solana toolchain (for local work)

## Deployment

**Mainnet v1:** `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`

This is the production deployment on Solana mainnet. The program is live and verifiable on-chain:

- **Verification Method:** otter-verify (solana-verify CLI)
- **Verification Commit:** `e17a156490e7847645830b2de4fde7f326eecf1c`
- **Deployed Slot:** 376962961
- **Verification PDA:** `5335zbC2hFqEG8RvXuMzaMDPCsuw1w3Wvdq2KwnCYdoo`

To verify the build yourself:
```bash
solana-verify get-program-pda --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --url https://api.mainnet-beta.solana.com
```

## Build
- Localnet/devnet: `anchor build`
- For mainnet deployment: Use the program ID above or deploy your own instance.

## Contents
- `programs/token-2022/` — The complete, hardened Anchor program, including
  modules for claims, governance, and the ring‑buffer state. Module names and
  crate name are brand‑neutral.

## Security & Secrets
- No `.env` or provider URLs with query parameters are committed.
- `.gitignore` prevents common key formats and wallet JSON files from entering history.
- Do not hardcode RPC keys, JWTs, cookies, or API secrets in code or configs.

## License
MIT — see `LICENSE`.
