# Live Status

This document tracks **on-chain deployment facts** and the current **verification status** for the programs in this
repository.

Until a repo commit is proven to reproduce the on-chain hash via a verifiable build (and is tagged), any statement like
"this exact commit is live" is intentionally treated as **Unknown**.

**Last updated:** 2026-01-06

---

## Quick Status

| Program | Verification | Deployed Slot (UTC) | On-Chain Hash | Tagged Commit |
|---------|--------------|---------------------|--------------|---------------|
| token_2022 | 🟢 Verified | `391176164` (2026-01-04T03:00:04Z) | `ca17ba59...` | `mainnet/token_2022@391176164` (`3215f7b`) |
| ccm_hook | 🟢 Verified | `391176540` (2026-01-04T03:02:31Z) | `fae7cf0c...` | `mainnet/ccm_hook@391176540` (`3215f7b`) |

---

## token_2022

**Program ID:** `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`

| Property | Value |
|----------|-------|
| Last Deployed Slot | `391176164` |
| Deploy Timestamp | 2026-01-04T03:00:04Z |
| On-Chain Hash | `ca17ba5923d1867e1a66feb6aaa05e18b27ebeee0f98a04dcea8e5c6af6ab18d` |
| Tagged Commit | `mainnet/token_2022@391176164` → `3215f7b` |
| Verification Status | 🟢 Verified (on-chain hash matches verifiable build) |

**Drift from main:** 1 commits (repo has unreleased changes)

---

## ccm_hook

**Program ID:** `8VE2Wt5JNusGUCwkrpVmdwgdgyD6vYPEHop2g2CAArzS`

| Property | Value |
|----------|-------|
| Last Deployed Slot | `391176540` |
| Deploy Timestamp | 2026-01-04T03:02:31Z |
| On-Chain Hash | `fae7cf0cd9fcd6d19d97fd45720727f85bfd4c2d31d102d7d8b7be9df4c06140` |
| Tagged Commit | `mainnet/ccm_hook@391176540` → `3215f7b` |
| Verification Status | 🟢 Verified (on-chain hash matches verifiable build) |

**Drift from main:** 0 commits (up to date)

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

Tags created for live deployments:

```bash
# Current deployment:
mainnet/token_2022@391176164  →  3215f7b
mainnet/ccm_hook@391176540    →  3215f7b

# Previous deployments:
mainnet/token_2022@390464000  →  3042848
mainnet/ccm_hook@384832984    →  a56b21b
```

To build from a tagged commit:
```bash
git checkout mainnet/token_2022@391176164
anchor build --verifiable --program-name token_2022
```

---

## Legend

| Status | Meaning |
|--------|---------|
| 🟢 Verified | On-chain hash matches verifiable build from tagged commit |
| 🟡 Pending | On-chain hash recorded; tagged candidate exists but verifiable hash match not yet proven |
| 🔴 Mismatch | On-chain hash does not match a verifiable build for any attempted commit |
