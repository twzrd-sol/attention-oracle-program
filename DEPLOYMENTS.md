# Deployments

This document records public program IDs and release policy for the TWZRD attention oracle programs.

## Program IDs

| Cluster | Program | Program ID |
|---------|---------|------------|
| mainnet-beta | token_2022 | GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop |
| mainnet-beta | ccm_hook   | 8VE2Wt5JNusGUCwkrpVmdwgdgyD6vYPEHop2g2CAArzS |
| devnet       | token_2022 | GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop |
| devnet       | ccm_hook   | 8VE2Wt5JNusGUCwkrpVmdwgdgyD6vYPEHop2g2CAArzS |

Program IDs are sourced from `Anchor.toml`. Update this table when IDs change.

## Upgrade policy

- These programs are upgradeable.
- The current upgrade authority is visible on-chain via `solana program show`.
- Upgrade policy should be published before institutional integrations.

## Release process (recommended)

1. Tag a release commit.
2. Build verifiable artifacts (`anchor build --verifiable`).
3. Verify binaries on-chain (`VERIFY.md`).
4. Deploy the new program and update this document if IDs change.
