# Attention Oracle

## Open-Source Token Distribution Protocol for Creator Economies on Solana

### Overview

Attention Oracle is an open-source protocol that enables verifiable token distribution for content creators and their communities. Built on Solana using Token-2022 extensions, it provides gas-efficient merkle tree-based claiming with dynamic fee tiers.

**Status**: Production (Mainnet Deployed)  
**Program ID**: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`  
**License**: Dual MIT/Apache-2.0  

📊 **[View Pitch Deck](PITCH.md)** - Learn about our vision and roadmap

### Core Components

#### 1. Token-2022 Program (`/programs`)
- Merkle tree-based token distribution
- Dynamic fee tiers with passport verification
- Transfer hooks for automated fee collection
- Gas-optimized claiming (~5,000 CU per claim)
- Multi-channel support with ring buffer storage

#### 2. X402 Integration (`/x402-api-server`)
- Switchboard Oracle integration
- Off-chain data aggregation
- Merkle root generation
- Fee harvest automation

### Technical Architecture

```
┌────────────────────────┐
│   Off-Chain Oracle     │
│  (X402 + Switchboard)  │
└───────────┬────────────┘
            │
┌───────────▼────────────┐
│   Solana Program       │
│  Token-2022 + Hooks    │
└────────────────────────┘
```

### Quick Start

```bash
# Clone the repository
git clone https://github.com/twzrd-sol/attention-oracle-program
cd attention-oracle-program

# Build the program
cd programs
cargo build-sbf

# Run tests
cargo test-sbf
```

### Verify On-Chain Deployment

```bash
solana-verify verify-from-repo \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --mount-path programs \
  --library-name token_2022 \
  https://github.com/twzrd-sol/attention-oracle-program
```

### Documentation

- [Pitch Deck](PITCH.md) - Vision, roadmap, and business model
- [Security Policy](SECURITY.md) - Vulnerability disclosure process

### Security

For security concerns, please email: security@twzrd.xyz

### License

This project is dual-licensed under:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

You may choose either license at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you shall be dual licensed as above, without any additional terms or conditions.
