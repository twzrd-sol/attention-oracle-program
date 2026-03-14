# attention-oracle-program — On-Chain Programs

## Overview

Solana programs powering the WZRD protocol's on-chain layer. Both programs are deployed to mainnet under Squads V4 multisig governance.

## Programs

| Program | ID | Framework | Purpose |
|---------|----|-----------|---------|
| **Attention Oracle (AO) v2** | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | Pinocchio 0.9 | Merkle claims, channel staking CPI targets, prediction markets, strategy vaults, price feeds. 45 IX across 9 modules. |
| **Channel Vault** | `5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ` | Anchor 0.32.1 | vLOFI staking vault. Deposits, withdrawals, compound (CPIs into AO channel_staking), ExchangeRateOracle. |

## Key Accounts

| Account | Address | Description |
|---------|---------|-------------|
| CCM Mint | `Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM` | Token-2022 mint (TransferFeeConfig only) |
| vLOFI Mint | `E9Kt33axpCy3ve2PCY9BSrbPhcR9wdDsWQECAahzw2dS` | Lofi 3h receipt token |
| Protocol State PDA | `596VBoVvzASAhe38CcBSJnv1LdVFPu4EdB8gw1Ko2nx3` | AO global state |
| Vault PDA | `7tjCgZcsK4sgV65wsNajUVRuGHQ7GZELWfTaWYbCBDTw` | Channel Vault state |
| ExchangeRateOracle | `9QaDWJESP1vYWGSpxRHKLmqvcQvXLkUt45aQeVRTGgZY` | CCM/vLOFI rate (bump 253) |
| Treasury ATA | `95qQ4kePYadvMQETjjqCmHJLzJq9YnrAEBwtVfzqMwAF` | Locked to claims only |
| Multisig (Squads V4) | `BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ` | 3-of-5 upgrade authority |

## Architecture

```
programs/
  ├── attention-oracle/          # AO v2 — Pinocchio (153KB binary)
  │   └── src/
  │       ├── lib.rs             # Entry point, 27-arm discriminator router
  │       ├── state.rs           # Account structs, PDA seeds, discriminators
  │       ├── error.rs           # 84 error variants
  │       ├── keccak.rs          # Custom Keccak-256 (no external dep)
  │       ├── klend.rs           # Kamino K-Lend CPI helpers
  │       └── instructions/
  │           ├── vault.rs       # Market vault operations (8 IX)
  │           ├── global.rs      # Merkle root + claims (6 IX)
  │           ├── governance.rs  # Fee harvesting + treasury (3 IX)
  │           ├── admin.rs       # Admin operations (5 IX)
  │           ├── channel_staking.rs  # CPI targets for channel-vault compound (5 IX)
  │           ├── markets.rs     # Prediction markets (11 IX)
  │           ├── strategy.rs    # Strategy vault CPI (5 IX)
  │           └── price_feed.rs  # Price feed oracle (3 IX)
  │
  └── channel-vault/             # Anchor (still deployed, separate program)
      └── src/
          ├── lib.rs             # Vault logic, staking, compound
          └── instructions/      # deposit, redeem, compound (CPIs into AO)
```

## Tech Stack

- **AO v2**: Pinocchio 0.9, `cargo build-sbf`, 153KB binary
- **Channel Vault**: Anchor 0.32.1, `anchor build --verifiable`, 818KB binary
- **Token standard**: Token-2022 (NOT legacy SPL)
- **Deploy**: Squads V4 proposal workflow (3-of-5 multisig) for both programs

## Build & Test

```bash
# AO v2 (Pinocchio)
cargo build-sbf -p attention-oracle
cargo test -p attention-oracle --features localtest

# Channel Vault (Anchor)
anchor build --verifiable --program-name channel_vault
cargo test -p channel-vault --lib
```

## AO v2 Feature Flags

| Flag | Instructions | Purpose |
|------|-------------|---------|
| `channel_staking` | 5 | CPI targets for channel-vault compound |
| `strategy` | 5 | K-Lend strategy vault CPI |
| `prediction_markets` | 11 | Creator prediction markets |
| `price_feed` | 3 | On-chain price oracle |
| `localtest` | — | Enables litesvm integration tests |

## Deployment

**CRITICAL**: Never deploy from host build. Always use deterministic builds.

- **AO v2**: `cargo build-sbf` (Docker) → write buffer → Squads 3/5 proposal → `solana-verify verify-from-repo`
- **Channel Vault**: `anchor build --verifiable` → deploy from `target/verifiable/` → Squads proposal

## SBF Constraints

- Stack frame limit: 4096 bytes per function
- Pattern: extract heavy logic to `#[inline(never)]` functions
- Minimize Pubkey arguments (32 bytes each on caller stack)
- Use `crate::id()` inside callees instead of passing program_id
- Manual byte-packing for CPI instruction data (no Borsh) — AO v2 only

## Gitignore Policy

This is a public repo. Markdown files are blocked by default (`.gitignore` + pre-commit hook). Only allowlisted docs (SECURITY.md, VERIFY.md, README.md, CLAUDE.md, etc.) may be committed.
