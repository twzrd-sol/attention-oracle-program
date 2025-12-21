# Transfer Fee Capture (Public Spec)

This document describes the protocol-level intent of "transfer fee capture" for the TWZRD Token-2022 program. It is **not** an operational runbook and intentionally omits operational steps, private endpoints, or key management details.

## Purpose

Transfer-fee capture exists to route a predefined portion of transfer-related fees into protocol-controlled accounts for treasury, staking, or other program-defined purposes.

## High-level behavior

- Transfers are subject to Token-2022 extensions and program-defined rules.
- Fee capture is deterministic and enforced by the on-chain program.
- The program expects required accounts to be supplied by the caller and will reject transfers missing required accounts.

## Invariants

- Fee capture logic is permissionless and applies uniformly to all transfers that meet the program's criteria.
- The transfer hook does not rely on off-chain secrets.
- Fee destinations are on-chain accounts controlled by the program.

## Non-goals / excluded

- This document does **not** describe operational procedures or deployment steps.
- This document does **not** list internal endpoints, run commands, or key locations.
- This document does **not** override the on-chain program logic.

## References

- See `INTEGRATION.md` for the required accounts and common integration flows.
- See `VERIFY.md` for build verification guidance.
- See `DEPLOYMENTS.md` for program IDs and upgrade policy.
