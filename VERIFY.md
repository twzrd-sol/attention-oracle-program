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
git checkout v1.1.0  # Use the specific release tag
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
# Get on-chain hash
solana program show GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop --url mainnet-beta

# Get your local build hash
sha256sum target/deploy/token_2022.so

# v1.2.1 Expected hash (Docker build, trimmed to 534224 bytes): 8e60919edb1792fa496c20c871c10f9295334bf2b3762d482fd09078c67a0281
```

### 5. Verify on Solscan
Navigate to: https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

Look for the "Verified" badge and matching source code link.

**Verified Hash (v1.2.1 - Docker Build, AMM Compatible):** `8e60919edb1792fa496c20c871c10f9295334bf2b3762d482fd09078c67a0281`
**Deployed:** Slot 381553914, November 21, 2025

### 6. Optional: Cryptographic Proof via `solana-verify`
```bash
solana-verify verify -u m \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --commit v1.1.0 \
  twzrd-sol/attention-oracle-program
```
The command above fetches this repository at `v1.1.0`, performs the deterministic build in a containerized environment, and compares the resulting ELF hash with the one deployed on mainnet.

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
1. Ensure you're on the correct git tag
2. Check tool versions match exactly
3. Try `cargo clean && anchor clean` before building
4. Open an issue with your build output

---

*Built and verified by the team. No middlemen.*
