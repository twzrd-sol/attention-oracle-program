# Integration Guide (Token-2022 Mint + Claims)

This document provides high-level integration notes for parties that interact with a Token-2022 mint
configured to use the `token_2022` program in this repository.

## TL;DR

- The token mint may use **Token-2022 Transfer Fee Extension** (native fees).
- The on-chain `token_2022` program provides:
  - cumulative Merkle claims (`claim_cumulative`, `claim_cumulative_sponsored`)
  - fee harvesting of withheld transfer fees (`harvest_fees`)
- A transfer-hook program is **optional** and only required if your mint is configured with the
  Transfer Hook Extension.

You can verify mint extensions on mainnet:

```bash
spl-token display --program-2022 -u mainnet-beta -v <MINT_ADDRESS>
```

## Token-2022 transfer fees (native)

When the Transfer Fee Extension is enabled on a Token-2022 mint, the Token-2022 program:

1. Withholds transfer fees in recipient token accounts.
2. Allows an authorized withdrawal authority to sweep (harvest) withheld fees later.

In this system, the `token_2022` program provides `harvest_fees` to sweep withheld fees into the
protocol treasury.

## Reward claims (cumulative Merkle)

Claims are delta-based cumulative claims:

- The publisher publishes a cumulative root for a channel + root sequence.
- A user (or relayer) submits a Merkle proof for `(wallet, cumulative_total)`.
- The program pays out the delta from the protocol treasury.

If a channel config has a `creator_fee_bps`, the claim delta is split between:

- user payout
- creator payout (`creator_wallet`)

## Transfer hook (optional)

A transfer hook is **not required** unless your Token-2022 mint is explicitly configured with the
Transfer Hook Extension. If you enable a transfer hook:

- Your mint must reference the hook program.
- Wallets/exchanges may need to append extra account metas to transfers.

## References

- `docs/specs/transfer-fee-capture.md` - fee model summary
- `docs/TREASURY.md` - what can move funds
- `VERIFY.md` - reproducible build + verification
- `DEPLOYMENTS.md` / `docs/LIVE_STATUS.md` - program IDs + on-chain status
