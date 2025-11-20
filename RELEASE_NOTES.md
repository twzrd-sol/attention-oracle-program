# Release Prep: v19.11.2025 Core Solana Stack

## Stack Highlights
- **Rust 1.91.1 (Clippy Zero)**: fixes the Wasm linker regression and lets us enforce `clippy::all + clippy::pedantic + clippy::nursery` across the workspace without hacks.
- **Anchor 0.32.1 (Skew Eliminated)**: guarantees the IDL upload wait finishes so CI/CD sees a deterministic deploy.
- **Agave 3.0.10 (Loader v4-watch)**: nodes run the high-performance runtime, but program upgrades still use the legacy loader until the CLI bug in 3.0.11+ ships.

## Release Checklist
1. Merge `release/main-upgrade` once the guard + verify jobs pass.
2. Run `anchor build --verifiable`; hash → `761f3e7358be3e3faf4b1a1ffc044a5b997a44fa3e735e7ae06d6ece13440ebf`.
3. Write-buffer + upgrade the program ID `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` on mainnet.
4. Verify via `anchor verify token_2022 --current-dir` (match repo hash).
5. Tag release `v0.2.2-agave` and publish release notes referencing the stack + hash.
