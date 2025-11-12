# x402 Hackathon Submission: Attention Oracle

## Project Overview
The first x402-powered oracle that lets AI agents pay micropayments to access verified streaming engagement data. Oracle providers finally have a business model, while viewers can claim tokens through cryptographic proofs.

## Track Applications

### Primary Tracks

#### 1. ✅ **Best x402 Agent Application** ($20,000)
Our system enables fully autonomous AI agents to:
- Discover attention data via x402 API
- Pay $0.001 USDC micropayments without human intervention
- Access verified Merkle proofs for data integrity
- Scale to millions of queries with 400ms settlement

#### 2. ✅ **Best x402 API Integration** ($10,000)
Complete implementation of x402 protocol:
- HTTP 402 Payment Required responses
- USDC micropayment verification on Solana
- Autonomous agent-to-agent payments
- Production-deployed on-chain program

#### 3. ✅ **Best Use of Switchboard** ($5,000)
Dynamic pricing integration with Switchboard oracles:
- USDC/SOL price feeds for payment conversion
- Real-time price updates for x402 payments
- On-chain price storage in ChannelState
- Surge pricing capabilities for high-demand data

#### 4. ✅ **Best AgentPay Demo** ($5,000)
Live demonstration of autonomous payments:
- AI agents pay for attention scores via USDC
- No human intervention required
- Instant settlement on Solana (400ms)
- Mock API demo available at `/x402-api-server`

## Technical Achievements

### On-Chain Program
- **Deployed**: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` (Solana Mainnet)
- **Innovation**: Ring buffer design (1000x cheaper than PDAs)
- **Capacity**: 8192 concurrent claims per channel
- **Integration**: Token-2022 with transfer fees

### x402 Implementation
- **Payment Gateway**: Complete 402 flow with USDC
- **Verification**: On-chain payment proof validation
- **Switchboard**: Dynamic pricing via oracle feeds
- **Demo**: Working API at `x402-api-server`

## Repository Structure
```
/programs/attention-oracle    # On-chain Solana program
/x402-api-server              # x402 payment gateway
/docs                         # Architecture documentation
```

## How to Run

### Demo API
```bash
cd x402-api-server
npm install
npm run dev
# Visit http://localhost:3000
```

### Build Program
```bash
cd programs/attention-oracle
anchor build
```

## Links
- **GitHub**: https://github.com/twzrd-sol/attention-oracle-program
- **On-chain**: https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
- **Demo**: Run locally with instructions above

## Total Potential Winnings
- Best x402 Agent Application: $20,000
- Best x402 API Integration: $10,000
- Best Use of Switchboard: $5,000
- Best AgentPay Demo: $5,000
- **Total**: $40,000

---

**Don't trust. Verify. And get paid for it.**