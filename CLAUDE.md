# attention-oracle-program — On-Chain Programs

## Coding Principles (karpathy-skills, Apr 18 2026)

AO v2 is immutable. Every principle applies with extra force:

1. **Think before coding.** On an immutable program, an assumption that hides in code becomes permanent. Surface assumptions. State them in comments if they're load-bearing.
2. **Simplicity first.** Minimum viable IX. No speculative feature flags, no unused fields, no "future-proof" scaffolding. The binary that ships is the binary you live with.
3. **Surgical changes.** For ANY code that survives in-repo for reference (smoke tests, scripts, forensic tools): edit narrowly. Drive-by cleanup in unrelated files pollutes the audit trail.
4. **Goal-driven execution.** Each IX has a verifiable success criterion: does it dispatch correctly on-chain, does its state struct round-trip, does the litesvm test encoding the success criterion pass? Loop until all three.

See `~/.claude/projects/-home-twzrd-attention-oracle-program/memory/project_solana_economy_plan.md` for the v3 economic build plan that this repo supports.

## Overview

Solana programs powering the WZRD protocol's on-chain layer.

**AO v2 is IMMUTABLE** — ProgramData `5GyaaVmzRr2r9KcUuzt9SxBVq9ubTT5m3pH9Lzy3Kh4L` upgrade authority was set to `null` on Apr 5, 2026 (two-stage: Feb 5 Squads V4 PDA → operational keypair `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD`; Apr 5 keypair → null). No further on-chain upgrades are possible. **Channel Vault is CLOSED** (zombie state — on live RPC: "Error: Program 5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ has been closed", ProgramData null). It is no longer upgradeable.

**CRITICAL: ALL phase2 features are UNROUTED in the deployed immutable binary** (verified Apr 18, 2026 via on-chain IX probe). Binary strings are present but the instruction dispatcher omits them, so every feature-gated IX (strategy, channel_staking, prediction_markets, price_feed) returns error 101 (`InstructionFallbackNotFound`) on-chain. 92 orphan accounts on-chain (31 ChannelStakePool + 49 MarketState + 10 StrategyVault + 2 PriceFeedState) are rent-locked — no IX can read/write/close them.

**Exact deployed source is UNRESOLVED as of Apr 19, 2026.** Both candidate trees FAIL verifiable-build reproduction of on-chain hash `b5330fcc...`: (1) this repo's `origin/main` (commit `7f45bf8`) rebuilds to `15367a5...`; (2) `~/wzrd-final/programs/attention-oracle/` rebuilds to `dbdf5c...`. An earlier Apr 19 hypothesis that wzrd-final was the deployed source was REFUTED by the verifiable build test. Forensic search for the actual commit snapshot is pending. What remains confirmed: framework = Pinocchio 0.9 for this repo's source tree (per `programs/attention-oracle/Cargo.toml`), immutable since Apr 5, phase2 unrouted (on-chain-verified).

Phase 3 economic work happens in `programs/wzrd-rails/` (upgradeable Anchor 0.32.1), not via AO v2 extensions.

What DOES work in the deployed binary (verified present):
- Core attention loop: `initialize_protocol_state`, `initialize_market_vault`, `deposit_market`, `update_attention`, `update_nav`, `claim_yield`, `settle_market`
- Global V4 claims: `publish_global_root`, `claim_global_v2`, `claim_global`, `claim_global_sponsored*`
- Treasury: `route_treasury`, `set_treasury`, `realloc_market_vault`
- Fee harvest: `harvest_fees`, `withdraw_fees_from_mint`
- Admin: `realloc_legacy_protocol`, `admin_fix_ccm_authority`

See `UPGRADE_AUTHORITY.md` for the Feb 5 transfer record. `DEPLOYMENTS.md` still reflects the pre-Feb 5 Squads V4 PDA state — update pending. The on-chain truth supersedes both.

## Doc drift warnings (Apr 20 2026)

- `VERIFY.md` line 13 shows the **Feb 2026** deployed slot (`398836086`) and executable hash (`9b911dcc...`). The live program was upgraded to Pinocchio on **Mar 14 2026** (slot `411276636`, hash `b5330fcc...`) and then made **immutable on Apr 5 2026**. `VERIFY.md` needs a refresh once the deployed-source forensic lands.
- `docs/SECURITY_AUDIT.md` last edited **2026-02-09** (commit `9592e1b`) — pre-dates the authority-null, the Pinocchio port, and wzrd-rails. Treat as historical.
- `DEPLOYMENTS.md` reflects pre-Feb 5 Squads V4 PDA state.
- The on-chain state supersedes all three.

## Programs

| Program | ID | Framework | Purpose |
|---------|----|-----------|---------|
| **Attention Oracle (AO) v2** | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | **Pinocchio 0.9** (source in repo) / on-chain binary framework unresolved | **IMMUTABLE**. Core attention loop only — deposit, settle, global V4 claims, treasury routing, fee harvest. Phase2 IXs unrouted (strings present in binary, dispatcher omits — channel_staking, markets, strategy, price_feed all return error 101). On-chain hash `b5330f...`. **Deployed source snapshot NOT yet located**: neither this repo's origin/main (rebuilds to `15367a`) nor `~/wzrd-final/programs/attention-oracle/` (rebuilds to `dbdf5c`) matches on-chain. Forensic search pending. |
| **Channel Vault (CLOSED)** | `5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ` | Anchor 0.32.1 (historical) | **Closed/zombie on-chain** — no longer upgradeable, IXs unreachable. Historical: vLOFI staking vault (deposits, withdrawals, compound into AO channel_staking, ExchangeRateOracle). |

## Key Accounts

| Account | Address | Description |
|---------|---------|-------------|
| CCM Mint | `Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM` | Token-2022 mint, **9 decimals** (TransferFeeConfig only). On-chain supply = 2e18 base units = 2B CCM. |
| vLOFI Mint | `E9Kt33axpCy3ve2PCY9BSrbPhcR9wdDsWQECAahzw2dS` | Lofi 3h receipt token |
| Protocol State PDA (CCM admin chain — **LIVE**) | `vAbgvkjtVDYELfqh2xv1mbwz38WBvotTQ5hAkrPCXyP` | admin=`99MB5hviEqZP7DnqGXk8JuUh4gz6WWP2ftFu9XpQnErP` (offset 10), publisher=`87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy` (offset 42), treasury=`B6fmMVezPSsWYd5JJND43oJHpTACbrAiDRDBuYwxqGxA` (offset 74), oracle_authority=`99MB5hv...` (offset 106, same key as admin). Mint=CCM. |
| Protocol State PDA (secondary) | `596VBoVvzASAhe38CcBSJnv1LdVFPu4EdB8gw1Ko2nx3` | AO-owned 173-byte PDA but **NOT** the CCM admin chain. Footgun: same owner + same size as `vAbgvkjt...`. Always decode offsets against `vAbgvkjt...` for the live admin chain — see `memory/reference_ao_protocolstate_decode.md`. |
| Vault PDA | `7tjCgZcsK4sgV65wsNajUVRuGHQ7GZELWfTaWYbCBDTw` | Channel Vault state (closed program — vestigial on-chain) |
| ExchangeRateOracle | `9QaDWJESP1vYWGSpxRHKLmqvcQvXLkUt45aQeVRTGgZY` | CCM/vLOFI rate (bump 253) |
| Treasury ATA | `95qQ4kePYadvMQETjjqCmHJLzJq9YnrAEBwtVfzqMwAF` | Locked to claims only |
| Multisig (Squads V4) | `BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ` | 3-of-5 — RETIRED from AO v2 (authority null as of Apr 5 2026). Previously relevant for Channel Vault (now closed). |

## Architecture

```
programs/
  ├── attention-oracle/          # AO v2 source (**Pinocchio 0.9** per Cargo.toml, default=[]). Post-Apr-5 refactor shape (consolidated error.rs, custom keccak.rs, signal/velocity_feed). **Does NOT match on-chain binary** (rebuilds to `15367a`, live is `b5330f`). Deployed source snapshot NOT yet located; `~/wzrd-final/programs/attention-oracle/` also fails verification (rebuilds to `dbdf5c`). Forensic search pending.
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
  └── channel-vault/             # Anchor source — on-chain program is CLOSED (zombie). Retained for reference.
      └── src/
          ├── lib.rs             # Vault logic, staking, compound
          └── instructions/      # deposit, redeem, compound (CPIs into AO)
```

## Tech Stack

- **AO v2**: **Pinocchio 0.9** (source in repo per Cargo.toml); on-chain binary immutable, hash `b5330f...`. **Exact deployed source snapshot UNRESOLVED**: this repo's `programs/attention-oracle/` rebuilds to `15367a5...`, and `~/wzrd-final/programs/attention-oracle/` rebuilds to `dbdf5c...` — neither matches. Forensic search pending.
- **Channel Vault**: Anchor 0.32.1, `anchor build --verifiable`, 818KB binary. **On-chain program is CLOSED** — source retained for reference only.
- **wzrd-rails** (new, upgradeable): Anchor 0.32.1, `programs/wzrd-rails/`, where all new staking/claiming/rewards work happens. Program ID `BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9`.
- **Token standard**: Token-2022 (NOT legacy SPL)
- **Deploy**: AO v2 is IMMUTABLE — no further deploys possible. Channel Vault is CLOSED on-chain; Squads V4 multisig remains relevant only for a potential future redeployment to a new program ID. wzrd-rails stays upgradeable until the 6-criteria gate in the plan is met.

## Build & Test

```bash
# AO v2 (Pinocchio — cargo build-sbf, NOT anchor build; source here does NOT match on-chain, so verification requires forensic source match)
cargo build-sbf -p attention-oracle
cargo test -p attention-oracle --features localtest

# Channel Vault (Anchor — CLOSED on-chain; historical only, no deploy path)
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

- **AO v2**: IMMUTABLE — cannot be redeployed. Any bug in the phase2 code paths (`channel_staking`, `prediction_markets`, `strategy`, `price_feed`) is permanent even though they are unreachable via the dispatcher — the bytes cannot be removed. Workaround is an off-chain keeper or a NEW program.
- **Channel Vault**: CLOSED on-chain (zombie). Build command kept for historical reference; no active upgrade path exists on the original program ID.

## SBF Constraints

- Stack frame limit: 4096 bytes per function
- Pattern: extract heavy logic to `#[inline(never)]` functions
- Minimize Pubkey arguments (32 bytes each on caller stack)
- Use `crate::id()` inside callees instead of passing program_id
- Manual byte-packing for CPI instruction data (no Borsh) — AO v2 only

## Gitignore Policy

This is a public repo. Markdown files are blocked by default (`.gitignore` + pre-commit hook). Only allowlisted docs (SECURITY.md, VERIFY.md, README.md, CLAUDE.md, etc.) may be committed.
