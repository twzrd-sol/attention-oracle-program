# wzrd-rails Deploy Runbook

**Version:** 1.1
**Date:** 2026-04-19
**Scope:** Devnet only (mainnet runbook deferred until devnet smoke test + external review complete)
**Related PR:** #76 merged as `fd88168` on `main`
**Verifiable Build Hash (solana-verify executable):** `314190d793e11bf37a0c10e65b00d088368612a869b1dc77ec930f764615639a`
**Target Program:** `wzrd-rails` at `BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9`

---

## What This Runbook Delivers

The goal is to validate the full `stake → unstake → claim → restake` loop on **devnet** with a realistic Token-2022 transfer-fee CCM mint, using the same program ID that will ship to mainnet. Once §8 sign-off passes, the only remaining mainnet blocker is external review + Squads proposal.

**Success criteria**:

1. Verifiable build hash reproduces `314190d7...` bit-for-bit.
2. Config PDA, global Pool PDA, StakeVault PDA, RewardVault PDA all initialize successfully.
3. Single-agent round trip (stake → claim → unstake → stake again) succeeds with correct Token-2022 fee accounting.
4. 10-agent concurrent load test passes with < 5% RPC-level failure rate.
5. Swarm dashboard transition `program_not_deployed → healthy` is clean (preflight succeeds first try).
6. Rollback drill: unset `SOLANA_RPC` → swarm falls back cleanly in < 60s.

**Explicit non-goals**:
- No Squads integration on devnet (deployer key is the admin).
- No economic parameterization (`reward_rate = 0` throughout; emissions off).
- No velocity → reward-rate feedback loop (deferred to post-devnet design pass).
- No compensation merkle event (L-01 off-chain tree builder still TBD).

---

## §0 — LiteSVM Pre-Flight (run before touching any cluster)

**Runs in `programs/wzrd-rails/tests/core_loop.rs` via `cargo test -p wzrd-rails --test core_loop`**.

The 13 existing LiteSVM tests validate the stake/unstake/claim flow but use a **plain Token-2022 mint without a TransferFeeConfig extension**. The production fee path (`actual_received = balance_after - balance_before`) is therefore unexercised — it passes in LiteSVM only because `actual_received == amount` (no fee taken).

**Pre-flight action** (recommended, 20 min of Rust test writing):

Add one test to `core_loop.rs` that creates a CCM-like mint with `TransferFeeConfig` initialized (e.g., 50 bps fee, basis-point granularity, maximum fee cap), runs a full stake cycle, and asserts:

- `pool.total_staked == actual_received` (not `requested_amount`)
- `user_stake.amount == actual_received`
- `unstake_amount` transferred to user is pre-fee from stake vault (user pays the outbound fee, not the pool)
- `reward_debt` is computed against `actual_received`, not `requested_amount`

Test scaffold (add to `core_loop.rs`):

```rust
#[test]
fn stake_with_transfer_fee_credits_actual_received() {
    // 1. setup_svm() with a Token-2022 mint configured with TransferFeeConfig
    //    (50 bps fee, no max cap, matching mainnet CCM).
    // 2. initialize_config / initialize_pool as usual.
    // 3. Call stake(amount = 1_000_000_000).
    // 4. Assert: pool.total_staked == 995_000_000 (post-fee)
    //            user_stake.amount == 995_000_000
    //            stake_vault.amount == 995_000_000
    // 5. Call set_reward_rate(100), advance slot, call claim, assert reward_debt
    //    arithmetic used 995_000_000 as the denominator, not 1_000_000_000.
}
```

**Why this matters**: the transfer-fee semantic is the single most likely production-only bug surface. Catching it locally in LiteSVM (seconds) is cheaper than catching it on devnet (one deploy iteration) or mainnet (irreversible rent burn).

**Exit criteria for §0**: all 14 tests green (13 existing + 1 new fee-path). Only then proceed to §1.

---

## §1 — Devnet CCM Mint Provisioning

**Purpose**: create a Token-2022 CCM mint on devnet that mirrors mainnet's `TransferFeeConfig` so the fee path is exercised during cluster smoke tests.

### §1.1 Create the devnet CCM mint

```bash
# Fund a devnet admin keypair
solana-keygen new -o ~/.config/solana/devnet-rails-admin.json --force
solana config set --url https://api.devnet.solana.com
solana config set --keypair ~/.config/solana/devnet-rails-admin.json
solana airdrop 5

# Create the Token-2022 mint with TransferFeeConfig (9 decimals, 50 bps, no max cap)
spl-token --program-id TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb \
    create-token \
    --decimals 9 \
    --transfer-fee 50 18446744073709551615 \
    --mint-authority $(solana-keygen pubkey ~/.config/solana/devnet-rails-admin.json) \
    --enable-metadata
```

The second arg `18446744073709551615` is `u64::MAX`, meaning no maximum fee cap (matches mainnet CCM's uncapped basis-point fee). Record the mint pubkey that `spl-token create-token` emits — call it `$DEVNET_CCM`.

### §1.2 Create admin's ATA and mint an initial float

```bash
# Associated Token Account for the admin (will be the treasury ATA)
spl-token --program-id TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb \
    create-account $DEVNET_CCM

# Mint 100M CCM to admin for smoke tests
spl-token --program-id TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb \
    mint $DEVNET_CCM 100000000
```

### §1.3 Record for downstream steps

Capture these into `ops/devnet/rails-state.env` (or wherever you keep cluster state):

```
DEVNET_CCM_MINT=<pubkey from §1.1>
DEVNET_ADMIN_KEYPAIR=~/.config/solana/devnet-rails-admin.json
DEVNET_ADMIN_ATA=<ATA from §1.2>
```

---

## §2 — Prerequisites

### §2.1 Toolchain (local)

| Tool | Version | Notes |
|------|---------|-------|
| Docker | ≥ 24.x | Required — the verifiable build runs inside `solanafoundation/anchor:v0.32.1`. No local Rust toolchain is needed; the container ships Rust 1.91.1. |
| Solana CLI | ≥ 1.18 | For `solana program deploy` and on-chain inspection. |
| `solana-verify` | ≥ 0.4 | For `get-executable-hash` (matches on-chain stored hash). |
| `spl-token` (Token-2022 variant) | Current | For §1 CCM mint provisioning. |
| Python ≥ 3.10 | — | Runs `scripts/devnet-init-rails.py` + swarm tests. Needs `solders` + `aiohttp`. |

**Note**: earlier drafts said "Rust 1.84+" — that was the AO v2 source-candidate toolchain and is **not** relevant to wzrd-rails builds. wzrd-rails uses 1.91.1 inside the Anchor container.

### §2.2 Keypair inventory

```bash
# Devnet admin (created in §1) — used as initial admin + upgrade authority
solana-keygen pubkey ~/.config/solana/devnet-rails-admin.json

# Program keypair — MUST be the one whose pubkey equals declare_id!
solana-keygen pubkey target/deploy/wzrd_rails-keypair.json
# Expected: BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9
```

If the pubkey doesn't match `BdSv8...`, the declared program ID in `lib.rs` is out of sync with the keypair file. **Stop and resolve before deploying** — deploying with a mismatched keypair creates a new program ID and breaks all PDAs.

### §2.3 SOL balance

Admin keypair needs ≥ 10 SOL on devnet:
- Program deploy: ~4-5 SOL for rent (varies with binary size)
- Config/Pool/Vault PDA rent: ~0.02 SOL
- Priority fees + buffer: ~0.5 SOL

```bash
solana balance --url devnet  # verify
```

---

## §3 — Build + Verify

The verifiable build runs inside the pinned Anchor Docker container. **Do NOT use `cargo build-sbf` or `cargo build-bpf` locally** — the binary hash will not match.

```bash
cd /path/to/attention-oracle-program  # or the current worktree
anchor build --verifiable --program-name wzrd_rails
```

This pulls `solanafoundation/anchor:v0.32.1` if not cached, fetches Rust 1.91.1 toolchain into the container, compiles, and writes the verifiable `.so` to `target/verifiable/wzrd_rails.so`.

**Verify the hash**:

```bash
solana-verify get-executable-hash target/verifiable/wzrd_rails.so
```

Expected output:
```
314190d793e11bf37a0c10e65b00d088368612a869b1dc77ec930f764615639a
```

If it does not match bit-for-bit, **stop**. Investigate:
- Docker image digest (`docker inspect solanafoundation/anchor:v0.32.1`)
- Source tree state (`git status` — should be clean at `fd88168`)
- Any host-level cargo cache interference (clear `target/` and rebuild)

**Raw file sha256 vs solana-verify hash**: raw `sha256sum target/verifiable/wzrd_rails.so` will return `994e094f0c6fbc8895d2a0b2f901f0e5d85e76ce21e5cc3496e6c68c6f289a4e` — that's the ELF file hash including metadata sections. The `314190d7...` hash strips ELF wrapper and hashes only executable bytecode, matching what `solana program show` reports post-deploy. Both are informative; only the `solana-verify` one should match on-chain.

---

## §4 — Deploy to Devnet

```bash
solana program deploy target/verifiable/wzrd_rails.so \
    --program-id target/deploy/wzrd_rails-keypair.json \
    --keypair ~/.config/solana/devnet-rails-admin.json \
    --upgrade-authority ~/.config/solana/devnet-rails-admin.json \
    --url https://api.devnet.solana.com
```

Critical flags:
- `--program-id target/deploy/wzrd_rails-keypair.json` — the program keypair whose pubkey equals `BdSv8...SZy9`. This fixes the deployed program at the same ID that will ship to mainnet.
- `--keypair ...devnet-rails-admin.json` — the fee payer / deployer. Can be the same as upgrade authority on devnet.
- `--upgrade-authority ...devnet-rails-admin.json` — kept intentionally (not revoked) for devnet so we can iterate. On mainnet, upgrade authority will go to Squads.

**Verify deployment**:

```bash
solana program show BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9 --url devnet
```

Expected fields:
- `Program Id: BdSv824...`
- `Owner: BPFLoaderUpgradeab1e11111111111111111111111`
- `Executable: true`
- `Data Length: ~400KB`
- `Upgrade Authority: <devnet-rails-admin pubkey>`

The on-chain `ProgramData Hash` should match `314190d7...`.

---

## §5 — Initialize Program State

State init uses the committed script `scripts/devnet-init-rails.py`. The script is idempotent — each IX is skipped if the target PDA already exists on-chain, so partial failures can be recovered by re-running.

### §5.1 Dry-run first

```bash
python3 scripts/devnet-init-rails.py \
    --rpc https://api.devnet.solana.com \
    --admin ~/.config/solana/devnet-rails-admin.json \
    --ccm-mint $DEVNET_CCM \
    --dry-run
```

The output prints every derived PDA. **Save this output** — you'll reference the Config / Pool / StakeVault / RewardVault addresses throughout the rest of the runbook.

### §5.2 Run for real

```bash
python3 scripts/devnet-init-rails.py \
    --rpc https://api.devnet.solana.com \
    --admin ~/.config/solana/devnet-rails-admin.json \
    --ccm-mint $DEVNET_CCM
```

This calls three IXs in sequence:
1. `initialize_config(ccm_mint = $DEVNET_CCM, treasury_ccm_ata = admin_ata)` — creates the Config PDA at `[b"config"]`.
2. `initialize_pool(pool_id = 0, lock_duration_slots = 1_512_000)` — creates the global 7-day pool.
3. `set_reward_rate` is **skipped** because `--reward-rate` defaults to 0. Emissions remain off until you explicitly turn them on (separate call).

If any step fails mid-sequence (e.g., tx timeout), re-run the script. The idempotency check skips completed steps.

### §5.3 Verify on-chain state

```bash
# Config PDA should exist, 141 bytes, owned by program
solana account <Config PDA from §5.1> --url devnet

# Pool PDA should exist, 61 bytes, total_staked=0, reward_rate=0
solana account <Pool PDA from §5.1> --url devnet

# StakeVault and RewardVault should exist as Token-2022 accounts
spl-token account-info <StakeVault from §5.1> --url devnet
spl-token account-info <RewardVault from §5.1> --url devnet
```

---

## §6 — Point the Swarm at Devnet

The new `swarm/stake.py` reads `RAILS_PROGRAM_ID` from an environment variable (default = mainnet pubkey `BdSv8...`). Since the program ID is **the same on devnet and mainnet** (deterministic from the program keypair), only the **RPC cluster** changes between environments.

### §6.1 Environment flip

In the swarm's process environment (Doppler, Docker compose env, or local `.env`):

```bash
# Devnet test configuration — point swarm agents at devnet RPC.
# RAILS_PROGRAM_ID is NOT changed (it's the same keypair on both clusters).
export SOLANA_RPC=https://api.devnet.solana.com
export CCM_MINT=$DEVNET_CCM  # override swarm's mainnet CCM constant for devnet test
```

**Note**: `swarm/stake.py` currently hardcodes `CCM_MINT = Pubkey.from_string("Dxk8mAb3...X2BM")` — the mainnet mint. For devnet testing, either:

- **Option A (preferred for durable testing)**: add a `CCM_MINT` env override in `stake.py`, analogous to how `RAILS_PROGRAM_ID` is loaded. One-line change:
  ```python
  CCM_MINT = Pubkey.from_string(
      os.environ.get("CCM_MINT", "Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM")
  )
  ```
- **Option B (temporary)**: hardcode the devnet CCM mint in a test-only branch you don't merge.

Option A is cleaner and should land alongside this runbook as a follow-up commit.

### §6.2 Fund devnet agents

The swarm test wallets need devnet SOL + devnet CCM:

```bash
# Mint devnet CCM to each test agent's ATA
for agent in agent-58 agent-68 agent-69 agent-70; do
    AGENT_PUBKEY=$(solana-keygen pubkey ~/.config/solana/swarm/$agent.json)
    spl-token --program-id TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb \
        create-account $DEVNET_CCM --owner $AGENT_PUBKEY
    spl-token --program-id TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb \
        mint $DEVNET_CCM 10000 --recipient-owner $AGENT_PUBKEY
    solana airdrop 1 $AGENT_PUBKEY --url devnet
done
```

### §6.3 Restart affected agents

Restart the swarm containers / processes so they pick up the new RPC + CCM env vars. Watch logs for:

- `wzrd-rails Config PDA does not exist` — means the preflight detected the program isn't there. Should NOT appear; if it does, §5 didn't land.
- `agent=X staked N CCM on-chain tx=...` — success.
- `agent=X stake lock active until slot=...` — expected for agents who already have an active stake.

---

## §7 — Smoke Tests

### §7.1 Unit tests (local, fast)

```bash
cd /home/twzrd/wzrd-final/agents/swarm-runner
python3 -m pytest tests/test_stake.py -v
```

All 10 tests should still pass — they mock the RPC layer, so they don't depend on cluster state.

### §7.2 Single-agent happy path (devnet, ~30 s)

Pick one test agent. Run a single `attempt_stake` manually (via Python REPL or a one-shot script):

```python
import asyncio, aiohttp
from solders.keypair import Keypair
from pathlib import Path
import json
from swarm import stake

keypair = Keypair.from_bytes(bytes(json.loads(Path("~/.config/solana/swarm/agent-58.json").expanduser().read_text())))

async def go():
    async with aiohttp.ClientSession() as http:
        r = await stake.attempt_stake(http, "https://api.devnet.solana.com", keypair, 1_000_000_000, "agent-58")
        print(r)

asyncio.run(go())
```

Expected: `StakeResult(status='staked', amount=1_000_000_000, tx_sig='...')`.

Verify on-chain:
```bash
solana account <user_stake PDA for agent-58> --url devnet  # should show 113 bytes, amount=995000000 (post-fee)
```

If `amount` is `1_000_000_000` instead of `995_000_000`, the TransferFee path is broken — investigate whether `§1.1`'s mint really has `TransferFeeConfig` initialized.

### §7.3 Full round trip (devnet, ~2 min)

Same agent, sequential calls:

```python
# Stake → claim (immediate, should succeed with 0 if reward_rate=0 or pay=0) → wait for lock expiry
# → unstake → re-stake. The restake flow should use attempt_restake() for the atomic claim+unstake+stake.
```

Expected:
- First `attempt_stake` → `status='staked'`
- Second `attempt_stake` → `status='already_staked'` OR `'lock_active'`
- After lock expiry (1,512,000 slots is too long for manual testing — for smoke test, re-deploy pool with `--lock-slots 100` for a 40-second lock)
- `attempt_restake` → `status='staked'` with new tx_sig

**For smoke testing, use a short lock**: re-run `scripts/devnet-init-rails.py --pool-id 1 --lock-slots 100` to create a **second** pool (pool_id=1) with a 40-second lock. Use that pool for round-trip tests. The 7-day pool stays pristine for production behavior validation.

### §7.4 Concurrent load test (devnet, ~1 min)

Script that fires 10 agents at once:

```python
import asyncio, aiohttp, json
from pathlib import Path
from solders.keypair import Keypair
from swarm import stake

async def stake_one(http, rpc, kp, agent_id):
    return await stake.attempt_stake(http, rpc, kp, 1_000_000_000, agent_id)

async def main():
    agents = []
    for i in range(10):
        raw = json.loads(Path(f"~/.config/solana/swarm/agent-{i}.json").expanduser().read_text())
        agents.append((Keypair.from_bytes(bytes(raw)), f"agent-{i}"))
    async with aiohttp.ClientSession() as http:
        rpc = "https://api.devnet.solana.com"
        results = await asyncio.gather(*[stake_one(http, rpc, kp, aid) for kp, aid in agents], return_exceptions=True)
        for r in results:
            print(r)
        failures = [r for r in results if isinstance(r, Exception) or getattr(r, "status", "") != "staked"]
        print(f"\nSUMMARY: {len(failures)}/10 failures")

asyncio.run(main())
```

**Pass criteria**: ≤ 1 of 10 fails (< 10% error rate on devnet under simultaneous submit). Most devnet failures will be priority-fee auction losses — not a correctness issue, just throughput. If correctness failures appear (`AccountNotFound`, `InvalidAccountData`, etc.), stop and investigate before proceeding.

---

## §8 — Monitoring + Alerts

Devnet monitoring can be lightweight — this is a test cluster, not prod. For mainnet, expand this list.

**Metrics to watch during §7 tests**:

| Metric | Target | Source |
|--------|--------|--------|
| % agent stakes returning `status=staked` | ≥ 90% | Python harness output |
| Config PDA lookup latency | p99 < 250ms | RPC timings |
| Priority fee spend per stake | < 0.0005 SOL | `solana transaction <sig>` output |
| StakePool.total_staked progression | monotonic ↑ | `solana account <Pool PDA>` polling |
| Any `program_not_deployed` after §5 | 0 | swarm logs |

**On failure**:
- `program_not_deployed` after §5 → §5 didn't land; re-run the init script.
- `AccountNotFound` on Config/Pool → admin signer mismatch or seed mismatch in the init script.
- Priority-fee auction failures > 30% → manually bump priority fee in swarm config.

---

## §9 — Rollback

Devnet rollback is trivial because nothing on mainnet is affected:

**Fast path** (restore swarm to "no-op" mode):

```bash
# Unset the devnet RPC override; swarm falls back to mainnet default where AO v2
# staking is already known-broken (error 101). Agents stop attempting staking IXs.
unset SOLANA_RPC
unset CCM_MINT
# Restart swarm containers
```

**Full teardown** (if the devnet deploy is conclusively broken and you want to redeploy cleanly):

```bash
# Close the devnet program (recovers rent to upgrade authority)
solana program close BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9 \
    --keypair ~/.config/solana/devnet-rails-admin.json \
    --recipient $(solana-keygen pubkey ~/.config/solana/devnet-rails-admin.json) \
    --url devnet

# Fix whatever was wrong, rebuild (§3), redeploy (§4), re-init (§5)
```

**Rollback drill** (required for §10 sign-off): time the fast-path sequence. Target < 60s from "issue detected" to "swarm idle on mainnet defaults."

---

## §10 — Sign-Off Checklist

Do not declare devnet complete until every box is checked:

- [ ] §0: LiteSVM fee-path test green (14/14 tests including the new TransferFeeConfig case)
- [ ] §1: Devnet CCM mint created with 50 bps `TransferFeeConfig`; admin ATA holds ≥ 10M CCM
- [ ] §2: Program keypair pubkey matches `BdSv824...SZy9`; admin keypair funded with ≥ 10 SOL
- [ ] §3: `solana-verify get-executable-hash` on `target/verifiable/wzrd_rails.so` returns `314190d7...`
- [ ] §4: `solana program show` confirms deploy at `BdSv8...`, `Executable: true`, correct upgrade authority
- [ ] §5: Config PDA, Pool PDA (id=0), StakeVault PDA, RewardVault PDA all exist on-chain with expected data lengths
- [ ] §6: `swarm/stake.py` devnet-pointed; `_fetch_config_exists` preflight returns True; test agents funded with devnet CCM
- [ ] §7.2: Single-agent `attempt_stake` succeeds; `user_stake.amount = 995_000_000` for a `1_000_000_000` stake (fee path verified)
- [ ] §7.3: Full round-trip on pool_id=1 (short-lock) passes — stake → claim → unstake → re-stake
- [ ] §7.4: 10-agent concurrent load passes with ≤ 1 correctness failure
- [ ] §9: Rollback drill executed in < 60s; swarm recovers cleanly
- [ ] PR #76 comment updated with devnet program show output + test harness logs
- [ ] MEMORY.md updated with `DEVNET_CCM_MINT`, devnet deploy tx signature, observed failure modes (if any)

---

## §11 — Risks + Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Verifiable build hash mismatch | Low | High | §3 hash gate; does not proceed without match. Root-cause Docker digest or cargo cache. |
| Program keypair mismatch → new ID deployed | Medium | High | §2.2 explicit pubkey verification before `solana program deploy`. |
| TransferFeeConfig not set on §1 CCM mint | Medium | Medium | §7.2 asserts post-fee `amount = 995_000_000`. Fails loudly if fee path is not exercised. |
| `initialize_pool` called twice (Anchor `init` reverts) | Low | Low | Script checks Pool PDA existence first and skips if it's there. Re-running the script is safe. |
| Priority fee auction losses during §7.4 | High | Low | Normal on devnet under simultaneous submit; not a correctness issue. If it becomes a confidence blocker, add jitter to the harness. |
| Devnet RPC rate limits | Medium | Low | Use a dedicated devnet RPC (Helius has a free tier) instead of `api.devnet.solana.com` if hit. |
| Admin CCM ATA not initialized before §5 | Medium | Medium | §1.2 explicitly creates it; §5's `initialize_config` only stores the pubkey, doesn't validate — so misconfigured ATA only surfaces at first `fund_reward_pool` call. Verify with `spl-token account-info` in §5.3. |
| `swarm/stake.py` `CCM_MINT` hardcoded to mainnet | High | Medium | §6.1 Option A: add `CCM_MINT` env override. One-line change; should merge as follow-up. |

---

## §12 — After Devnet Sign-Off

Once §10 is fully checked:

1. **External review of PR #76** — share devnet deploy tx signature + hash evidence + `§7.2`/`§7.4` test logs. Give reviewers ≥ 1 week.
2. **Mainnet Runbook v2** — same structure as v1.1 but with Squads proposal workflow for admin + upgrade authority transfer. Incorporate any failure modes observed during devnet §7.
3. **Economic parameterization** (angle B) — now that the fee path is proven working, decide initial `reward_rate`, `lock_duration`, and whether to seed the reward pool from treasury CCM at launch or later.
4. **Velocity → reward_rate feedback loop** (angle D) — design sketch; connect the data engine's aggregate velocity signal to `set_reward_rate` keeper cadence.
5. **Compensation event** (angle C) — requires off-chain merkle tree builder (L-01 from PR #76) to inflate leaf amounts by `fee_bps / (10000 - fee_bps)` so post-fee net equals the intended gross compensation.
6. **Swarm full migration on mainnet** — after Squads proposal passes, flip `SOLANA_RPC` to mainnet for the swarm. Thundering-herd mitigations in `swarm/stake.py` (`_fetch_config_exists` preflight + env-driven config) already handle the transition.

---

## Appendix — Quick Reference Commands

```bash
# Build + hash verify
anchor build --verifiable --program-name wzrd_rails
solana-verify get-executable-hash target/verifiable/wzrd_rails.so

# Deploy
solana program deploy target/verifiable/wzrd_rails.so \
    --program-id target/deploy/wzrd_rails-keypair.json \
    --keypair ~/.config/solana/devnet-rails-admin.json \
    --upgrade-authority ~/.config/solana/devnet-rails-admin.json \
    --url https://api.devnet.solana.com

# Init state
python3 scripts/devnet-init-rails.py \
    --rpc https://api.devnet.solana.com \
    --admin ~/.config/solana/devnet-rails-admin.json \
    --ccm-mint $DEVNET_CCM

# Inspect
solana program show BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9 --url devnet
solana logs BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9 --url devnet

# Close (rollback)
solana program close BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9 \
    --keypair ~/.config/solana/devnet-rails-admin.json \
    --recipient $(solana-keygen pubkey ~/.config/solana/devnet-rails-admin.json) \
    --url devnet

# Tests
cargo test -p wzrd-rails --test core_loop            # LiteSVM
cd /home/twzrd/wzrd-final/agents/swarm-runner && python3 -m pytest tests/test_stake.py -v  # swarm
```

---

**Changes from v1.0**:
- §0 added: LiteSVM fee-path pre-flight (previously missing; Token-2022 fee path was unexercised)
- §1 added: explicit devnet CCM mint provisioning with TransferFeeConfig (previously implicit)
- §2: removed incorrect "Rust 1.84+" requirement (wzrd-rails uses 1.91.1 inside Docker)
- §3: correct build command is `anchor build --verifiable --program-name wzrd_rails` (was `cargo build-bpf`); correct hash tool is `solana-verify get-executable-hash` (was `sha256sum`); binary path is `target/verifiable/` (was `target/deploy/`)
- §4: deploy command uses `target/deploy/wzrd_rails-keypair.json` as `--program-id` (was deployer keypair, which would have created a new program ID)
- §5: rewritten to use `scripts/devnet-init-rails.py` (committed, idempotent) instead of non-existent `solana program invoke` CLI; correct `initialize_config` signature (ccm_mint + treasury_ata), not (reward_rate + lock + min_stake)
- §6: env-var flip (`SOLANA_RPC`, `CCM_MINT`) instead of editing a `swarm/config.py` that doesn't exist; devnet uses the SAME program ID as mainnet (deterministic from keypair)
- §7: concrete Python snippets that match the real `attempt_stake(http, rpc_url, keypair, ccm_amount, agent_id, ...)` async signature
- §9: rollback = `unset SOLANA_RPC` (not `sed` on a nonexistent config file)
- §11: risk matrix updated with Token-2022 fee config, program keypair mismatch, and `CCM_MINT` hardcoding items
