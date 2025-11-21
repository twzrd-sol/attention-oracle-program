# Attention Oracle — Verifiable Token‑2022 Program

This repository contains the minimal, verifiable on‑chain program that is deployed to Solana mainnet. All off‑chain components and any non‑critical code live in private repos. The public tree is kept intentionally small to guarantee reproducibility and trustless verification.

- Program ID: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- Program: `programs/token_2022/`
- Toolchain: Anchor 0.32.1 + Agave 3.0.10 + Rust 1.91.1

## What’s Included (and why)

- Only `programs/token_2022/` is published. This is the exact code used to produce the deployed binary. Keeping the public tree to this program ensures anyone can rebuild the same bytes and compare them to what’s on‑chain.

## One‑Command Verification

Use Anchor’s native verifiable pipeline (Dockerized, deterministic):

```bash
# From repo root
anchor verify -p token_2022 GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
```

This builds in the pinned container and compares the trimmed executable section to mainnet. Our CI runs the same check on release tags.

## Expected Verifiable Build (v1.2.1)

- Size: `534,224` bytes
- SHA256: `8e60919edb1792fa496c20c871c10f9295334bf2b3762d482fd09078c67a0281`

If your local build or environment differs, re‑run in Docker via `anchor build --verifiable` or use the CI workflow on the `v1.2.1` tag.

## CI: What We Publish

Our GitHub Actions workflow (Verify Program) does the following on tags:
- Runs `anchor verify` against mainnet for the Program ID above
- Builds a verifiable artifact and uploads:
  - `local` verifiable `.so`
  - `on-chain.so` dump
  - A summary with local size/hash, on‑chain trimmed hash, and tool versions

## Security

Report vulnerabilities → security@twzrd.xyz

## License

MIT OR Apache‑2.0
