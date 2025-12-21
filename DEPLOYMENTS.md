# Deployments

This document records public program IDs, upgrade authority, and release policy for the TWZRD attention oracle programs.

## Program IDs

| Cluster | Program | Program ID | Program Data Account |
|---------|---------|------------|---------------------|
| mainnet-beta | token_2022 | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | `5GyaaVmzRr2r9KcUuzt9SxBVq9ubTT5m3pH9Lzy3Kh4L` |
| mainnet-beta | ccm_hook   | `8VE2Wt5JNusGUCwkrpVmdwgdgyD6vYPEHop2g2CAArzS` | `GsSC1Apdea7sPiqPvv9ed18zNL36FAXzQ3VPk8GRxQC9` |
| devnet       | token_2022 | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | — |
| devnet       | ccm_hook   | `8VE2Wt5JNusGUCwkrpVmdwgdgyD6vYPEHop2g2CAArzS` | — |

Program IDs are sourced from `Anchor.toml`. Update this table when IDs change.

## Upgrade Authority

| Program | Upgrade Authority |
|---------|-------------------|
| token_2022 | `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD` |
| ccm_hook   | `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD` |

Both programs share the same upgrade authority. This key controls all program upgrades on mainnet.

To verify on-chain:
```bash
solana program show GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop --url mainnet-beta
solana program show 8VE2Wt5JNusGUCwkrpVmdwgdgyD6vYPEHop2g2CAArzS --url mainnet-beta
```

## Upgrade Policy

1. **Programs are upgradeable.** The upgrade authority can deploy new bytecode at any time.
2. **No timelock.** Upgrades take effect immediately upon transaction confirmation.
3. **Single signer.** The upgrade authority is a single keypair (not a multisig).

### Planned Improvements

- [ ] Transfer upgrade authority to a multisig (e.g., Squads)
- [ ] Implement governance timelock for upgrades
- [ ] Publish verified builds for each release

## Deployment History

| Date | Program | Slot | Description |
|------|---------|------|-------------|
| — | token_2022 | 387878036 | Current deployment |
| — | ccm_hook   | 384832984 | Current deployment |

## Release Process

1. Tag a release commit (e.g., `v1.0.0`).
2. Build verifiable artifacts:
   ```bash
   anchor build --verifiable
   ```
3. Verify binaries match on-chain (`VERIFY.md`).
4. Deploy via upgrade authority:
   ```bash
   anchor upgrade --program-id <PROGRAM_ID> target/deploy/<program>.so
   ```
5. Update this document with new slot and description.

## Verification

See `VERIFY.md` for instructions on verifying deployed bytecode matches source.
