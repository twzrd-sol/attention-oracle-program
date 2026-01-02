# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.2.x   | :white_check_mark: |
| < 0.2   | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability in this program, please report it responsibly:

**Email:** security@twzrd.xyz

**What to include:**
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Any suggested fixes (optional)

**Response timeline:**
- Acknowledgment within 48 hours
- Initial assessment within 7 days
- Fix timeline depends on severity

**Scope:**
- On-chain program logic (token_2022)
- Merkle proof verification
- Access control and PDA derivation
- Transfer hook implementation

**Out of scope:**
- Frontend/UI issues
- Third-party dependencies (report to upstream)
- Issues requiring social engineering

## Program Verification

The deployed program can be verified against this source code:

```bash
# Install solana-verify
cargo install solana-verify

# Get on-chain hash
solana-verify get-program-hash -u https://api.mainnet-beta.solana.com GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

# Build and compare
anchor build --verifiable
solana-verify get-executable-hash target/verifiable/token_2022.so
```

## Audit Status

- [ ] Formal audit pending
- [x] Internal review complete
- [x] Automated testing via CI
