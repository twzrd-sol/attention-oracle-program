# Live Status

This document tracks the exact relationship between on-chain deployed programs and repository commits.

**Last updated:** 2026-01-02

---

## Quick Status

| Program | Verification | Live Commit | Main Ahead By |
|---------|--------------|-------------|---------------|
| token_2022 | 🟡 Pending | `3042848` | 3 files |
| ccm_hook | 🟡 Pending | `a56b21b` | 2 files |

---

## token_2022

**Program ID:** `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`

| Property | Value |
|----------|-------|
| Last Deployed Slot | `390464000` |
| Deploy Timestamp | 2025-12-31T21:06:25Z |
| On-Chain Hash | `5898135a6fe46985d4329c6b18387593b9fc0c3ca5572c8133df2d59922916fe` |
| **Live Commit** | `3042848` |
| Live Commit Date | 2025-12-31T11:39:36Z |
| Live Commit Message | `fix: use u128 for creator fee calculation to prevent overflow` |
| Verification Status | 🟡 Pending (hash match unconfirmed via verifiable build) |

### Files Changed Since Live (main vs 3042848)

```
M  programs/token_2022/src/instructions/cleanup.rs
M  programs/token_2022/src/instructions/cumulative.rs
M  programs/token_2022/src/lib.rs
```

### NOT Live on Mainnet (in main but not deployed)

| Commit | Date | Description |
|--------|------|-------------|
| `c5c29fd` | 2026-01-02 | chore: remove unused imports |
| `476e3ef` | 2026-01-02 | docs: update deployments + verification |
| `b68c01e` | 2026-01-01 | add update_channel_creator_fee instruction |

---

## ccm_hook

**Program ID:** `8VE2Wt5JNusGUCwkrpVmdwgdgyD6vYPEHop2g2CAArzS`

| Property | Value |
|----------|-------|
| Last Deployed Slot | `384832984` |
| Deploy Timestamp | 2025-12-06T08:54:41Z |
| On-Chain Hash | `394a919a7b816c3ae323de1ea9927767af50f451c243670b39fed45e2298fa90` |
| **Live Commit** | `a56b21b` |
| Live Commit Date | 2025-12-06T08:56:58Z |
| Live Commit Message | `feat: PUSH distribution + Transfer Hook` |
| Verification Status | 🟡 Pending (hash match unconfirmed via verifiable build) |

### Files Changed Since Live (main vs a56b21b)

```
M  programs/ccm_hook/Cargo.toml
M  programs/ccm_hook/src/lib.rs
```

---

## Verification Commands

### Dump on-chain binaries
```bash
solana program dump -u m GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop mainnet-token_2022.so
solana program dump -u m 8VE2Wt5JNusGUCwkrpVmdwgdgyD6vYPEHop2g2CAArzS mainnet-ccm_hook.so
```

### Get on-chain executable hash
```bash
solana-verify get-program-hash -u https://api.mainnet-beta.solana.com GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
solana-verify get-program-hash -u https://api.mainnet-beta.solana.com 8VE2Wt5JNusGUCwkrpVmdwgdgyD6vYPEHop2g2CAArzS
```

### Build from live commit and verify
```bash
# token_2022
git checkout 3042848
anchor build --verifiable -p token_2022
solana-verify get-executable-hash target/verifiable/token_2022.so

# ccm_hook
git checkout a56b21b
anchor build --verifiable -p ccm_hook
solana-verify get-executable-hash target/verifiable/ccm_hook.so
```

### View diff from live to main
```bash
git diff 3042848..main -- programs/token_2022/
git diff a56b21b..main -- programs/ccm_hook/
```

---

## Tagging Convention

Once verification is complete, tag the deployed commits:

```bash
git tag -a mainnet/token_2022@390464000 3042848 -m "Mainnet deploy slot 390464000"
git tag -a mainnet/ccm_hook@384832984 a56b21b -m "Mainnet deploy slot 384832984"
git push origin --tags
```

---

## Legend

| Status | Meaning |
|--------|---------|
| 🟢 Verified | On-chain hash matches verifiable build from tagged commit |
| 🟡 Pending | Live commit identified, verification build not yet run |
| 🔴 Mismatch | On-chain hash does not match any known commit |
