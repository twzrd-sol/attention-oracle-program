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

## Mainnet Status (On-Chain)

The canonical source of truth for **ProgramData Address**, **Upgrade Authority**, and **Last Deployed Slot** is:

```bash
solana program show <PROGRAM_ID> --url mainnet-beta
```

To convert a slot to a timestamp:

```bash
solana block-time <SLOT> --url mainnet-beta
```

To fetch the current on-chain executable hash:

```bash
solana-verify get-program-hash -u https://api.mainnet-beta.solana.com <PROGRAM_ID>
```

### token_2022

- Program ID: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- Loader: `BPFLoaderUpgradeable`
- ProgramData: `5GyaaVmzRr2r9KcUuzt9SxBVq9ubTT5m3pH9Lzy3Kh4L`
- Upgrade authority: `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD`
- Last deployed slot: `390464000` (`2025-12-31T21:06:25Z`)
- On-chain executable hash: `5898135a6fe46985d4329c6b18387593b9fc0c3ca5572c8133df2d59922916fe`
- Verification status: **Pending** (does not currently match a verifiable build from repo `main`)

### ccm_hook

- Program ID: `8VE2Wt5JNusGUCwkrpVmdwgdgyD6vYPEHop2g2CAArzS`
- Loader: `BPFLoaderUpgradeable`
- ProgramData: `GsSC1Apdea7sPiqPvv9ed18zNL36FAXzQ3VPk8GRxQC9`
- Upgrade authority: `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD`
- Last deployed slot: `384832984` (`2025-12-06T08:54:41Z`)
- On-chain executable hash: `394a919a7b816c3ae323de1ea9927767af50f451c243670b39fed45e2298fa90`
- Verification status: **Pending** (does not currently match a verifiable build from repo `main`)

> Note: The `main` branch may contain features that are not yet deployed on mainnet. Treat this document and `VERIFY.md`
> as canonical for the on-chain status until verification is green.

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
| 2025-12-31 | token_2022 | 390464000 | Current mainnet deployment |
| 2025-12-06 | ccm_hook   | 384832984 | Current mainnet deployment |

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
