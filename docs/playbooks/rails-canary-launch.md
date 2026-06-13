# Rails Canary Launch Notes

Date: 2026-05-05

This document is a release-note draft and approval checklist for re-opening the
single-agent `wzrd-rails` canary after the swarm sell-pressure incident. It is
not approval to mutate mainnet, flip Doppler, restart containers, or expand the
allowlist.

## Current State

- The legacy AO staking sink is unavailable for channel staking. The deployed
  immutable AO program returns `InstructionFallbackNotFound` / `Custom(101)` for
  the channel staking instruction family.
- Swarm REDUCE is disabled in production by config and by the fail-closed
  strategy gate merged upstream (PR #335).
- The intended productive sink is now `wzrd-rails`, gated to a one-agent canary:
  `WZRD_USE_RAILS=true` and `WZRD_RAILS_AGENT_ALLOWLIST=58`.
- Do not set `WZRD_USE_RAILS=true` without the allowlist in the same deploy.

## Live Rails Facts

Read-only discovery on 2026-05-05 found:

| Surface | Value |
| --- | --- |
| Program | `BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9` |
| Config PDA | `7pwUU1hv3hCNNTAPmDyMRCeKoMPEz3k5cH1PTbWDNQR6` |
| Config admin | `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD` |
| Upgrade authority | `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD` |
| Pool 0 | `6oQDChd9wJv4CJdPT8zsBwPmYT2jUmogetVP9me6u5Vf` |
| Stake vault | `H8uqT29s3Kc9JLR3s6G2L3ZyF9avz2CJKfhPK1EbcmXr` |
| Reward vault | `4HnYVcAs91Az5JYt4p5DaJzFvSxzvjpFnXsgzqNejBKh` |
| Agent 58 wallet | `E4F4MY6Fm3hzq3xmrDB5nnrTxXgKPw19wviaZxVSMwQD` |
| Agent 58 CCM ATA | `8i1TgsTPQRpmos9qoATwzRyXpADTU3qejVHBWqgKb72x` |
| Agent 58 UserStake | `HnR59DAGJNiW4q8ZHzgQzWYJcMYPiWrFhb733Tcqxb34` |

Pool 0 currently has `total_staked=0`, `reward_rate_per_slot=1000`, and an
empty reward vault. The swarm preflight correctly refuses to stake into that
state because active emissions before a seed cohort would make the first staker
capture the initial reward window.

## Required Sequence

### 0. Fresh Read-Only Proof

Before any mutation, re-decode:

- Config admin and program upgrade authority.
- Pool 0 `total_staked`, `reward_rate_per_slot`, `last_update_slot`, and lock
  duration.
- Stake vault and reward vault Token-2022 balances.
- Agent 58 CCM balance and `UserStake` account existence.
- Running swarm flags: `WZRD_USE_RAILS` and `WZRD_RAILS_AGENT_ALLOWLIST`.

Stop if any value differs from the expected canary shape and write the new
state down before proceeding.

### 1. Reset Reward Rate To Zero

Use `scripts/set-reward-rate.ts` from a clean `attention-oracle-program`
checkout. It defaults to dry-run simulation.

Dry-run example:

```bash
CLUSTER=mainnet-beta \
RPC_URL="$SOLANA_RPC_URL" \
KEYPAIR=/path/to/2pHj-admin.json \
POOL_ID=0 \
NEW_RATE=0 \
npx tsx scripts/set-reward-rate.ts
```

Broadcast requires a separate approval and non-interactive confirmation:

```bash
CLUSTER=mainnet-beta \
RPC_URL="$SOLANA_RPC_URL" \
KEYPAIR=/path/to/2pHj-admin.json \
POOL_ID=0 \
NEW_RATE=0 \
BROADCAST=1 \
I_UNDERSTAND_MAINNET=1 \
CONFIRM_BROADCAST=mainnet-beta:0:0 \
npx tsx scripts/set-reward-rate.ts
```

Postcondition: Pool 0 has `reward_rate_per_slot=0`.

### 2. Seed Pool 0

Seed with agent 58 only after rate is zero and the canary is still paused.
Token-2022 transfer fees mean the credited stake is the post-fee amount, not
the requested raw transfer amount. The prior one-CCM canary target used
`1_005_025_125` base units requested to credit about `1_000_000_000` base
units after the 50 bps fee.

Do not pair seed staking with a canary flip. Prove the seed account and pool
state first.

Postcondition: Pool 0 has `total_staked > 0`, and agent 58's `UserStake` exists.

### 3. Fund Reward Vault

Use `scripts/fund-reward-pool.ts`. It is permissionless at the program layer but
still moves CCM, so treat it as a mainnet funding operation requiring explicit
approval.

At the current `reward_rate_per_slot=1000`, 30 days of backing is:

- 216,000 slots/day: `6.48 CCM`
- 432,000 slots/day: `12.96 CCM`

Use an operational buffer instead of exact math. The recommended starting range
is `100` to `1,000 CCM`, chosen by the signer/funding owner before broadcast.

Dry-run example for 100 CCM at 9 decimals:

```bash
CLUSTER=mainnet-beta \
RPC_URL="$SOLANA_RPC_URL" \
KEYPAIR=/path/to/funder.json \
POOL_ID=0 \
AMOUNT_BASE_UNITS=100000000000 \
npx tsx scripts/fund-reward-pool.ts
```

Broadcast requires a separate approval:

```bash
CLUSTER=mainnet-beta \
RPC_URL="$SOLANA_RPC_URL" \
KEYPAIR=/path/to/funder.json \
POOL_ID=0 \
AMOUNT_BASE_UNITS=100000000000 \
BROADCAST=1 \
I_UNDERSTAND_MAINNET=1 \
CONFIRM_BROADCAST=fund:mainnet-beta:0:100000000000 \
npx tsx scripts/fund-reward-pool.ts
```

If the funder uses a non-ATA CCM account, pass:

```bash
FUNDER_CCM_ACCOUNT=<token-account-pubkey>
```

Postcondition: reward vault balance increases by a positive post-fee amount.

### 4. Restore The Chosen Reward Rate

After the seed exists and the reward vault is funded, use
`scripts/set-reward-rate.ts` again to set the target rate.

Recommended canary target: keep the current policy value `1000` base units per
slot unless a separate economics decision changes it.

Postcondition: `check_rails_preflight()` returns ready for pool 0.

### 5. Flip Agent 58 Canary

Only after the preflight is green:

- Set `WZRD_USE_RAILS=true`.
- Preserve `WZRD_RAILS_AGENT_ALLOWLIST=58`.
- Recreate only the swarm container.
- Watch agent 58 for 24 hours before any allowlist expansion.

Do not globally enable rails by omitting the allowlist.

## Rollback

Off-chain rollback is:

- `WZRD_USE_RAILS=false`
- recreate only the swarm container

On-chain operations are state-preserving. Do not try to "rollback" the seed
stake by unstaking unless a separate incident plan proves that is the safest
action.

## Launch Blockers

- `2pHj...` is both Config admin and upgrade authority. That is acceptable for a
  canary only if acknowledged; governance rotation remains a separate task.
- The helper scripts require actual signer key custody. This document and PR do
  not grant that custody.
- The AO deployed-binary truth audit found documentation drift outside this
  rails launch. Do not rely on stale `CLAUDE.md` runtime claims when operating
  this canary.
