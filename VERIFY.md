# Verification Instructions

This program is deterministically built and verified on-chain. No third parties required.

## Quick Verification

**Program ID:** `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
**Solscan:** https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

## Self-Verification Steps

Anyone can reproduce our build and verify it matches on-chain:

### 1. Clone This Repository
```bash
git clone https://github.com/twzrd-sol/attention-oracle-program.git
cd attention-oracle-program
git checkout v1.2.0  # Use the specific release tag (latest: v1.2.0)
```

### 2. Install Dependencies
```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Solana/Agave CLI (must match our version)
sh -c "$(curl -sSfL https://release.anza.xyz/v3.0.10/install)"

# Install Anchor (must match our version)
avm install 0.32.1
avm use 0.32.1
```

### 3. Build Deterministically
```bash
# This produces a verifiable build
anchor build --verifiable --arch sbf
```

### 4. Compare Hash
```bash
# Get your local build hash
sha256sum target/deploy/token_2022.so

# Get on-chain program and trim to actual program size
LOCAL_SIZE=$(stat -c%s target/deploy/token_2022.so)
solana program dump GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop on-chain.so --url mainnet-beta
head -c "$LOCAL_SIZE" on-chain.so | sha256sum

# v1.2.0 Expected hash (trimmed): 357047e93929b6ad8f6879575b0633d2ae97d7ec78475a48c73000d6156b8a27
```

**Note:** On-chain accounts are padded with zeros to their account size. We trim to the actual program size (510144 bytes) before comparing hashes. Solscan and other verification tools handle this trimming automatically.

### 5. Verify on Solscan
Navigate to: https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

Look for the "Verified" badge and matching source code link.

## Verified Hashes by Version

| Version | Hash (Trimmed) | Features |
|---------|---------------|----------|
| **v1.2.0** | `357047e93929b6ad8f6879575b0633d2ae97d7ec78475a48c73000d6156b8a27` | AMM-compatible, Audit Mode |
| v1.1.1 | `e6bda5c18d1ac7efbec7f7761d48f326ea73fcbe3753873c4de3c5f19a017322` | Production hooks |
| v1.1.0 | `647b8bd464d3837f03f0e68b4823cfc719e1e4793c3c72c7d5cdc09bbf816cb2` | Initial verified release |

**Current on-chain:** v1.2.0 (deployed Nov 21, 2025)

## Understanding On-Chain vs Local Hashes

When comparing hashes, it's important to understand how Solana stores programs:

- **Local build** (`target/deploy/token_2022.so`): 510144 bytes - the actual compiled program
- **On-chain account**: 830936 bytes - includes padding (zero-filled slack space)

When you dump the on-chain program with `solana program dump`, you get the entire account including padding. To verify it matches the local build, you must:

1. Compare only the first 510144 bytes of the on-chain dump
2. Or rebuild locally and compare to the trimmed on-chain binary

This is why verification tools like Solscan trim before comparing. The hashes will only match when both binaries are the same size.

## Build Environment

For exact reproducibility, our CI uses:
- **Rust:** 1.91.1 (workspace) + 1.84.1-sbpf (Solana toolchain)
- **Solana/Agave:** 3.0.10
- **Anchor:** 0.32.1
- **Platform:** Linux x86_64

These versions match the verified on-chain deployment.

## Automated Verification

Our GitHub Actions workflow automatically verifies every commit:
- [.github/workflows/verify-build.yml](.github/workflows/verify-build.yml)

## No Third Parties

We perform all verification without:
- Audit firms
- Verification services
- External dependencies

Just deterministic builds and cryptographic proofs.

## Questions?

If hashes don't match:
1. Ensure you're on the correct git tag (v1.2.0 for latest)
2. Check tool versions match exactly
3. Try `cargo clean && anchor clean` before building
4. Make sure to trim on-chain binary to program size before comparing
5. Open an issue with your build output

---

*Built and verified by the team. No middlemen.*
