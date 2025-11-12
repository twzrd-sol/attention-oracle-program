# Attention Oracle: Verifiable Distribution Protocol with x402 Payment Integration

## 🚀 Overview

A production-grade Solana program implementing Token-2022 claim verification with Merkle proofs, integrated with x402 payment-gated API access for AI agents.

## 🚀 Live Demo

- **On-chain Program**: [`GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`](https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop)
- **x402 API Demo**: Run `cd x402-api-server && npm run dev` then visit http://localhost:3000

## 🎯 Problem We Solve

Every off-chain aggregation system faces the same coordination failure:
```
Off-chain measurement → Centralized database → Manual distribution → Trust requirement
```

**Our Solution**: Separate measurement (subjective, off-chain) from settlement (objective, on-chain) using cryptographic proofs and x402 payment rails.

## 🏗️ Architecture

```
┌─────────────────────┐
│      Off-Chain      │
│    Data Collector   │ ← Off-chain data aggregation  
└──────────┬──────────┘   Measures engagement
           │ Merkle Root
           ↓
┌─────────────────────┐
│    x402-Gated API   │ ← Payment required for access
│   /get-attention-   │   AI agents pay to query
│       score         │
└──────────┬──────────┘
           │ Commitment
           ↓
┌─────────────────────┐
│    Solana Program   │ ← On-chain verification
│    (Token-2022)     │   Cryptographic proofs
└──────────┬──────────┘
           │ Claims
           ↓
┌─────────────────────┐
│      Viewers        │ ← Token distribution
│    Claim Rewards    │   Verifiable & trustless
└─────────────────────┘
```

## 💡 Key Innovation: x402 Payment Integration

AI agents and data consumers must pay via x402 to access attention metrics:

1. **Request without payment** → 402 Payment Required
2. **Submit x402 payment proof** → Access granted
3. **Retrieve attention data** → Merkle roots, scores, distributions

This creates a sustainable economic model where:
- Data consumers (AI agents) pay for access
- Payments fund the oracle infrastructure
- Viewers receive tokens based on verifiable engagement

## 🔧 Technical Implementation

### On-Chain Program (Solana) + Switchboard Oracle Integration

**Ring Buffer Storage Model** (Gas-optimized):
```rust
#[account(zero_copy)]
pub struct ChannelState {
    pub mint: Pubkey,           // Token-2022 mint
    pub latest_epoch: u64,
    pub slots: [ChannelSlot; 9], // Ring buffer
} // ~9.5KB total

pub struct ChannelSlot {
    pub root: [u8; 32],         // Merkle commitment
    pub claim_count: u16,       // Expected claims
    pub bitmap: [u8; 1024],     // 8192 claim slots
}
```

**Merkle Proof Verification**:
```rust
fn verify_claim(proof: &[[u8; 32]], claimer: Pubkey, index: u32, amount: u64) -> bool {
    let leaf = keccak256(claimer || index || amount);
    verify_merkle_proof(proof, leaf, root) && !is_claimed(bitmap, index)
}
```

### x402 API Server

Protected endpoint with payment verification:
```typescript
// GET /api/get-attention-score
if (!verifyX402Payment(request)) {
    return Response(402, {
        'X-402-Payment-Required': 'true',
        'X-402-Price': '0.001',
        'X-402-Currency': 'USDC'
    });
}
// Return attention data after payment...
```

### Switchboard Oracle Integration (V2 aggregators)

**API Layer (implemented):** Price feeds via `@switchboard-xyz/sbv2-lite` decoder
- Endpoint: `/api/switchboard/price` — returns the latest decoded value if fresh, otherwise `{ ok: false }`
- Included in `/api/get-attention-score` as `oracle_context` when available

**On‑Chain Layer (optional extension):** If desired, the program can persist external pricing or use it for policy. This repository does not include on‑chain Switchboard handlers; the snippet below is illustrative only.
```rust
// Example extension only (not implemented in this repo)
pub fn update_price_feed(ctx: Context<UpdatePriceFeed>) -> Result<()> {
    let usdc_sol_price = decode_switchboard_price(&price_feed);
    // persist or validate against policy
    Ok(())
}
```

**Configuration** (optional environment variables)
```bash
SB_CLUSTER=devnet  # or mainnet-beta (default: devnet)
SB_FEED=GvDMxPzN1sCj7L26YDK2HnMRXEQmQ2aemov8YBtPS7vR
SB_MAX_STALENESS_SEC=300   # if the feed is older than this, ok will be false
```

## 📊 Performance & Security

- **Gas Optimization**: Ring buffer uses 1 bit per claim (256 claims = 32 bytes vs 8KB+ for PDAs)
- **Time-lock Protection**: 7-day grace period, no privileged admin overrides
- **Double-claim Prevention**: Bitmap guards ensure each proof works exactly once
- **Cryptographic Security**: Leaf binding prevents proof reuse across wallets

## 🚀 Quick Start

**No environment variables required.** The demo works out of the box. Optional: Set `SB_CLUSTER=devnet` for Switchboard integration.

### Run the x402 API Demo

```bash
# Install dependencies
cd x402-api-server
npm install

# Start the development server
npm run dev

# Visit http://localhost:3000
```

**What you'll see:**
1. Interactive demo page with creator input field
2. "Try Without Payment" → Returns HTTP 402 Payment Required
3. "Pay with x402" → Simulates payment and returns data with Merkle roots

### Verify a Payment (trustless agent helper)

Optionally verify an x402 payment via Solana RPC:

```
export X402_RECIPIENT=<your_wallet_pubkey>
export X402_MIN_LAMPORTS=1000

# After sending a SOL transfer, verify by signature
curl "http://localhost:3000/api/verify-payment?tx=<SIG>"
```

### Test the Payment Flow

1. Enter any creator name
2. Click "Try Without Payment" → Receives 402 error
3. Click "Pay with x402" → Simulates payment
4. Data is returned with mock attention scores

### Build the On-Chain Program

```bash
cd programs/attention-oracle
anchor build
```

### Optional: Minimal JS Client (x402 Dev Tool)

```ts
import { getAttentionScore } from './clients/js/x402-client';

const data = await getAttentionScore('example_user', { baseUrl: 'http://localhost:3000' });
console.log(data);
```


## 🌟 Why This Matters

**Current State**: Every project rebuilds the same infrastructure. Centralized claim servers. Trust assumptions. No composability.

**Our Vision**: Shared primitive for verifiable distribution. Trustless. Composable. Economically sustainable through x402 payments.

This is infrastructure for the **measurement layer of crypto** - enabling:
- Gaming achievements & leaderboards
- Content engagement tracking
- Governance voting & delegation
- Reputation credentials & attestations

## 📝 License

Dual-licensed under either of:
- Apache License, Version 2.0
- MIT License

You may use this project under the terms of either license at your option.

## 🔗 Links

- [GitHub Repository](https://github.com/twzrd-sol/attention-oracle-program)
- [On-chain Program](https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop)
- [x402 Documentation](https://docs.x402.org)

---

**Don't trust. Verify. And get paid for it.**
