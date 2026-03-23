# Upgrade Authority

## Current State

| Program | Program ID | Upgrade Authority |
|---------|-----------|-------------------|
| ao-v2 (Attention Oracle) | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | Squads V4 multisig |
| channel_vault | `5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ` | Squads V4 multisig |

**Multisig**: `BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ` (3-of-5 threshold)
**Vault PDA**: `2v9jrkuJM99uf4xDFwfyxuzoNmqfggqbuW34mad2n6kW`

## Upgrade Process

1. Build deterministic binary via Docker (`solana-verify build`)
2. Upload buffer (`solana program write-buffer`)
3. Transfer buffer authority to Squads vault
4. Create Squads vault transaction with BPF Loader `Upgrade` instruction
5. Create proposal — 2 automated approvals + 1 manual approval (3-of-5)
6. Execute upgrade
7. Verify on-chain hash matches repo (`solana-verify verify-from-repo`)

All upgrade transactions are public on-chain. See [VERIFY.md](VERIFY.md) for verification instructions.

## Proposal History

### Phase 3: Security & Verification (Mar 2026)
| # | Date | Description |
|---|------|-------------|
| 158 | Mar 23 | On-chain security.txt, NonTransferable mint opcode fix (37→32) |

### Phase 2: Pinocchio v2 + Yield Protocol (Mar 2026)
| # | Date | Description |
|---|------|-------------|
| 135 | Mar 14 | **Pinocchio v2 rewrite** — Anchor to Pinocchio, 867KB to 153KB. Same program ID. |
| 136 | Mar 14 | Hotfix: fee_harvest, scoring, compound fixes |
| 117 | Mar 12 | NAV underflow fix, CCM mint_to removal, V2 claims, MIN_MULTIPLIER_BPS |
| 116 | Mar 10 | Governance fixes: fee_harvest, route_treasury, LEGACY_BUMP_OFFSET |
| 112 | Mar 10 | PDA realloc 141 to 173 bytes (yield routing) |
| 110 | Mar 10 | Strategy vault, Kamino CPI, governance, staking, markets, price feed |

### Phase 1: Authority Migration (Feb 2026)
| # | Date | Description |
|---|------|-------------|
| 43-46 | Feb 5 | Temporary authority transfer to operational keypair (BPF Loader CPI limitation) |
| — | Feb 8 | Authority returned to Squads vault. Verified builds deployed. |

### Historical Note

On Feb 5, 2026, upgrade authority was temporarily transferred to an operational keypair (`2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD`) because the BPF Upgradeable Loader rejects CPI calls for `Upgrade` instructions — PDAs (like the Squads vault) can only sign via CPI. Authority was returned to Squads on Feb 8 after deploying verified builds with the correct upgrade flow (buffer upload + Squads proposal).

## Trust Model

- **3-of-5 multisig** — no single key can upgrade the program
- **Deterministic builds** — Docker-based builds produce reproducible binaries
- **Public verification** — anyone can verify on-chain binary matches source via `solana-verify`
- **Public source** — program source at [github.com/twzrd-sol/attention-oracle-program](https://github.com/twzrd-sol/attention-oracle-program)
- **On-chain security.txt** — embedded in binary (Neodyme standard), scanned by explorers
