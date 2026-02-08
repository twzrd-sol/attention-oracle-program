# Deployments

This document records public program IDs, upgrade authority, and release policy for the attention oracle programs.

## Program IDs

| Cluster | Program | Program ID | Program Data Account | Status |
|---------|---------|------------|---------------------|--------|
| mainnet-beta | Attention Oracle (Token-2022) | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | `5GyaaVmzRr2r9KcUuzt9SxBVq9ubTT5m3pH9Lzy3Kh4L` | Active |
| mainnet-beta | Channel Vault | `5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ` | `2ubXWFAJFCnBqJ1vYCsf4q8SYRcqf5DaTfkC6wASK5SQ` | Active |
| devnet       | Attention Oracle (Token-2022) | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | — | Active |

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

| Program | Upgrade Authority | Type |
|---------|-------------------|------|
| Attention Oracle | `2v9jrkuJM99uf4xDFwfyxuzoNmqfggqbuW34mad2n6kW` | Squads V4 vault PDA (3-of-5 multisig `BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ`) |
| Channel Vault | `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD` | Single signer (id.json) — transfers to Squads after Phase 2 |

To verify on-chain:
```bash
solana program show GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop --url mainnet-beta
```

## Upgrade Policy

1. **Programs are upgradeable.** The upgrade authority can deploy new bytecode at any time.
2. **No timelock.** Upgrades take effect immediately upon transaction confirmation.
3. **Attention Oracle: Squads V4 multisig (3-of-5).** Requires 3 member approvals to deploy.
4. **Channel Vault: Single signer.** Intentional — composability upgrades pending. Transfers to Squads after Phase 2.

### Governance Progress

- [x] Transfer AO upgrade authority to Squads V4 multisig
- [ ] Transfer Channel Vault upgrade authority to Squads (after Phase 2)
- [ ] Implement governance timelock for upgrades
- [x] Publish verified builds (see VERIFY.md)

## Deployment History

| Date | Program | Slot | Commit | Description |
|------|---------|------|--------|-------------|
| 2025-12-31 | token_2022 | 390,464,000 | — | Initial mainnet deployment |
| 2026-01-25 | token_2022 | 395,779,276 | — | Verified deployment |
| 2026-02-06 | token_2022 | 398,209,178 | — | Pre-verifiable AO deployment |
| 2026-02-08 | token_2022 | 398,836,086 | `430ccc6` | Verified deployment (Squads proposal #48) |
| 2026-02-08 | channel_vault | 398,811,120 | `b1a9fee` | ExchangeRateOracle PDA + auto-update on compound |
| 2026-02-08 | channel_vault | 398,835,029 | `b1a9fee` | Redeploy verifiable build (verified on-chain) |

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
