# Technical Deep Dive: How x402 Powers the Attention Oracle

## Architecture Overview

The Attention Oracle is split into two distinct systems:

1. **Off-chain Collector** — Collects and processes event data (reference architecture)
2. **x402 API** — Monetization layer that gates access via HTTP‑402

This separation ensures clear boundaries between off‑chain aggregation and on‑chain verification while enabling a sustainable business model.

---

## ⚙️ Part 1: Off-chain Collector (Reference)

This component runs off‑chain and aggregates events, builds allocations, and publishes Merkle roots on a schedule. The on‑chain program and API in this repo are fully open‑source; collectors can be implemented using the interface below.

### Step 1: Collect (Off-Chain)
```typescript
// stream-collector.ts (example)
// Continuously monitors channels and aggregates engagement signals
// Outputs normalized events for allocation pipeline
```

### Step 2: Process (Off-Chain)
```typescript
// build-allocations.ts (example)
// Runs every epoch (hourly)
// Aggregates normalized events
// Calculates participation scores
// Builds Merkle Tree of all claims
```

### Step 3: Publish (On-Chain)
```rust
// Only the Merkle root goes on-chain (32 bytes)
pub fn publish_merkle_root(
    ctx: Context<PublishRoot>,
    root: [u8; 32],
    epoch: u64
) -> Result<()> {
    // Commit root to ChannelState on Solana
    channel_state.slots[epoch % 9].root = root;
    channel_state.latest_epoch = epoch;
}
```

**Result:** Attention data is cryptographically proven via Merkle commitments; underlying raw data never needs to be published.

---

## 🤖 Part 2: The Vending Machine (Public x402 API)

*This is what AI agents interact with - fully autonomous payment flow.*

### The Complete x402 Flow

#### 1. The Request (Agent → API)
```bash
GET /api/get-attention-score?creator=example_user
```
AI agent wants attention data for a creator.

#### 2. The Paywall (API → Agent)
```http
HTTP/1.1 402 Payment Required
X-402-Payment-Required: true
X-402-Price: 0.001
X-402-Currency: SOL
X-402-Recipient: GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

{
  "error": "Payment Required",
  "payment_instructions": {
    "method": "x402",
    "price": "0.001 SOL",
    "recipient": "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
  }
}
```
Server returns 402 with payment invoice.

Note: For dynamic pricing/validation, the API can read a Switchboard price feed (e.g., SOL/USD) and surface it as `oracle_context` in responses.

#### 3. The Payment (Agent → Solana)
```typescript
// Agent builds and sends transaction (SOL)
const tx = await connection.sendTransaction({
    from: agent_wallet,
    to: "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop",
    amount: 0.001 * LAMPORTS_PER_SOL,
    token: "SOL"
});
// Confirmed in ~400ms on Solana
```

#### 4. The Proof (Agent → API)
```bash
GET /api/get-attention-score?creator=example_user
Header: X-402-Payment: 5xKb9...transaction_signature
```
Agent retries with payment proof.

#### 5. The Verification (API Internal)
```typescript
async function verifyPayment(txSignature: string) {
    // Check Solana blockchain
    const tx = await connection.getTransaction(txSignature);

    // Verify payment details
    if (tx.recipient === OUR_WALLET &&
        tx.amount >= 0.001 &&
        tx.token === "SOL") {
        return true;
    }
    return false;
}
```

#### 6. The Fulfillment (API → Agent)
```json
HTTP/1.1 200 OK
{
  "status": "success",
  "data": {
    "creator": "example_user",
    "attention_score": 9435,
    "merkle_root": "0x7f9a8b2c...",
    "epoch": 489697,
    "participants": 5132,
    "distribution_available": true,
    "oracle_context": { "source": "switchboard", "cluster": "devnet", "feed": "GvDMxPz...", "sol_usd": 183.42 }
  },
  "payment": {
    "verified": true,
    "transaction_id": "5xKb9..."
  }
}
```

---

## 💎 Why This Matters

### Traditional Oracle Problems:
- **No Revenue:** Provide data for free → Die from costs
- **Trust Required:** Users must trust the oracle's data
- **Not Scalable:** Can't handle micropayments efficiently

### Your x402 Solution:
- **Revenue Stream:** $0.001 per query × millions of queries = sustainable business
- **Trustless:** Merkle proofs allow independent verification
- **Instant:** 400ms payment confirmation enables real-time access
- **Autonomous:** AI agents can pay without human intervention

---

## 🔐 Security & Data Boundaries

### Off‑Chain (runs outside the chain):
- Aggregation/collection processes
- Scoring heuristics and allocation logic
- Event normalization pipelines
- Raw telemetry storage

### What Goes Public:
- ✅ Merkle roots (just hashes)
- ✅ x402 payment interface
- ✅ Claim verification logic
- ✅ Token distribution mechanics

---

## 📊 Economic Model

### Revenue Projections:
```
Conservative:
- 1,000 queries/day × $0.001 = $1/day
- 30,000 queries/month = $30/month

Growth scenario:
- 100,000 queries/day × $0.001 = $100/day
- 3,000,000 queries/month = $3,000/month

Scale scenario:
- 10,000,000 queries/day × $0.001 = $10,000/day
- 300,000,000 queries/month = $300,000/month
```

### Cost Structure:
```
- Solana fees: $0.00025 per transaction (negligible)
- Infrastructure: ~$100/month for servers
- Break-even: 100,000 queries/month
```

---

## 🚀 Implementation Checklist

### Already Built ✅ (Open Source)
- [x] On-chain Merkle verification program
- [x] Ring buffer state management
- [x] x402 payment gateway
- [x] Mock API for demonstration
- [x] Token-2022 integration

### Off-chain Components (Implement using interface)
- [x] Example collectors (design interface)
- [x] Merkle tree builders
- [x] Database infrastructure
- [x] Claim distribution system

### Future Enhancements 🔮
- [ ] Multi-chain x402 support
- [ ] Subscription tiers for bulk queries
- [ ] Real-time WebSocket feeds
- [ ] Cross-platform oracle expansion

---

## Summary

The Attention Oracle demonstrates the perfect x402 use case:

1. **High-value data** (Streaming engagement metrics)
2. **Micropayment-appropriate** ($0.001 is reasonable for this data)
3. **Agent-friendly** (No human interaction needed)
4. **Verifiable** (Merkle proofs ensure trustlessness)
5. **Sustainable** (Revenue model built-in from day one)

This isn't just an integration - it's the blueprint for how all Web3 oracles should operate.

**The future of oracles is paid, verifiable, and autonomous. We built it.**
