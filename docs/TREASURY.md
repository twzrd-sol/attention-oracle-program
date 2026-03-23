# Treasury Architecture

How the protocol treasury is represented on-chain and what instructions can move funds.

## Overview

The protocol treasury holds token reserves used for reward payouts.
The treasury token account is owned by a Program Derived Address (PDA), not by a hot wallet.

## Treasury Accounts

The treasury is an Associated Token Account (ATA) owned by the Protocol State PDA.

### Derivation

```
Protocol State PDA = find_program_address(["protocol_state"], program_id)

Treasury ATA = get_associated_token_address(protocol_state_pda, mint)
```

### Account Relationship

```
┌─────────────────────────────────────────────────────────────────┐
│  Protocol State PDA                                              │
│  Seeds: [b"protocol_state"]                                     │
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
- Standard ATA derivation makes the treasury address deterministic and verifiable

## What Can Move Funds?

### Outflows (treasury to user)

Treasury funds move out only through merkle claim instructions:

- `claim_global_v2` (user-submitted claim with merkle proof)
- `claim_global_sponsored_v2` (relay-submitted claim on behalf of user)

### Inflows (user accounts to treasury)

Token-2022 transfer fees withheld in user accounts are harvested via:

- `harvest_fees` (sweeps withheld fees from a bounded list of token accounts into treasury)

### No Admin Withdrawal

There is **no `admin_withdraw` instruction**. Treasury outflows occur via claims only.
The program is upgradeable (see [UPGRADE_AUTHORITY.md](/UPGRADE_AUTHORITY.md)); the upgrade authority is the primary governance surface.

## Incident Response

- `set_paused` halts claims during incident response
- Publisher key rotatable by admin (`update_protocol_state`)

## References

- [SECURITY.md](/SECURITY.md) — Security policy and reporting
- [INTEGRATION.md](/INTEGRATION.md) — Integration guide
- [VERIFY.md](/VERIFY.md) — Build verification
