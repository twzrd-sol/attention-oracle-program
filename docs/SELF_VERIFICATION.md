# Self-Verification for Solana Programs (Verified Builds)

This guide shows how to verify that the on-chain program binary exactly matches this repository’s source code, without any third party. It uses Ellipsis Labs’ Solana Verified Builds tooling to reproduce and compare binaries deterministically.

Status (as of November 14, 2025)
- Program: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- Local vs on-chain: SHA256 match confirmed via `solana program dump` + `sha256sum`.
- security.txt: Embedded and visible in the deployed binary.

Why self-verification works
- Deterministic builds: fixed toolchain (Rust/Solana) → reproducible `.so`.
- The Ellipsis Labs verifier runs on Solana 1.18.26 (rustc 1.75); we pin `toml_datetime` to `0.6.5` in the verification snapshot so the Docker builder can compile it. Regular local builds can continue to target Rust 1.76+.
- No third party required: builds/compare happen locally; optional remote attestation is just for caching/public badge.
- Transparency: logs and hashes can be published with releases.

Quick local check (already used)
```bash
# Dump on-chain program and compare with local build
solana program dump GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop /tmp/onchain.so -u mainnet-beta
sha256sum /tmp/onchain.so clean-hackathon/target/deploy/token_2022.so
# Hashes must match exactly
```

Install the Verified Builds CLI (one-time)
```bash
cargo install --git https://github.com/Ellipsis-Labs/solana-verifiable-build solana-verify --locked
solana-verify --version
```

Prepare repo for deterministic builds
- Ensure `Cargo.lock` is committed.
- Pin versions in `Cargo.toml` (e.g., `anchor-lang = "0.30.1"`, `solana-program = "1.18.26"`).
- Build locally with `cargo build-sbf` to confirm success before verifying.
- Tag the exact commit that produced the deployed binary (example):
```bash
git tag v1.0.0-upgraded
git push origin v1.0.0-upgraded
```

Run Verified Build (from this repo root)
```bash
# Rebuild deterministically and compare to on-chain
solana-verify verify-from-repo \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --url https://api.mainnet-beta.solana.com \
  https://github.com/twzrd-sol/attention-oracle-program.git \
  --tag v1.0.0-upgraded
```

Success criteria
- CLI prints that verification passed; explorers may show a “Verified” badge shortly after.

Optional: public attestation (for explorer badge caching)
```bash
solana-verify remote submit-job \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --uploader <YOUR_PUBKEY>
```

Common pitfalls
- Repo drift: make sure the tag matches the deployed `.so` commit.
- Toolchain drift: pin Solana and Anchor versions; consider `solana-install init 1.18.26`.
- Missing lockfile: commit `Cargo.lock`.

Automation
- See `scripts/verify-onchain.sh` for a thin wrapper that runs local dump, hash compare, and (optionally) verified builds.
