# attention-oracle-program — On-Chain Programs

## Overview

Anchor workspace containing the two Solana programs that power the WZRD protocol's on-chain layer. Both programs are deployed to mainnet under Squads V4 multisig governance and have verified builds on Solscan.

## Programs

| Program | ID | Purpose |
|---------|----|---------|
| **Attention Oracle (AO)** | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | Merkle-based cumulative reward claims (V2). Stores published roots, verifies proofs, transfers CCM to claimants. |
| **Channel Vault** | `5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ` | vLOFI staking vault. Deposits, withdrawals, compound (auto-reinvest transfer fees), ExchangeRateOracle. |

## Key Accounts

| Account | Address | Description |
|---------|---------|-------------|
| CCM Mint | `Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM` | Token-2022 mint (TransferFeeConfig only) |
| vLOFI Mint | `E9Kt33axpCy3ve2PCY9BSrbPhcR9wdDsWQECAahzw2dS` | Lofi 3h receipt token |
| Protocol State PDA | `596VBoVvzASAhe38CcBSJnv1LdVFPu4EdB8gw1Ko2nx3` | AO global state |
| Vault PDA | `7tjCgZcsK4sgV65wsNajUVRuGHQ7GZELWfTaWYbCBDTw` | Channel Vault state |
| ExchangeRateOracle | `9QaDWJESP1vYWGSpxRHKLmqvcQvXLkUt45aQeVRTGgZY` | CCM/vLOFI rate (bump 253) |
| Treasury ATA | `95qQ4kePYadvMQETjjqCmHJLzJq9YnrAEBwtVfzqMwAF` | Locked to claims only |
| Multisig (Squads V4) | `BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ` | 3-of-5 upgrade authority for AO |

## Architecture

```
wzrd-core (backend)
  ├── Publishes merkle roots → AO program (claim_cumulative IX)
  ├── Relays gasless claims → AO program (claim_cumulative_sponsored IX)
  └── Triggers compound    → Channel Vault (compound IX)

programs/
  ├── token_2022/          # Attention Oracle (AO) — the "token_2022" name is historical
  │   └── src/lib.rs       # Claims, merkle verification, protocol state
  └── channel-vault/
      └── src/lib.rs       # Vault logic, staking, compound, exchange rate oracle
```

## Tech Stack

- **Framework**: Anchor 0.32.1
- **Solana SDK**: solana-program 2.x
- **Token standard**: Token-2022 (NOT legacy SPL)
- **Build**: `anchor build --verifiable` (Docker deterministic builds)
- **Deploy**: Squads V4 proposal workflow for AO; single-signer for Channel Vault (Phase 2 pending)

## Build & Test

```bash
# Local build (for testing only — NOT for deployment)
anchor build

# Verifiable build (required for deployment)
anchor build --verifiable --program-name token_2022
anchor build --verifiable --program-name channel_vault

# Tests
anchor test                                    # Full suite
cargo test -p channel-vault --lib              # Vault unit tests (57 tests)
```

## Deployment

**CRITICAL**: Never deploy from `anchor build` (host). Always use verifiable builds.

Pipeline: `anchor build --verifiable` → deploy `.so` from `target/verifiable/` → `solana-verify verify-from-repo`

AO upgrades require Squads V4 3-of-5 multisig approval. See `~/private_twzrd/defi/scripts/propose-ao-upgrade.ts`.

## SBF Constraints

- Stack frame limit: 4096 bytes per function
- Pattern: extract heavy logic to `#[inline(never)]` functions
- Minimize Pubkey arguments (32 bytes each on caller stack)
- Use `crate::id()` inside callees instead of passing program_id

## Gitignore Policy

This is a public repo. Markdown files are blocked by default (`.gitignore` + pre-commit hook). Only allowlisted docs (SECURITY.md, VERIFY.md, README.md, CLAUDE.md, etc.) may be committed.
