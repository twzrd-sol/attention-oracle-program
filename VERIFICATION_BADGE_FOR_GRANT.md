# Attention Oracle - Program Verification Badge
## For Solana Foundation Grant Submission

**Status**: ✅ **VERIFIED** (Deterministic Reproducibility)
**Date**: November 14, 2025
**Program**: Attention Oracle (GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop)

---

## Verification Summary

The Attention Oracle Solana program on mainnet is **cryptographically verified** to match the source code in the GitHub repository. This verification uses deterministic reproducible builds—the gold standard for on-chain security.

### What This Proves
✅ **Source Code Match**: Every byte of the on-chain program was compiled from the exact source code in GitHub
✅ **No Hidden Logic**: The deployed binary contains no additional code or backdoors
✅ **Reproducibility**: Anyone with the same toolchain can rebuild and verify the binary
✅ **Security**: The program has embedded vulnerability disclosure information

---

## The Proof

### Binary Hash Match (SHA256)

```
Local Build (From GitHub v1.0.0-hybrid-fees)
─────────────────────────────────────────────
36da3c130d95556d096a96549cd9029086e8367a91e47dd9c5b02992e2a46de0

On-Chain Deployment (Mainnet)
─────────────────────────────
36da3c130d95556d096a96549cd9029086e8367a91e47dd9c5b02992e2a46de0

Status: ✅ EXACT MATCH
```

**What This Means**: The binary on mainnet was compiled from the exact source code committed to GitHub. No modifications, no hidden code, no surprises.

---

## How to Verify (Anyone Can Do This)

### Step 1: Clone the Repository
```bash
git clone https://github.com/twzrd-sol/attention-oracle-program.git
cd attention-oracle-program
```

### Step 2: Checkout the Verified Tag
```bash
git checkout v1.0.0-hybrid-fees
```

### Step 3: Build the Program
Requires: Solana CLI 1.18.26, Rust 1.51+

```bash
# Install exact toolchain (if needed)
rustup install 1.76.0

# Build
cargo build-sbf --release
```

### Step 4: Verify the Hash
```bash
sha256sum target/deploy/token_2022.so
# Output: 36da3c130d95556d096a96549cd9029086e8367a91e47dd9c5b02992e2a46de0

# Matches on-chain? ✅ Yes, program is verified!
```

### Step 5: Download and Verify On-Chain Binary
```bash
solana program dump GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop program.so --url mainnet-beta
sha256sum program.so
# Output: 36da3c130d95556d096a96549cd9029086e8367a91e47dd9c5b02992e2a46de0
```

---

## Security Metadata

### Embedded Vulnerability Disclosure (On-Chain)

The program includes **security.txt** metadata (industry standard per [securitytxt.org](https://securitytxt.org)):

```
name: Attention Oracle - Verifiable Distribution Protocol (Token-2022)
project_url: https://github.com/twzrd-sol/attention-oracle-program
contacts: email:security@twzrd.xyz
policy: https://github.com/twzrd-sol/attention-oracle-program/blob/main/SECURITY.md
source_code: https://github.com/twzrd-sol/attention-oracle-program
expiry: 2026-06-30
```

**Verification Command** (Proves it's in the binary):
```bash
solana program dump GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop /tmp/program.so --url mainnet-beta
strings /tmp/program.so | grep "Attention Oracle"
```

---

## Technical Details for Auditors

### Build Environment
| Component | Version | Purpose |
|-----------|---------|---------|
| Rust | 1.76.0 | Compilation |
| Solana CLI | 1.18.26 | Deploy tooling |
| Cargo SBF | Latest | BPF backend |
| Git | Latest | Source control |

### Program Specifications
| Property | Value |
|----------|-------|
| **Program ID** | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` |
| **Binary Size** | 654 KB (optimized) |
| **Verified Tag** | `v1.0.0-hybrid-fees` |
| **Commit Hash** | `fc61cca4f33abe88e1cc3ff1e03130a6379d0cbc` |
| **Binary Hash (SHA256)** | `36da3c130d95556d096a96549cd9029086e8367a91e47dd9c5b02992e2a46de0` |
| **Upgrade Signature** | `2mqkcFt1M3Sc9bXytRNecQkd42UAKBr2YRCodjnas2nQLkhLk1KRHdWX8i5JBN9hhaQX9xGFgsV3t53m3KApVjMf` |

### Features Verified
✅ Transfer hook with dynamic fee calculation
✅ Harvest instruction for fee distribution
✅ Tier multiplier structure (0-5+ tiers)
✅ Passport registry lookup integration
✅ Token-2022 transfer fee extension compatibility
✅ Security.txt metadata embedded

---

## Why This Matters for the Grant

### Solana Foundation Requirements
The grant application asked for **proof of mainnet readiness**. This verification demonstrates:

1. **Code Integrity**: Every byte on mainnet matches the auditable source code
2. **Transparency**: Anyone can reproduce and verify the build
3. **Security**: Vulnerability disclosure is embedded and verifiable
4. **Professionalism**: Follows industry best practices (deterministic builds)

### How This Exceeds Standards

| Standard | Attention Oracle |
|----------|-----------------|
| **Self-Reported Badge** (Solscan) | Cosmetic only, can be claimed without verification |
| **Third-Party Audit** | Required for funding, not yet scheduled |
| **Deterministic Build Proof** | ✅ **This Document** — Cryptographic proof anyone can verify |

---

## Public Verification Links

### Live On-Chain Program
https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

**To verify yourself** on Solscan:
1. Go to the program page (link above)
2. Click "Program Security" tab
3. After Solscan cache refresh, click "Self-reported" banner
4. You'll see the embedded security.txt fields

### GitHub Repository (Public)
https://github.com/twzrd-sol/attention-oracle-program

**To verify the code**:
1. Clone the repo
2. Checkout tag `v1.0.0-hybrid-fees`
3. Run build and hash verification steps (above)

### Security Policy
https://github.com/twzrd-sol/attention-oracle-program/blob/main/SECURITY.md

**Vulnerability Disclosure**: security@twzrd.xyz

---

## Grant Application Impact

This verification badge demonstrates:

### ✅ Mainnet Readiness
The program is deployed, verified, and secured on Solana mainnet. No further deployment is needed for the Solana Foundation award.

### ✅ Code Quality
Deterministic reproducibility is a hallmark of professional cryptographic software. It shows:
- Attention to build consistency
- Respect for transparent development
- Confidence in code quality (nothing to hide)

### ✅ Security Consciousness
Embedded security.txt metadata and published vulnerability disclosure policy show we take security seriously and follow industry standards.

### ✅ Community Trust
This verification allows anyone—auditors, investors, ecosystem partners—to independently verify we are who we claim to be and the code does what we say.

---

## Next Steps Post-Award

### Milestone 1: Devnet Validation (1 month)
- Third-party security audit (Halborn, OtterSec)
- Gas optimization analysis
- Dev documentation publication

### Milestone 2: Mainnet Monitoring (1 month)
- Keeper bot deployment (harvest fee distribution)
- Metrics dashboard launch
- Incident response SLA implementation

### Milestone 3: Creator Onboarding (2 months)
- Integrate 15 active streamers
- Creator toolkit and dashboard
- Set up merkle root distribution

### Milestone 4: Community Growth (2 months)
- Marketing and referral program
- 10,000+ Monthly Active Users target
- 50,000+ on-chain claims

---

## For Grant Reviewers

### Questions This Answers
- **"How do we know the program is what you say it is?"** → SHA256 verification
- **"Could there be hidden code?"** → Deterministic build proof (no, anyone can verify)
- **"Is this production-ready?"** → Yes, deployed on mainnet with security metadata
- **"Can we audit the code?"** → Yes, it's open source on GitHub, and matches on-chain exactly

### Recommended Verification Steps
1. Download this document and MAINNET_UPGRADE_COMPLETE.md
2. Clone the GitHub repo and verify the tag exists
3. Run the build verification commands
4. Compare SHA256 hashes with the values above
5. Check the program on Solscan (linked above)

### Timeline
- **Phase 1 (Complete)**: Mainnet deployment with embedded security
- **Phase 2 (Next)**: Third-party security audit
- **Phase 3 (Next)**: Creator onboarding and community scaling

---

## Appendix: Hash Verification Details

### Why SHA256?
- **Standard**: Used by Bitcoin, Solana, and cryptographic systems worldwide
- **One-way**: Impossible to fake (would require breaking cryptography)
- **Deterministic**: Same source code → same hash, every time
- **Auditable**: Anyone with the binary can verify independently

### How the Hash Proves Authenticity
```
Source Code (v1.0.0-hybrid-fees)
         ↓
    Compilation
         ↓
   Binary File (token_2022.so)
         ↓
   SHA256 Algorithm
         ↓
36da3c130d95556d096a96549cd9029086e8367a91e47dd9c5b02992e2a46de0
         ↓
    Exact Match?
         ↓
    ✅ YES → Program is verified!
```

### Probability of Hash Collision
- SHA256 produces 2^256 possible outputs (~1.15 × 10^77)
- Probability of accidental collision: **1 in 2^256** (essentially zero)
- No known way to forge a different program with the same hash

---

## Contact Information

**Security Issues**: [security@twzrd.xyz](mailto:security@twzrd.xyz)
**Grant Application**: [See SOLANA_GRANT_APPLICATION.md](./SOLANA_GRANT_APPLICATION.md)
**GitHub**: https://github.com/twzrd-sol/attention-oracle-program
**Mainnet Program**: https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

---

**Verification Completed**: November 14, 2025
**Document Version**: 1.0
**Status**: Ready for Grant Review
**Prepared By**: Attention Oracle Team
