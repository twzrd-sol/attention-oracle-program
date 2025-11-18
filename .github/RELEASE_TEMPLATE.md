# Attention Oracle Release {VERSION}

## ✅ Verified Reproducible Build

| Metric | Value |
|--------|-------|
| **Program ID** | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` |
| **Binary SHA256** | `{BINARY_SHA256}` |
| **Binary Size** | {BINARY_SIZE} bytes |
| **On-Chain Match** | {ON_CHAIN_MATCH} |
| **Build Date** | {BUILD_DATE} |

---

## 🔍 Verification Instructions

### Quick Verify (On-Chain Match)

```bash
# Download deployed program
solana program dump GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop onchain.so --url mainnet-beta

# Trim to local binary size
head -c {BINARY_SIZE} onchain.so > onchain-trimmed.so

# Verify hash
sha256sum onchain-trimmed.so
# Expected: {BINARY_SHA256}
```

### Full Reproducible Build

```bash
# Clone repository
git clone https://github.com/twzrd-sol/attention-oracle-program.git
cd attention-oracle-program

# Checkout this release
git checkout {VERSION}

# Build (requires Solana CLI 2.3.0+)
cd programs
cargo build-sbf

# Verify hash
sha256sum target/deploy/token_2022.so
# Expected: {BINARY_SHA256}
```

**Detailed guide**: [VERIFY.md](https://github.com/twzrd-sol/attention-oracle-program/blob/main/VERIFY.md)

---

## 🛠️ Build Environment

| Component | Version |
|-----------|---------|
| Solana CLI | 2.3.0 (Agave/Anza) |
| Anchor | 0.32.1 |
| Rust | 1.84.0 |
| Platform | ubuntu-latest (GitHub Actions) |

**Install toolchain**:

```bash
# Solana CLI
sh -c "$(curl -sSfL https://release.anza.xyz/v2.3.0/install)"

# Anchor CLI
cargo install --git https://github.com/coral-xyz/anchor --tag v0.32.1 anchor-cli --locked

# Rust toolchain (managed by rust-toolchain.toml)
rustup default stable
```

---

## 📦 Artifacts

This release includes:

- **`token_2022.so`** - Program binary (verified reproducible)
- **`token_2022.json`** - Anchor IDL (interface definition)
- **`ottersec-submission.json`** - OtterSec Verify submission data

### Using the IDL

```bash
# Download IDL
curl -L -O https://github.com/twzrd-sol/attention-oracle-program/releases/download/{VERSION}/token_2022.json

# Generate TypeScript types
npx @coral-xyz/anchor ts-generate-from-idl \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --idl-path token_2022.json \
  --out-path ./generated
```

---

## 🚀 What's New

{CHANGELOG_SECTION}

---

## 🔗 OtterSec Verify

This release has been submitted to [OtterSec Verify](https://verify.osec.io) for independent verification.

**Submission Details**:
- **Program ID**: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- **Repository**: https://github.com/twzrd-sol/attention-oracle-program
- **Tag**: {VERSION}
- **Commit**: {COMMIT_SHA}

To verify independently:

```bash
# Download the submission JSON
curl -L -O https://github.com/twzrd-sol/attention-oracle-program/releases/download/{VERSION}/ottersec-submission.json

# Submit to OtterSec Verify
# https://verify.osec.io
```

---

## 📚 Documentation

- **Main README**: https://github.com/twzrd-sol/attention-oracle-program
- **Verification Guide**: [VERIFY.md](https://github.com/twzrd-sol/attention-oracle-program/blob/main/VERIFY.md)
- **Security Policy**: [SECURITY.md](https://github.com/twzrd-sol/attention-oracle-program/blob/main/SECURITY.md)
- **TypeScript SDK**: [sdk/typescript/README.md](https://github.com/twzrd-sol/attention-oracle-program/tree/main/sdk/typescript)
- **Rust SDK**: [sdk/rust/](https://github.com/twzrd-sol/attention-oracle-program/tree/main/sdk/rust)
- **CLI**: [cli/README.md](https://github.com/twzrd-sol/attention-oracle-program/tree/main/cli)

---

## 🧪 Testing

### Devnet Deployment

A devnet instance is available for testing:

**Devnet Program ID**: `J42avxcb6MFavCA5Snaw4u24QLznBdbLvuowxPYNdeAn`

```bash
# Test claim on devnet
npm install @attention-oracle/sdk
# (see examples in sdk/examples/)
```

### Integration Tests

```bash
# Clone and test
git clone https://github.com/twzrd-sol/attention-oracle-program.git
cd attention-oracle-program
anchor test
```

---

## 🔒 Security

### Audit Status

{AUDIT_STATUS}

### Responsible Disclosure

Found a security issue? Please report it to:

**Email**: security@twzrd.com
**PGP Key**: (See [SECURITY.md](https://github.com/twzrd-sol/attention-oracle-program/blob/main/SECURITY.md))

**Do not** create a public GitHub issue for security vulnerabilities.

---

## 📊 On-Chain Metrics

| Metric | Value |
|--------|-------|
| Deploy Slot | {DEPLOY_SLOT} |
| Deploy Transaction | {DEPLOY_TX} |
| Program Data Address | {PROGRAM_DATA_ADDRESS} |
| Authority | {AUTHORITY_PUBKEY} |
| Lamports | {LAMPORTS} SOL |

**Explorer Links**:
- [Solscan](https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop)
- [Solana Explorer](https://explorer.solana.com/address/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop)

---

## 💾 Rollback

If you need to rollback to a previous version:

```bash
# Download previous verified binary
gh release download {PREVIOUS_VERSION} -p token_2022.so -O rollback.so

# Deploy (requires upgrade authority)
solana program deploy rollback.so \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --upgrade-authority <authority-keypair> \
  --url mainnet-beta
```

**Previous Release**: [v{PREVIOUS_VERSION}](https://github.com/twzrd-sol/attention-oracle-program/releases/tag/{PREVIOUS_VERSION})

---

## 🤝 Contributing

We welcome contributions! See:
- [CONTRIBUTING.md](https://github.com/twzrd-sol/attention-oracle-program/blob/main/CONTRIBUTING.md) (if exists)
- [Open Issues](https://github.com/twzrd-sol/attention-oracle-program/issues)
- [Discussions](https://github.com/twzrd-sol/attention-oracle-program/discussions)

---

## 📄 License

Dual MIT/Apache-2.0

---

**Release Manager**: Attention Oracle Team
**Build Automation**: GitHub Actions (Release Autopilot)
**Verification**: Reproducible + OtterSec

🤖 Generated with [Claude Code](https://claude.com/claude-code)
