# Treasury Architecture

This document explains how the protocol treasury is represented on-chain and what instructions can move funds.

---

## Overview

The protocol treasury holds token reserves used for cumulative rewards payouts.

Key property: the treasury token account is owned by a Program Derived Address (PDA), not by a hot wallet.

## Treasury accounts

The treasury is **not** a standalone PDA with seeds `["treasury"]`. It is an Associated Token Account (ATA)
owned by the Protocol State PDA.

### Derivation

```
Protocol State PDA = find_program_address(["protocol", mint], program_id)

Treasury ATA = get_associated_token_address(protocol_state_pda, mint)
```

### Account Relationship

```
┌─────────────────────────────────────────────────────────────────┐
│  Protocol State PDA                                              │
│  Seeds: [b"protocol", mint.key()]                               │
│  ├── admin: Pubkey (current authority)                          │
│  ├── publisher: Pubkey (root publisher)                         │
│  ├── paused: bool (emergency halt)                              │
│  └── mint: Pubkey (Token-2022 mint)                             │
└─────────────────────────────────────────────────────────────────┘
                           │
                           │ owns (PDA is the ATA authority)
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│  Treasury ATA                                                    │
│  Derived: get_associated_token_address(protocol_state_pda, mint) │
│  Token Account holding protocol reserves                         │
└─────────────────────────────────────────────────────────────────┘
```

This design ensures:
- No hot wallet holds the treasury private key
- Only the program can sign for treasury outflows (via PDA seeds)
- Standard ATA derivation makes treasury address deterministic and verifiable

## What can move funds?

### Outflows (treasury -> user/creator)

Treasury funds can move out only through cumulative claim instructions:

- `claim_cumulative` (user-submitted claim)
- `claim_cumulative_sponsored` (relayer-submitted claim on behalf of user)

If a channel config has a `creator_fee_bps` set, the claim instruction splits the delta:

- User receives `delta - creator_cut`
- Creator receives `creator_cut` to `creator_wallet`

### Inflows (user accounts -> treasury)

The protocol can capture **native Token-2022 transfer fees** that were withheld in user token accounts by calling:

- `harvest_fees`

This instruction performs a Token-2022 CPI to withdraw withheld fees from a bounded list of source token accounts
(`remaining_accounts`) into the treasury token account.

### No admin withdrawal

There is **no `admin_withdraw` instruction** in the current public program interface.
Operationally, this means there is no dedicated on-chain "treasury drain" path besides claims.

Important: the program itself is upgradeable (see `DEPLOYMENTS.md`), and an upgrade could introduce new behavior.

## Incident response

- `set_paused` can halt claims during incident response.
- `update_publisher` can rotate the publisher key that is allowed to publish cumulative roots.

## Legacy notes

Some legacy state/event definitions may still exist in source for historical decoding, but they are not part of the
current operational treasury flow (e.g., `WithdrawTracker`, `TreasuryWithdrawn`).

## References

- `SECURITY.md` - security policy and reporting
- `docs/specs/transfer-fee-capture.md` - Token-2022 fee model summary
- `programs/token_2022/src/instructions/cumulative.rs` - claim logic
- `programs/token_2022/src/instructions/governance.rs` - fee harvesting CPI
