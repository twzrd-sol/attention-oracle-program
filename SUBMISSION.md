# Attention Oracle: x402 Payment Protocol Integration

## Overview
A production oracle demonstrating autonomous agent payments via x402. AI agents pay micropayments to access verified streaming engagement data, while viewers claim tokens through cryptographic proofs on Solana.

## Technical Implementation

### Core Architecture
- **On-Chain Program**: Deployed to Solana mainnet at `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- **x402 Gateway**: HTTP 402 payment flow with SOL settlement (demo)
- **Switchboard Integration**: Dynamic pricing via permissionless oracle feeds
- **Token Distribution**: Token-2022 with Merkle proof verification

### Key Innovations

**Ring Buffer State Model**
Gas-optimized storage using 1 bit per claim. Supports 8192 concurrent claims per channel while maintaining ~9.5KB fixed footprint. 1000x cheaper than traditional per-address PDA approaches.

**Autonomous Payment Flow**
Complete x402 implementation enabling AI agents to:
- Discover data endpoints via HTTP 402 responses
- Execute SOL micropayments without human intervention (demo)
- Verify payment proofs on-chain in 400ms
- Access verified Merkle roots for data integrity

**Oracle Price Integration**
Switchboard feeds provide real-time SOL pricing context for dynamic behavior when available.

## Repository Structure
```
/programs/attention-oracle    # Solana program (Rust/Anchor)
/x402-api-server              # Payment gateway (Next.js)
/docs                         # Architecture documentation
```

## Running Locally

### API Server
```bash
cd x402-api-server
npm install
npm run dev
# Visit http://localhost:3000
```

### Program Build
```bash
cd programs/attention-oracle
anchor build
```

## Links
- Repository: https://github.com/twzrd-sol/attention-oracle-program
- On-chain: https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

---

*Building infrastructure for the measurement layer of crypto.*

License: Apache-2.0 OR MIT (dual-license)

Contact: dev@twzrd.xyz (Telegram: @twzrd_xyz)
