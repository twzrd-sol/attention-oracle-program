# Attention Oracle: Verifiable Distribution Protocol with x402 Payment Integration

## 🏆 Hackathon Submission - Best x402 API Integration Track

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
│   Twitch Oracle     │ ← Off-chain data aggregation
│   (Private IP)      │   Measures engagement
└──────────┬──────────┘
           │ Merkle Root
           ↓
┌─────────────────────┐
│  x402-Gated API     │ ← Payment required for access
│  /get-attention-    │   AI agents pay to query
│       score         │
└──────────┬──────────┘
           │ Commitment
           ↓
┌─────────────────────┐
│   Solana Program    │ ← On-chain verification
│   (Token-2022)      │   Cryptographic proofs
└──────────┬──────────┘
           │ Claims
           ↓
┌─────────────────────┐
│      Viewers        │ ← Token distribution
│   Claim Rewards     │   Verifiable & trustless
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

### On-Chain Program (Solana)

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
        'X-402-Currency': 'SOL'
    });
}
// Return attention data after payment...
```

## 📊 Performance & Security

- **Gas Optimization**: Ring buffer uses 1 bit per claim (256 claims = 32 bytes vs 8KB+ for PDAs)
- **Time-lock Protection**: 7-day grace period, no emergency admin overrides
- **Double-claim Prevention**: Bitmap guards ensure each proof works exactly once
- **Cryptographic Security**: Leaf binding prevents proof reuse across wallets

## 🚀 Quick Start

### Run the x402 API Demo

```bash
# Install dependencies
cd x402-api-server
npm install

# Start the development server
npm run dev

# Visit http://localhost:3000
```

### Test the Payment Flow

1. Enter any creator name (e.g., "kai_cenat")
2. Click "Try Without Payment" → Receives 402 error
3. Click "Pay with x402" → Simulates payment
4. Data is returned with mock attention scores

### Build the On-Chain Program

```bash
cd programs/attention-oracle
anchor build
```

## 🎥 Demo Video

[View Demo Video](#) - Shows full x402 payment flow and on-chain claiming

## 🌟 Why This Matters

**Current State**: Every project rebuilds the same infrastructure. Centralized claim servers. Trust assumptions. No composability.

**Our Vision**: Shared primitive for verifiable distribution. Trustless. Composable. Economically sustainable through x402 payments.

This is infrastructure for the **measurement layer of crypto** - enabling:
- Gaming achievements & leaderboards
- Content engagement tracking
- Governance voting & delegation
- Reputation credentials & attestations

## 📝 License

MIT - Public good infrastructure

## 🔗 Links

- [GitHub Repository](https://github.com/twzrd-sol/attention-oracle-program)
- [On-chain Program](https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop)
- [x402 Documentation](https://docs.x402.org)

---

**Built for the x402 Hackathon** | Don't trust, verify.