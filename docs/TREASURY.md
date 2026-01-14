# Treasury Architecture

This document explains treasury controls, withdrawal mechanisms, and the governance roadmap.

---

## Overview

The TWZRD treasury holds CCM tokens for:
- Liquidity provision (DEX LP seeding)
- Creator/partner payments
- Protocol operational expenses
- Community incentives

**Key point:** The treasury is PDA-controlled, not a hot wallet.

---

## Treasury Control Model

```
┌─────────────────────────────────────────────────────────────────┐
│  Protocol State PDA                                              │
│  Seeds: [b"protocol", mint.key()]                               │
│  ├── admin: Pubkey (current authority)                          │
│  ├── treasury: Pubkey (treasury PDA)                            │
│  └── mint: Pubkey (CCM token)                                   │
└─────────────────────────────────────────────────────────────────┘
                           │
                           │ owns (PDA authority)
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│  Treasury ATA                                                    │
│  Associated token account holding CCM reserves                   │
│  Can ONLY be moved via program instructions                      │
└─────────────────────────────────────────────────────────────────┘
```

**No private key can directly transfer from treasury.** Funds move only through the `admin_withdraw` instruction, which enforces rate limits and emits audit events.

---

## Withdrawal Rate Limits

| Limit Type | Amount | Notes |
|------------|--------|-------|
| Per-transaction | 50M CCM | Single tx cap |
| Per-day | 100M CCM | ~5% of 2B supply |
| Minimum drain time | ~20 days | At max continuous rate |

### Why Rate Limits?

Rate limits are a **circuit breaker**, not absolute security:

1. **Detection window:** If admin key is compromised, 20 days provides time to:
   - Detect unusual withdrawal patterns
   - Coordinate emergency response
   - Migrate to new admin via `update_admin`

2. **Operational flexibility:** Legitimate operations (LP seeding, payments) can proceed without governance overhead during early phase

3. **Audit trail:** Every withdrawal emits `TreasuryWithdrawn` event with:
   - Admin address
   - Destination account
   - Amount
   - Daily running total
   - Cumulative total
   - Timestamp

---

## Withdrawal Tracking

The `WithdrawTracker` PDA maintains:

```rust
pub struct WithdrawTracker {
    pub version: u8,
    pub mint: Pubkey,
    pub day_start: i64,        // UTC day boundary
    pub withdrawn_today: u64,   // Resets daily
    pub total_withdrawn: u64,   // All-time cumulative
    pub last_withdraw_at: i64,  // Last withdrawal timestamp
    pub bump: u8,
}
```

Daily limits reset at UTC midnight automatically.

---

## Governance Roadmap

| Phase | Authority | Timeline | Controls |
|-------|-----------|----------|----------|
| **Phase 1** (Current) | Single admin key | Launch → +3 months | Rate limits, event monitoring |
| **Phase 2** | Multisig (3-of-5) | +3 months | Multiple signers required |
| **Phase 3** | DAO + Timelock | Post token distribution | Community governance, execution delay |

### Migration Path

```
admin_withdraw (current)
        │
        │ update_admin() to multisig
        ▼
Squads/Realms Multisig
        │
        │ Integrate DAO voting
        ▼
DAO Governance + Timelock
```

The `update_admin` instruction allows transferring authority to any pubkey, including multisig program PDAs.

---

## Monitoring

Recommended monitoring for treasury operations:

1. **On-chain:** Subscribe to `TreasuryWithdrawn` events
2. **Alerting thresholds:**
   - Any withdrawal > 10M CCM
   - Daily total approaching limit
   - Multiple withdrawals in short window
3. **Dashboard:** Track `withdraw_tracker.total_withdrawn` vs treasury balance

---

## FAQ

### "Can the admin drain the treasury?"

Technically yes, but:
- It would take ~20 days at maximum rate
- Every withdrawal is on-chain, auditable
- Monitoring can detect and alert immediately
- Admin key will migrate to multisig before significant operations

### "Why have admin withdrawal at all?"

Operational necessity during bootstrap:
- LP seeding requires moving tokens to DEX pools
- Creator payments need disbursement
- Can't wait for DAO votes on every operational tx

The rate limits bound the damage from compromise while enabling operations.

### "What happens if admin key is compromised?"

1. Rate limits cap damage to 100M CCM/day
2. On-chain events provide immediate visibility
3. Team can coordinate response within 20-day window
4. `update_admin` to new key stops the attacker

### "When will this move to DAO control?"

After token distribution is complete and community governance is established. The technical migration is straightforward (`update_admin` to DAO program PDA).

---

## References

- [admin.rs](/programs/token_2022/src/instructions/admin.rs) - Implementation
- [state.rs](/programs/token_2022/src/state.rs) - WithdrawTracker struct
- [events.rs](/programs/token_2022/src/events.rs) - TreasuryWithdrawn event
