# Attention Oracle Program

Solana on-chain program for the [Liquid Attention Protocol](https://twzrd.xyz). Deployed at [`GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`](https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop).

Built with [Anchor 0.32.1](https://www.anchor-lang.com/) on Solana.

## What it does

Permissionless attention markets on Solana. Users deposit USDC, receive vLOFI, accrue attention multipliers, and claim CCM (Token-2022 with transfer fee) based on merkle proof distributions.

**Token mints:**

| Token | Mint | Standard |
|-------|------|----------|
| CCM | `Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM` | Token-2022 (50 BPS transfer fee) |
| vLOFI | `E9Kt33axpCy3ve2PCY9BSrbPhcR9wdDsWQECAahzw2dS` | Standard SPL |

## Build

```bash
anchor build --verifiable --program-name token_2022
```

The default features include `phase2` (channel staking, prediction markets, strategy vault, price feeds).

## Verify

```bash
# Get on-chain hash
solana-verify get-program-hash GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

# Build and compare
solana-verify get-executable-hash target/verifiable/token_2022.so

# Full verification from repo
solana-verify verify-from-repo \
  https://github.com/twzrd-sol/attention-oracle-program \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --library-name token_2022
```

## Test

```bash
cargo test -p attention-oracle-token-2022
```

Tests use [LiteSVM](https://github.com/LiteSVM/litesvm) for fast local execution.

## Upgrade Authority

Squads V4 multisig (3-of-5): [`BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ`](https://solscan.io/account/BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ)

## License

MIT
