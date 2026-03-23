# Deployments

Program IDs, upgrade authority, and deployment history for the Attention Oracle programs.

## Program IDs

| Cluster | Program | Program ID | Status |
|---------|---------|------------|--------|
| mainnet-beta | ao-v2 (Attention Oracle) | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | Active |
| mainnet-beta | Channel Vault | `5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ` | Active |

## Upgrade Authority

| Program | Upgrade Authority | Type |
|---------|-------------------|------|
| ao-v2 | `2v9jrkuJM99uf4xDFwfyxuzoNmqfggqbuW34mad2n6kW` | Squads V4 vault PDA (3-of-5) |
| Channel Vault | `2v9jrkuJM99uf4xDFwfyxuzoNmqfggqbuW34mad2n6kW` | Squads V4 vault PDA (3-of-5) |

Multisig: `BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ`

Verify on-chain:
```bash
solana program show GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
```

See [UPGRADE_AUTHORITY.md](UPGRADE_AUTHORITY.md) for full proposal history and upgrade process.

## Upgrade Policy

1. **Programs are upgradeable.** Upgrade authority can deploy new bytecode.
2. **No timelock.** Upgrades take effect immediately upon transaction confirmation.
3. **Squads V4 multisig (3-of-5).** Both programs require 3 member approvals.

## Deployment History

| Date | Program | Proposal | Description |
|------|---------|----------|-------------|
| 2026-03-23 | ao-v2 | #158 | On-chain security.txt, NonTransferable mint opcode fix |
| 2026-03-14 | ao-v2 | #136 | Hotfix: fee_harvest, scoring, compound |
| 2026-03-14 | ao-v2 | #135 | **Pinocchio v2 rewrite** (Anchor to raw BPF, 867KB to 153KB) |
| 2026-03-12 | ao-v2 | #117 | NAV underflow fix, V2 claims, MIN_MULTIPLIER_BPS |
| 2026-03-10 | ao-v2 | #116 | Governance fixes: fee_harvest, route_treasury |
| 2026-03-10 | ao-v2 | #112 | PDA realloc 141 to 173 bytes |
| 2026-03-10 | ao-v2 | #110 | Strategy vault, Kamino CPI, governance, staking, markets, price feed |
| 2026-02-08 | token_2022 | #48 | Verified Anchor deployment |
| 2026-02-08 | channel_vault | — | Verified Anchor deployment, authority to Squads |
| 2025-12-31 | token_2022 | — | Initial mainnet deployment |

## Verification

See [VERIFY.md](VERIFY.md) for build and verification instructions.

```bash
# Quick hash check
solana-verify get-program-hash GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
```
