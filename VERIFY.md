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
git checkout v1.1.1  # Use the specific release tag
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
# Clean build
cargo clean

# Build with Solana's deterministic settings
cargo build-sbf
```

### 4. Compare Hash
```bash
# Get your local build hash
LOCAL_HASH=$(sha256sum programs/token_2022/target/deploy/token_2022.so | cut -d' ' -f1)
LOCAL_SIZE=$(stat -c%s programs/token_2022/target/deploy/token_2022.so)

# Get on-chain program and trim to actual program size
solana program dump GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop on-chain.so --url mainnet-beta
ON_CHAIN_HASH=$(head -c "$LOCAL_SIZE" on-chain.so | sha256sum | cut -d' ' -f1)

echo "Local:    $LOCAL_HASH"
echo "On-chain: $ON_CHAIN_HASH"

# Expected hash: e6bda5c18d1ac7efbec7f7761d48f326ea73fcbe3753873c4de3c5f19a017322
```

**Note:** On-chain accounts are padded with zeros to their account size. We trim to the actual program size (510936 bytes) before comparing hashes. Solscan and other verification tools handle this trimming automatically.

### 5. Verify on Solscan
Navigate to: https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

Look for the "Verified" badge and matching source code link.

**Verified Hash:** `e6bda5c18d1ac7efbec7f7761d48f326ea73fcbe3753873c4de3c5f19a017322`

## Understanding On-Chain vs Local Hashes

When comparing hashes, it's important to understand how Solana stores programs:

- **Local build** (`target/deploy/token_2022.so`): 510936 bytes - the actual compiled program
- **On-chain account**: 830936 bytes - includes 320000 bytes of padding (zero-filled slack space)

When you dump the on-chain program with `solana program dump`, you get the entire account including padding. To verify it matches the local build, you must:

1. Compare only the first 510936 bytes of the on-chain dump
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
1. Ensure you're on the correct git tag (v1.1.1)
2. Check tool versions match exactly
3. Try `cargo clean && anchor clean` before building
4. Open an issue with your build output

---

*Built and verified by the team. No middlemen.*
