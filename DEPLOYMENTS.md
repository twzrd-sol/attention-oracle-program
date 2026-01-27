# Deployments

This document records public program IDs, upgrade authority, and release policy for the attention oracle programs.

## Program IDs

| Cluster | Program | Program ID | Program Data Account | Status |
|---------|---------|------------|---------------------|--------|
| mainnet-beta | token_2022 | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | `5GyaaVmzRr2r9KcUuzt9SxBVq9ubTT5m3pH9Lzy3Kh4L` | Active |
| devnet       | token_2022 | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | — | Active |

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

For a continuously updated snapshot (last deployed slot, hash, and verification), see:

- `docs/LIVE_STATUS.md`

## Upgrade Authority

| Program | Upgrade Authority |
|---------|-------------------|
| token_2022 | `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD` |

To verify on-chain:
```bash
solana program show GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop --url mainnet-beta
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
