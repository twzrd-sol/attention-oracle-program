# Live Status

This document tracks **on-chain deployment facts** and the current **verification status** for the programs in this
repository.

Until a repo commit is proven to reproduce the on-chain hash via a verifiable build (and is tagged), any statement like
"this exact commit is live" is intentionally treated as **Unknown**.

**Last updated:** 2026-01-02

---

## Quick Status

| Program | Verification | Deployed Slot (UTC) | On-Chain Hash | Matching Repo Commit |
|---------|--------------|---------------------|--------------|----------------------|
| token_2022 | 🟡 Pending | `390464000` (2025-12-31T21:06:25Z) | `5898135a6fe46985d4329c6b18387593b9fc0c3ca5572c8133df2d59922916fe` | Unknown |
| ccm_hook | 🟡 Pending | `384832984` (2025-12-06T08:54:41Z) | `394a919a7b816c3ae323de1ea9927767af50f451c243670b39fed45e2298fa90` | Unknown |

---

## token_2022

**Program ID:** `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`

| Property | Value |
|----------|-------|
| Last Deployed Slot | `390464000` |
| Deploy Timestamp | 2025-12-31T21:06:25Z |
| On-Chain Hash | `5898135a6fe46985d4329c6b18387593b9fc0c3ca5572c8133df2d59922916fe` |
| Matching Repo Commit | Unknown (not yet tagged/verified) |
| Verification Status | 🟡 Pending |

---

## ccm_hook

**Program ID:** `8VE2Wt5JNusGUCwkrpVmdwgdgyD6vYPEHop2g2CAArzS`

| Property | Value |
|----------|-------|
| Last Deployed Slot | `384832984` |
| Deploy Timestamp | 2025-12-06T08:54:41Z |
| On-Chain Hash | `394a919a7b816c3ae323de1ea9927767af50f451c243670b39fed45e2298fa90` |
| Matching Repo Commit | Unknown (not yet tagged/verified) |
| Verification Status | 🟡 Pending |

---

## Verification Commands

Source of truth:

- `DEPLOYMENTS.md` (program IDs, slots, release policy)
- `VERIFY.md` (how to reproduce and verify)

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

### Build from a specific commit and verify
```bash
# Build verifiable artifacts from a specific commit, then compare hashes.
# See VERIFY.md for the recommended workflows (solana-verify verify-from-repo / anchor verify).
```

### View diff from a candidate commit to main
```bash
git diff <COMMIT>..main -- programs/token_2022/
git diff <COMMIT>..main -- programs/ccm_hook/
```

---

## Tagging Convention

Once verification is complete, tag the deployed commits:

```bash
git tag -a mainnet/token_2022@390464000 <COMMIT> -m "Mainnet deploy slot 390464000"
git tag -a mainnet/ccm_hook@384832984 <COMMIT> -m "Mainnet deploy slot 384832984"
git push origin --tags
```

---

## Legend

| Status | Meaning |
|--------|---------|
| 🟢 Verified | On-chain hash matches verifiable build from tagged commit |
| 🟡 Pending | On-chain hash recorded; matching repo commit not yet tagged/verified |
| 🔴 Mismatch | On-chain hash does not match a verifiable build for any attempted commit |
