# wzrd-rails → wzrd-markets Upgrade Runbook

**Program ID (mutable):** `BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9`
**Current binary:** wzrd-rails (MasterChef CCM staking + listen payout)
**Target binary:** wzrd-markets (prediction markets CPMM)
**Date assessed:** 2026-06-23

---

## On-Chain State Audit Summary

| Account | Address | Lamports | Notes |
|---------|---------|---------|-------|
| Config PDA | `7pwUU1hv...` | ~1,100 | Orphaned after upgrade |
| StakePool PDA | `6oQDChd9...` | ~2,200 | Orphaned after upgrade |
| UserStake (agent-058) | `DZZCvBvoLPc3eJM8jCS6fRW3s52GZZQs7UcnEqZgacE7` | ~2,000 | 449 CCM staked — UNLOCKED |
| UserStake (seed-cohort) | `HnR59DAGJNiW4q8ZHzgQzWYJcMYPiWrFhb733Tcqxb34` | ~2,000 | 3.1M CCM staked — UNLOCKED |
| stake_vault | `H8uqT29s3Kc9JLR3s6G2L3ZyF9avz2CJKfhPK1EbcmXr` | — | 3,116,139.77 CCM |
| reward_vault | `4HnYVcAs...` | — | 99.5 CCM |

Both UserStake positions belong to **internal WZRD wallets** (not external users).
Both locks have expired as of 2026-06-23.

**Accepted write-offs (internal wallets only):**
- 3.1M CCM in stake_vault — seed-cohort keypair unavailable; treated as permanent stake
- 99.5 CCM in reward_vault — negligible
- ~0.007 SOL across orphaned PDAs — negligible

---

## Pre-Upgrade Checklist

- [ ] **Optional**: Drain agent-058 position (449 CCM)
  ```bash
  # Keypair: /home/twzrd/security/swarm-keys/agent-058.json
  # Requires read access to that file (root-owned, enter as sudo or via founder terminal)
  # See: ops/scripts/smoke-rails-canary.sh for account addresses
  anchor idl type /home/twzrd/attention-oracle-program/target/idl/wzrd_rails.json
  # Call unstake with pool_id=0, wallet=agent-058.json
  ```
- [ ] Ensure upgrade authority wallet has **≥ 5.1 SOL liquid** (5.089 SOL buffer + fees)
  - Buffer wallet is returned after upgrade completes
  - Program data account: `~731,120 bytes → ~5.089 SOL buffer`
- [ ] Build release binary:
  ```bash
  cd /home/twzrd/attention-oracle-program
  anchor build --program-name wzrd_markets -- --release
  # Binary: target/deploy/wzrd_markets.so
  ```
- [ ] Verify binary size is within buffer capacity:
  ```bash
  ls -la target/deploy/wzrd_markets.so
  # Must be ≤ 731,120 bytes to fit current program data account
  # If larger, additional SOL is needed for the expanded buffer
  ```

---

## Upgrade Command (requires founder wallet + SOL)

```bash
# Step 1: Write buffer account
solana program write-buffer \
  target/deploy/wzrd_markets.so \
  --keypair /path/to/upgrade-authority.json \
  --url mainnet-beta

# Note the buffer address printed above, then:

# Step 2: Upgrade program
solana program upgrade \
  <BUFFER_ADDRESS> \
  BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9 \
  --keypair /path/to/upgrade-authority.json \
  --url mainnet-beta

# Buffer SOL is returned to the payer automatically on success.
```

---

## Post-Upgrade Verification

```bash
# Confirm new program is active
solana program show BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9

# Smoke test: initialize_markets_config should work with canonical USDC
# anchor test --skip-local-validator (requires RPC config)
```

---

## What Changes After Upgrade

| Before | After |
|--------|-------|
| wzrd-rails instructions (stake/unstake/claim/listen payout) | wzrd-markets instructions (create/resolve/deposit/redeem) |
| StakePool/UserStake accounts (owned by program, no close instruction) | Orphaned — ~0.007 SOL permanently locked |
| stake_vault CCM (3.1M + 449 CCM) | Permanently locked (no instruction to drain) |
| reward_vault CCM (99.5 CCM) | Permanently locked |

All orphaned amounts are internal WZRD wallets only — no external user funds at risk.
