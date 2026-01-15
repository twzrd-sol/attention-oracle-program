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

## Treasury Controls

The `admin_withdraw` instruction allows treasury fund movement with rate limits:

| Limit | Value | Rationale |
|-------|-------|-----------|
| Per-tx | 50M CCM | Bounds single-transaction exposure |
| Per-day | 100M CCM | ~5% of supply; 20-day minimum drain time |

**Design rationale:** Rate limits are a circuit breaker providing detection/response time if admin key is compromised. They are not a substitute for proper key management.

**Governance roadmap:**
1. Current: Single admin key with rate limits
2. Phase 2: Multisig (3-of-5) via Squads/Realms
3. Phase 3: DAO governance with timelock

See [docs/TREASURY.md](/docs/TREASURY.md) for full details.

## Audit Status

- [ ] Formal third-party audit pending
- [x] Internal security review complete (Jan 2026)
- [x] Automated testing via CI

### Internal Review Summary

A comprehensive internal review was completed covering:

| Area | Status |
|------|--------|
| Access Control | ✅ Admin/publisher separation verified |
| PDA Derivation | ✅ Seeds deterministic, bumps canonical |
| Merkle Proofs | ✅ Domain separation, sorted siblings |
| Transfer Hook | ✅ Caller validation via instruction sysvar |
| Token Handling | ✅ Fee-aware balance diff pattern |
| Arithmetic | ✅ All operations use checked math |

**Findings:** No critical or high-severity vulnerabilities identified.

**Full report:** [docs/SECURITY_AUDIT.md](/docs/SECURITY_AUDIT.md)
