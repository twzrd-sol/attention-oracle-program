# Integration Guide (Token-2022 Transfer Hook)

This document provides a high-level integration guide for custodians, exchanges, and wallets that interact with TWZRD's Token-2022 mint and transfer hook.

## Summary

- TWZRD uses Token-2022 extensions and a transfer hook program (`ccm_hook`).
- Transfers require extra account metas supplied by the caller.
- Missing required accounts will cause the transfer to fail.

## Required support

- Token-2022 support (token program v2022).
- Ability to append extra account metas to transfer instructions.

## Transfer hook flow (high level)

1. Fetch the ExtraAccountMetaList for the mint (owned by `ccm_hook`).
2. Append the required extra account metas to the transfer instruction.
3. Submit the transfer through the Token-2022 program.

If you do not supply the extra accounts, the transfer will be rejected by the hook.

## Where to find required accounts

- The ExtraAccountMetaList PDA is derived by the hook program and mint.
- For program addresses and seeds, inspect `programs/ccm_hook` and the on-chain IDL.

## Common integration issues

- Missing extra account metas (transfer fails).
- Using the SPL Token (v1) program instead of Token-2022.
- Wallets/exchanges that do not support transfer hooks.

## References

- `VERIFY.md` for build verification.
- `DEPLOYMENTS.md` for program IDs.
