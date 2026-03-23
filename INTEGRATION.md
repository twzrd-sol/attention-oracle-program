# Integration Guide

Technical reference for integrating with the Attention Oracle program on Solana mainnet.

## Program Overview

The Attention Oracle (`GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`) is a Pinocchio-based Solana program that manages:

- **Deposits and settlement** (USDC in, vLOFI minted)
- **Merkle-based reward claims** (CCM distribution via cumulative proofs)
- **Transfer fee harvesting** (Token-2022 withheld fees to treasury)
- **Price feeds** (on-chain oracle updates)

## Key Instructions

| Instruction | Discriminator | Description |
|-------------|--------------|-------------|
| `deposit_market` | `d435bac193358f7b` | Deposit USDC into a market vault, receive vLOFI |
| `settle_market` | `c1995fd8a60690d9` | Settle a matured position, burn vLOFI, return USDC |
| `claim_global_v2` | `f82caa6531aa8c7e` | Claim CCM rewards with merkle proof |
| `claim_global_sponsored_v2` | `59548450 8b5c5e04` | Sponsored (gasless) claim via relay |
| `update_attention` | `7bf77586d06b6c32` | Update attention scores (oracle authority only) |
| `publish_global_root` | — | Publish new merkle root for claims |
| `harvest_fees` | — | Sweep withheld Token-2022 transfer fees |

Discriminators are `SHA-256("global:<instruction_name>")[..8]`.

## Token Mints

| Token | Mint | Standard | Transfer Fee |
|-------|------|----------|-------------|
| CCM | `Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM` | Token-2022 | 50 BPS |
| vLOFI | `E9Kt33axpCy3ve2PCY9BSrbPhcR9wdDsWQECAahzw2dS` | SPL Token | None |

## PDA Seeds

```
ProtocolState:     ["protocol_state"]
MarketVault:       ["market_vault", protocol_state, market_id_le_u64]
UserPosition:      ["market_position", market_vault, user_pubkey]
GlobalRootConfig:  ["global_root", ccm_mint]
ClaimState:        ["claim_global", ccm_mint, user_pubkey]
```

## Reward Claims (Cumulative Merkle)

Claims use a cumulative delta model:

1. Off-chain publisher computes per-user cumulative totals
2. Publisher submits merkle root on-chain via `publish_global_root`
3. User submits proof for `(wallet, cumulative_total)` via `claim_global_v2`
4. Program pays out the delta (new total minus previously claimed)

Gasless claims are supported via `claim_global_sponsored_v2` where a relay cosigns as fee payer.

## Token-2022 Transfer Fees

CCM uses the Token-2022 Transfer Fee Extension (50 BPS). Fees are:

1. Withheld in recipient token accounts on every transfer
2. Swept to treasury via `harvest_fees` (permissionless, batched)

Wallets and exchanges should use `transfer_checked` to handle the fee correctly.

## References

- [VERIFY.md](VERIFY.md) — Build verification
- [DEPLOYMENTS.md](DEPLOYMENTS.md) — Program IDs and deployment history
- [UPGRADE_AUTHORITY.md](UPGRADE_AUTHORITY.md) — Governance and upgrade process
- [SECURITY.md](SECURITY.md) — Vulnerability disclosure
