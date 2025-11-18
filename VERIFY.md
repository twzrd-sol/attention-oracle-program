# Verifiable Build Guide

This document explains how to reproduce the Attention Oracle program binary from source code, enabling independent verification of the on-chain deployment.

## Program Information

- **Program ID**: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- **Source Repository**: https://github.com/twzrd-sol/attention-oracle-program
- **License**: Dual MIT/Apache-2.0

## Build Environment

### Required Toolchain

```bash
# Solana CLI (Anza/Agave 2.3.0+)
sh -c "$(curl -sSfL https://release.anza.xyz/v2.3.0/install)"
export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"

# Anchor CLI (0.32.1+)
cargo install --git https://github.com/coral-xyz/anchor --tag v0.32.1 anchor-cli --locked

# Rust toolchain (managed by rust-toolchain.toml)
rustup default stable
```

### Version Verification

```bash
anchor --version    # anchor-cli 0.32.1
solana --version    # solana-cli 2.3.0 (or higher, Agave)
rustc --version     # rustc 1.75.0+ (stable)
```

## Reproducible Build Steps

### 1. Clone Repository

```bash
git clone https://github.com/twzrd-sol/attention-oracle-program.git
cd attention-oracle-program
git checkout chore/anchor-0.32-upgrade  # Or specific commit/tag
```

### 2. Build the Program

```bash
cd programs
cargo build-sbf
```

Expected output location:
```
target/deploy/token_2022.so
```

### 3. Verify Binary Hash

```bash
sha256sum target/deploy/token_2022.so
```

**Expected SHA-256** (for commit `cf4686b`):
```
6dedc0ab78f3e3b8ea5533500b83c006a9542d893fb5547a0899bcbc4982593f
```

### 4. Compare with On-Chain Deployment

```bash
# Download deployed program
solana program dump GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop deployed.so --url mainnet-beta

# Compare hashes
sha256sum deployed.so
sha256sum target/deploy/token_2022.so
```

**Note**: If hashes differ, verify you're using the correct commit tag/branch corresponding to the on-chain deployment.

## CI/CD Verification

This repository includes a GitHub Actions workflow that automatically:

1. Builds the program on every commit
2. Computes and publishes the binary SHA-256
3. Extracts and uploads the Anchor IDL
4. Creates release artifacts with reproducible builds

See `.github/workflows/verify-build.yml` for details.

## Anchor Verifiable Build (Alternative)

Anchor 0.32+ supports verifiable builds with `--verifiable` flag:

```bash
anchor build --verifiable
```

This creates a Docker-based reproducible build that can be independently verified by:
- OtterSec Verify: https://verify.osec.io
- Ellipsis Labs: https://ellipsis.xyz/verify

## Understanding Version Changes

### Why Binary Hashes May Differ

If you're comparing against an older deployment:

1. **Toolchain Updates**: Anchor 0.30.1 → 0.32.1, Solana 1.18 → 2.3.0
2. **Dependency Changes**: `solana-program` removed, `sha3` added for Keccak256
3. **Compiler Optimizations**: Newer LLVM versions may produce different bytecode

### What Matters for Verification

Instead of byte-for-byte matching (which is brittle across toolchain versions), modern verification focuses on:

- **Source Code Transparency**: All code is open-source
- **Reproducible Builds**: Same source + same toolchain = same binary
- **IDL Verification**: Type signatures and instruction interfaces match
- **Security Audits**: Independent review by professional auditors

## Security Contact

Found a discrepancy or security issue?

- **Email**: security@twzrd.com
- **Policy**: https://github.com/twzrd-sol/attention-oracle-program/blob/main/SECURITY.md
- **Telegram**: @twzrd_xyz

## Deployment History

| Version | Commit | Deploy Date | Binary SHA-256 | Toolchain |
|---------|--------|-------------|----------------|-----------|
| v0.2.0 | `cf4686b` | 2025-11-18 | `6dedc0ab...593f` | Anchor 0.32.1, Solana 2.3.0 |
| v0.1.0 | `b38201a` | 2025-11-13 | `[legacy]` | Anchor 0.30.1, Solana 1.18 |

## IDL Extraction

The Anchor Interface Definition Language (IDL) describes all program instructions and accounts:

```bash
# Extract IDL from built program
anchor idl parse -f programs/src/lib.rs -o target/idl/token_2022.json

# Or from deployed program
anchor idl fetch GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop --provider.cluster mainnet
```

The IDL can be used to:
- Generate TypeScript/Rust clients
- Verify instruction signatures match documentation
- Build SDKs and integrations

## Frequently Asked Questions

### Q: Why doesn't the hash match the old deployment?

A: The program was upgraded from Anchor 0.30.1 to 0.32.1 and Solana 1.18 to 2.3.0. This modernizes the codebase and makes it Firedancer-ready, but changes the compiled binary.

### Q: How do I know the new binary is safe?

A: Three ways to verify:
1. **Reproduce the build** yourself using steps above
2. **Review the source code** - it's fully open-source
3. **Check security audits** - See SECURITY.md for audit reports

### Q: Can I verify against the old Solscan link?

A: The old verification link (commit `b38201a`) used Anchor 0.30.1. For the new deployment, use commit `cf4686b` with the toolchain specified in this document.

### Q: What's the upgrade path?

A: The program is upgradeable via program authority. The upgrade process:
1. Build new binary with `cargo build-sbf`
2. Submit to upgrade instruction signed by program authority
3. Update verification documentation with new commit hash

## Contributing

If you find issues with the build process or have suggestions for improving reproducibility, please open an issue or PR on GitHub.

---

**Last Updated**: 2025-11-18
**Verified By**: Community (see GitHub Actions builds)
