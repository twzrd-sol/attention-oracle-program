# attention-oracle-program — On-Chain Program

## Overview

Pinocchio-based Solana program powering the WZRD protocol's on-chain layer. Deployed to mainnet under Squads V4 multisig governance. Upgraded from Anchor to Pinocchio on 2026-03-14 (Proposal #135, slot 406276901).

## Program

| Program | ID | Purpose |
|---------|----|---------|
| **Attention Oracle (AO) v2** | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | Merkle-based cumulative reward claims, channel staking, prediction markets, strategy vaults, price feeds. 45 instructions across 9 modules. |

## Key Accounts

| Account | Address | Description |
|---------|---------|-------------|
| CCM Mint | `Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM` | Token-2022 mint (TransferFeeConfig only) |
| vLOFI Mint | `E9Kt33axpCy3ve2PCY9BSrbPhcR9wdDsWQECAahzw2dS` | Lofi 3h receipt token |
| Protocol State PDA | `596VBoVvzASAhe38CcBSJnv1LdVFPu4EdB8gw1Ko2nx3` | AO global state |
| Treasury ATA | `95qQ4kePYadvMQETjjqCmHJLzJq9YnrAEBwtVfzqMwAF` | Locked to claims only |
| Multisig (Squads V4) | `BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ` | 3-of-5 upgrade authority |

## Architecture

```
programs/
  └── attention-oracle/          # AO v2 — Pinocchio (153KB binary)
      └── src/
          ├── lib.rs             # Entry point, 27-arm discriminator router
          ├── state.rs           # Account structs, PDA seeds, discriminators
          ├── error.rs           # 84 error variants
          ├── keccak.rs          # Custom Keccak-256 (no external dep)
          ├── klend.rs           # Kamino K-Lend CPI helpers
          └── instructions/
              ├── vault.rs       # Market vault operations (8 IX)
              ├── global.rs      # Merkle root + claims (6 IX)
              ├── governance.rs  # Fee harvesting + treasury (3 IX)
              ├── admin.rs       # Admin operations (5 IX)
              ├── channel_staking.rs  # Channel staking (5 IX)
              ├── markets.rs     # Prediction markets (11 IX)
              ├── strategy.rs    # Strategy vault CPI (5 IX)
              └── price_feed.rs  # Price feed oracle (3 IX)
```

## Tech Stack

- **Framework**: Pinocchio 0.9 (no Anchor)
- **Token standard**: Token-2022 (NOT legacy SPL)
- **Build**: `cargo build-sbf` (deterministic builds via Docker)
- **Deploy**: Squads V4 proposal workflow (3-of-5 multisig)
- **Binary**: ~153KB active code (vs 867KB Anchor predecessor)

## Build & Test

```bash
# Local build
cargo build-sbf

# Tests (require localtest feature)
cargo test -p attention-oracle --features localtest

# Feature-gated builds
cargo build-sbf --features channel_staking,strategy,prediction_markets,price_feed
```

## Feature Flags

| Flag | Instructions | Purpose |
|------|-------------|---------|
| `channel_staking` | 5 | Channel staking + fee config |
| `strategy` | 5 | K-Lend strategy vault CPI |
| `prediction_markets` | 11 | Creator prediction markets |
| `price_feed` | 3 | On-chain price oracle |
| `localtest` | — | Enables litesvm integration tests |

## Deployment

**CRITICAL**: Never deploy from host build. Always use deterministic Docker builds.

Pipeline: `cargo build-sbf` (Docker) → write buffer to mainnet → Squads 3/5 proposal → `solana-verify verify-from-repo`

## SBF Constraints

- Stack frame limit: 4096 bytes per function
- Pattern: extract heavy logic to `#[inline(never)]` functions
- Minimize Pubkey arguments (32 bytes each on caller stack)
- Use `crate::id()` inside callees instead of passing program_id
- Manual byte-packing for CPI instruction data (no Borsh)
- All discriminators computed as `const fn` at compile time

## Gitignore Policy

This is a public repo. Markdown files are blocked by default (`.gitignore` + pre-commit hook). Only allowlisted docs (SECURITY.md, VERIFY.md, README.md, CLAUDE.md, etc.) may be committed.
