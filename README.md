# Attention Oracle Program

On-chain Solana program for the Liquid Attention Protocol — verifiable, engagement-based reward distribution with AI model velocity signals.

**Program ID:** `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`

## What This Does

- **Deposit & Yield:** Users deposit USDC, receive vLOFI, and earn CCM (Token-2022) based on attention scores
- **Merkle Claims:** Off-chain scoring produces merkle roots; users claim rewards with on-chain proofs
- **Transfer Fee Harvesting:** Automated management of Token-2022 transfer fees into protocol treasury
- **Price Feeds:** On-chain price oracle for CCM and vLOFI
- **Channel Staking:** Lock CCM for soulbound NFT positions with MasterChef-style rewards

## Architecture

| Program | Directory | Framework | Status |
|---------|-----------|-----------|--------|
| **ao-v2** (Attention Oracle) | `programs/ao-v2/` | Pinocchio (raw BPF) | **Live on mainnet** |
| attention-oracle (v1) | `programs/attention-oracle/` | Anchor 0.32.1 | Legacy (source reference) |
| channel-vault | `programs/channel-vault/` | Anchor 0.32.1 | Legacy (deployed, separate program) |

The program was rewritten from Anchor to Pinocchio in March 2026 (867KB -> 153KB binary). Same program ID, same account layouts, same instruction discriminators.

## Build

```bash
# Build the program
cargo build-sbf --manifest-path programs/ao-v2/Cargo.toml

# Run tests (LiteSVM, no validator needed)
cargo test --manifest-path programs/ao-v2/Cargo.toml
```

## Verify

See [VERIFY.md](VERIFY.md) for full verification instructions.

```bash
# Quick verify
solana-verify get-program-hash GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

# Verify from this repo
solana-verify verify-from-repo \
  --url https://github.com/twzrd-sol/attention-oracle-program \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --library-name ao_v2 \
  --bpf-flag channel_staking
```

## Upgrade Authority

Squads V4 multisig (3-of-5): `BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ`

## Security

Report vulnerabilities to **security@twzrd.xyz**. See [SECURITY.md](SECURITY.md).

## Links

- **Protocol:** [twzrd.xyz](https://twzrd.xyz)
- **Signal API:** [api.twzrd.xyz/v1/signals/momentum](https://api.twzrd.xyz/v1/signals/momentum)
- **Python SDK:** [wzrd-client on PyPI](https://pypi.org/project/wzrd-client/)

## License

[MIT](LICENSE-MIT) / [Apache-2.0](LICENSE-APACHE)
