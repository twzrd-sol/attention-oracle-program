# Transfer Fee Capture (Public Spec)

This document describes how transfer-related fees are handled for a Token-2022 mint configured
to use this program.
It is **not** an operational runbook and intentionally omits private endpoints, key management
details, and internal procedures.

## Summary

- **Transfer fees are native Token-2022.** A configured mint can use Token-2022's Transfer Fee Extension.
- **Fee capture happens via harvesting withheld fees.** The `token_2022` program exposes an
  admin-only instruction that sweeps withheld fees from user token accounts into the protocol
  treasury using Token-2022 CPI.

## How Token-2022 transfer fees work

When a Token-2022 mint has the Transfer Fee Extension configured, the Token-2022 program:

1. Calculates the fee on each transfer (based on mint configuration).
2. Withholds the fee amount in the recipient token account (not immediately transferred to a
   treasury).
3. Allows an authorized party to later withdraw ("harvest") withheld fees via Token-2022
   instructions.

## Protocol fee harvesting

The `token_2022` program provides a harvesting instruction (see `programs/token_2022`) that:

- Accepts a bounded list of source token accounts as `remaining_accounts`.
- Calls Token-2022's `withdraw_withheld_tokens_from_accounts` CPI to sweep any withheld fees from
  those accounts into the protocol treasury token account.
- Emits an on-chain event indicating the amount harvested.

## Non-goals / excluded

- This document does **not** specify how source accounts are selected for harvesting.
- This document does **not** describe deployment steps or key locations.
- This document does **not** override the on-chain program logic.

## References

- `INTEGRATION.md` for required extra accounts when sending transfers.
- `docs/TREASURY.md` for treasury behavior and what can move funds.
- `VERIFY.md` for build verification guidance.
- `DEPLOYMENTS.md` for program IDs and upgrade policy.
