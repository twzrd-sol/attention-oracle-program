# attention-oracle-program — On-Chain Programs

## Coding Principles (karpathy-skills, Apr 18 2026)

AO v2 is immutable. Every principle applies with extra force:

1. **Think before coding.** On an immutable program, an assumption that hides in code becomes permanent. Surface assumptions. State them in comments if they're load-bearing.
2. **Simplicity first.** Minimum viable IX. No speculative feature flags, no unused fields, no "future-proof" scaffolding. The binary that ships is the binary you live with.
3. **Surgical changes.** For ANY code that survives in-repo for reference (smoke tests, scripts, forensic tools): edit narrowly. Drive-by cleanup in unrelated files pollutes the audit trail.
4. **Goal-driven execution.** Each IX has a verifiable success criterion: does it dispatch correctly on-chain, does its state struct round-trip, does the litesvm test encoding the success criterion pass? Loop until all three.

If a local project-memory plan exists for the v3 economy, treat it as operator
context only. Public repo truth must still come from tracked files plus live
RPC/on-chain proof.

## Overview

Solana programs powering the WZRD protocol's on-chain layer.

**AO v2 is IMMUTABLE** — ProgramData `5GyaaVmzRr2r9KcUuzt9SxBVq9ubTT5m3pH9Lzy3Kh4L` upgrade authority was set to `null` on Apr 5, 2026 (two-stage: Feb 5 Squads V4 PDA → operational keypair `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD`; Apr 5 keypair → null). No further on-chain upgrades are possible. **Channel Vault is CLOSED** (zombie state — on live RPC: "Error: Program 5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ has been closed", ProgramData null). It is no longer upgradeable.

**CRITICAL: ALL phase2 features are UNROUTED in the deployed immutable binary** (verified Apr 18, 2026 via on-chain IX probe). Binary strings are present but the instruction dispatcher omits them, so every feature-gated IX (strategy, channel_staking, prediction_markets, price_feed) returns error 101 (`InstructionFallbackNotFound`) on-chain. 92 orphan accounts on-chain (31 ChannelStakePool + 49 MarketState + 10 StrategyVault + 2 PriceFeedState) are rent-locked — no IX can read/write/close them.

**Exact deployed source is UNRESOLVED.** The current public source tree in
`programs/attention-oracle/` is Anchor 0.32.1 (`Cargo.toml` package
`attention-oracle-token-2022`, lib `token_2022`). It does not reproduce the live
mainnet executable hash `b5330fcc...`: the latest documented clean verifiable
build of the public source produced `15367a5...`. Treat the public source as
reference/audit material for the immutable program, not as verified deployed
source, until a build hash comparison passes.

Phase 3 economic work happens in `programs/wzrd-rails/` (upgradeable Anchor 0.32.1), not via AO v2 extensions.

What DOES work in the deployed binary (verified present):
- Core attention loop: `initialize_protocol_state`, `initialize_market_vault`, `deposit_market`, `update_attention`, `update_nav`, `claim_yield`, `settle_market`
- Global V4 claims: `publish_global_root`, `claim_global_v2`, `claim_global`, `claim_global_sponsored*`
- Treasury: `route_treasury`, `set_treasury`, `realloc_market_vault`
- Fee harvest: `harvest_fees`, `withdraw_fees_from_mint`
- Admin: `realloc_legacy_protocol`, `admin_fix_ccm_authority`

If local-only docs such as `UPGRADE_AUTHORITY.md`, `DEPLOYMENTS.md`, or
`VERIFY.md` appear in a dirty checkout, treat them as historical/evidence files
until reconciled with `README.md` and live RPC proof. The on-chain truth
supersedes all local docs.

## Doc Drift Warnings

- `CLAUDE.md` is the agent-facing truth file, but still separate source truth
  from deployed-binary truth. Do not collapse those evidence streams.
- Current tracked source is Anchor 0.32.1; older Pinocchio/source-replacement
  notes are historical unless a fresh hash reproduction proves otherwise.
- Local dirty checkout docs may pre-date the authority-null, the public-source
  replacement, or `wzrd-rails`. Check whether they are actually tracked on
  current `main` before citing them.
- The on-chain state supersedes repo docs for authority, executable hash, and
  live account state.

## Programs

| Program | ID | Framework | Purpose |
|---------|----|-----------|---------|
| **Attention Oracle / token_2022** | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | Current public source: Anchor 0.32.1; deployed source unresolved | **IMMUTABLE**. Core attention loop only — deposit, settle, global V4 claims, treasury routing, fee harvest. Phase2 IXs are not usable on the live immutable binary; prior probes returned error 101 (`InstructionFallbackNotFound`). On-chain hash `b5330f...`. The public source hash mismatch remains unresolved. |
| **wzrd-rails** | `BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9` | Anchor 0.32.1 | **Upgradeable** CCM productivity rails. New staking, reward, payout, and agentic-economy work belongs here unless explicitly directed otherwise. |
| **Channel Vault (CLOSED)** | `5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ` | Historical Anchor source, not tracked on current main | **Closed/zombie on-chain** — no longer upgradeable, IXs unreachable. Retain only as historical context unless a new program ID is introduced. |

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
  ├── attention-oracle/          # Anchor 0.32.1 public source for token_2022. It is useful for reference, tests, and audits, but it is NOT verified as the live immutable binary source until a verifiable build hash matches `b5330f...`.
  │   └── src/
  │       ├── lib.rs             # Anchor entry point and instruction router
  │       ├── state.rs           # Account structs, PDA seeds, discriminators
  │       ├── errors.rs          # Program errors
  │       ├── merkle_proof.rs    # Merkle helpers
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
  └── wzrd-rails/                # Anchor 0.32.1 upgradeable CCM productivity rails. This is the active surface for agentic-economy work.
      └── src/
          ├── lib.rs             # Staking, reward, payout, admin handlers
          ├── state.rs           # PDA seeds, account layouts, events
          ├── listen_payout.rs   # Listen payout leaf/hash helpers
          └── error.rs           # Program errors
```

## Tech Stack

- **AO v2 / token_2022**: current public source is Anchor 0.32.1; on-chain binary is immutable, hash `b5330f...`. **Exact deployed source snapshot UNRESOLVED**: the latest documented public-source verifiable build produced `15367a5...`, which does not match live. Keep source/reference truth and deployed-binary truth separate.
- **Channel Vault**: on-chain program is CLOSED; source is not part of current tracked main.
- **wzrd-rails** (new, upgradeable): Anchor 0.32.1, `programs/wzrd-rails/`, where all new staking/claiming/rewards work happens. Program ID `BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9`.
- **Token standard**: Token-2022 (NOT legacy SPL)
- **Deploy**: AO v2 is IMMUTABLE — no further deploys possible. Channel Vault is CLOSED on-chain; Squads V4 multisig remains relevant only for a potential future redeployment to a new program ID. wzrd-rails stays upgradeable until the 6-criteria gate in the plan is met.

## Build & Test

```bash
# AO v2 / token_2022 public source (Anchor; source here does NOT match live on-chain hash yet)
anchor build --verifiable --program-name token_2022
cargo test -p attention-oracle-token-2022 --features localtest --tests

# wzrd-rails active economic surface
anchor build --verifiable --program-name wzrd_rails
cargo test -p wzrd-rails --features localtest --test core_loop
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
- **Channel Vault**: CLOSED on-chain (zombie). The original program ID has no
  active upgrade path; source is not tracked on current main.

## SBF Constraints

- Stack frame limit: 4096 bytes per function
- Pattern: extract heavy logic to `#[inline(never)]` functions
- Minimize Pubkey arguments (32 bytes each on caller stack)
- Use `crate::id()` inside callees instead of passing program_id
- Keep stack-heavy logic extracted into small helpers and prove any account
  layout or PDA derivation with focused tests.

## Gitignore Policy

This is a public repo. The tracked hygiene guard blocks `.env`-style files,
obvious key material paths, hardcoded secret material, project custody paths,
and non-local IP literals in docs/config files. Keep signer paths, local machine
paths, and run-specific evidence out of public docs unless deliberately
redacted.
