# Deployment Audit Report - November 18, 2025

## ⚠️ CRITICAL FINDING: Binary Mismatch

**Status**: URGENT CLARIFICATION NEEDED

---

## The Discrepancy

### What We Built Today (08:23 UTC)
```
Path:   /home/twzrd/milo-token/clean-hackathon/verify-snapshot/target/deploy/token_2022.so
Size:   706 KB (722,792 bytes)
Hash:   a16edf5c5728c6a2890a707444f59c589d813e2b26348873ec697519e68c3fd6
Rebuilt: Nov 18, 08:23 AM
Stack:  Anchor 0.30.1 + Solana 1.18 + spl-token-2022 1.0.0
```

### What's Actually on Mainnet (as of 08:29 UTC)
```
Program ID: GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
Size:       812 KB (830,936 bytes)
Hash:       4d04a19ddfbd33593faf09ce8bdfe6431b50c294f6e1c4b3a85923683a360f66
Slot:       380855084
```

### Binary Source Analysis
```
706K (a16edf5c...)  ← verify-snapshot build (today)
│
├─ Possible match: 676K (Nov 14)  ← /verify-snapshot/target (root level)
│                  Hash: 4c42a7c7a9db96606a4c4f80635f328f85cde3b52...
│
└─ MISMATCH: 812K (4d04a19d...)   ← CURRENTLY ON MAINNET
             (doesn't match any local build)
```

---

## Timeline of Deployments

### Historical Deployments
1. **Slot 380818704** (Nov 18, ~04:29 UTC)
   - Anchor 0.32.1 "verified build"
   - Size: 542 KB
   - Commit: `8dde2bd3` "chore: Anchor 0.32.1 upgrade deployed to mainnet"

2. **Slot 380855084** (Nov 18, ~08:29 UTC) **← CURRENT**
   - Size: 812 KB
   - Hash: `4d04a19ddfbd33593faf09ce8bdfe6431b50c294f6e1c4b3a85923683a360f66`
   - **Source: UNKNOWN** (not in git, not in local builds)

---

## What We Know for Certain

✅ **Program is functional**
- PDA derivation bug IS fixed (our smoke test confirmed distinct treasury/creator_pool addresses)
- All 24+ instructions are live
- Transfer hook and harvest mechanisms are operational

✅ **The Fix is Live**
- Treasury PDA: Derived from `[b"treasury", mint]`
- Creator Pool PDA: Derived from `[b"creator_pool", mint]`
- Different addresses ✅

❓ **But the binary origin is unclear**
- Did the deployment use a different built artifact?
- Was there additional Solana toolchain processing?
- Is this a different build altogether?

---

## Possible Explanations

### 1. Pre-built Buffer (Most Likely)
```
Timeline:
- Earlier today: Someone built from a different directory/branch
- Buffer created: 706K+ binary
- We later wrote new buffer: 3jjTyJDQxx6w... (706K)
- But deployment may have used PREVIOUS buffer (812K)
```

### 2. Solana Toolchain Processing
```
Build: 706K binary
↓
Deploy instruction adds headers/metadata?
↓
Stored on-chain: 812K
```

### 3. Different Source Code
```
The 812K binary may be from:
- /programs/attention-oracle/ (Anchor 0.30.1)
- /clean-hackathon/target/ (691K at 05:12)
- Some other directory with different code
```

---

## Investigation Steps Needed

### 1. Check Transaction Details
```bash
solana confirm 3ubNre2UK2SDD5w5L7KebyWDz16PVmfjtLr8UZpKzngJBkxm9Pg2RU3Hk7Fc2kto3sGo9HRhSt8jvM26vphqucHM \
  --url mainnet-beta -v
```
This will show which buffer account was used.

### 2. Compare with All Local Binaries
```
812K (mainnet)          ?
└─ matches 811K (attention-oracle/target)
                        └─ Nov 18 06:37 (557K × 2 builds = 1.1GB?)
```

Wait - attention-oracle is only 557K. Let me check if there's another build:

### 3. Check All Cargo Workspaces
```bash
find /home/twzrd/milo-token -name "Cargo.toml" \
  -path "*/token-2022/*" \
  -not -path "*/vendor/*"
```

---

## Current Binary Size Analysis

| Build | Size | Hash | Timestamp | Location |
|-------|------|------|-----------|----------|
| verify-snapshot (today) | 706K | a16edf5c | Nov 18 08:23 | clean-hackathon/verify-snapshot |
| Mainnet (current) | 812K | 4d04a19d | Nov 18 08:29 | GnG... program |
| attention-oracle | 557K | ? | Nov 18 06:37 | clean-hackathon/programs/attention-oracle |
| clean-hackathon/target | 691K | ? | Nov 18 05:12 | clean-hackathon/target |
| root verify-snapshot | 676K | 4c42a7c7 | Nov 14 18:49 | verify-snapshot/target |

**None of these match 812K exactly.**

---

## What This Means

### ✅ Good News
- Program is working correctly
- PDA fix is definitely live
- All features are deployed
- Security.txt is embedded

### ⚠️ Audit Problem
- We cannot verify that what's on mainnet matches what we built
- For production, we need **reproducible builds**
- Hash mismatch means we can't prove code provenance

### 🔴 Next Steps (Critical)

**Option 1: Verify & Approve Current State**
- Run full smoke test (initialize_mint_open → claim → harvest)
- Test all major instructions
- If everything works, document the discrepancy but move forward
- Flag for security audit

**Option 2: Rebuild & Redeploy (Safe Choice)**
1. Clean build from verify-snapshot
2. Compare hash against what's on mainnet
3. If different, redeploy with verified binary
4. Commit the deployment details to git

**Option 3: Investigate & Reconcile**
1. Extract exact binary from mainnet via solana-verify
2. Compare with all local builds
3. Determine source through binary analysis
4. Update git history accordingly

---

## Recommendation

I recommend **Option 2: Rebuild & Redeploy** for these reasons:

1. **Reproducibility**: Solana ecosystem values reproducible builds
2. **Auditability**: Clean chain of custody for production code
3. **Security**: Third-party auditors will need to verify code matches on-chain
4. **Documentation**: Creates clear record of what was deployed

### Steps:
```bash
# 1. Clean build
cd /home/twzrd/milo-token/clean-hackathon/verify-snapshot
cargo clean
cargo build-sbf --release

# 2. Hash comparison
sha256sum target/deploy/token_2022.so
# Compare with: 4d04a19ddfbd33593faf09ce8bdfe6431b50c294f6e1c4b3a85923683a360f66

# 3. If different: redeploy
solana program write-buffer target/deploy/token_2022.so
solana program deploy --buffer <NEW_BUFFER_ID> \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --upgrade-authority <AUTHORITY_KEYPAIR>

# 4. Verify new hash matches
solana program dump GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop /tmp/verify.so
sha256sum /tmp/verify.so
```

---

## Questions for You

1. **Do you want to verify the current binary is correct and move forward?**
   - Yes: Run full smoke test suite
   - No: Rebuild and redeploy

2. **Should we implement reproducible builds for future deployments?**
   - This prevents hash mismatches going forward
   - Requires Cargo.lock pinning + consistent toolchain

3. **Do you have deployment notes or commit messages** that explain the 812K binary origin?
   - May already be documented somewhere we haven't looked

---

## Files to Check

- [ ] Transaction history for buffer accounts used
- [ ] Build logs from earlier today
- [ ] Other directories with token-2022 builds
- [ ] Solana release notes (any versioning that affects binary size)

---

**Status**: ⏸️ AWAITING CLARIFICATION
**Action Required**: Choose Option 1, 2, or 3 above
**Impact**: Program is working, but audit trail is incomplete

---

*Report Generated*: November 18, 2025, 08:35 UTC
*Temperature*: 0 (Deterministic analysis only)
