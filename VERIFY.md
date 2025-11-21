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

# v1.1.0 Expected hash: 97f9880ddf21ba9d1b50c45ed7717e7bf646f23a203bf10392329ca8e416f1cf
# v1.2.0 Expected hash: 357047e93929b6ad8f6879575b0633d2ae97d7ec78475a48c73000d6156b8a27 (AMM-compatible)
```

### 5. Verify on Solscan
Navigate to: https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

Look for the "Verified" badge and matching source code link.

**Verified Hash (v1.1.0):** `97f9880ddf21ba9d1b50c45ed7717e7bf646f23a203bf10392329ca8e416f1cf`
**Verified Hash (v1.2.0 - AMM Compatible):** `357047e93929b6ad8f6879575b0633d2ae97d7ec78475a48c73000d6156b8a27`

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
