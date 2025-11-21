# Attention Oracle Reference API

**Official reference implementation** demonstrating how to integrate with the Attention Oracle protocol.

Shows integrators how to:
- ✅ Query on-chain reputation (PassportRegistry)
- ✅ Inspect channel state (ChannelState ring buffer)
- ✅ Verify Merkle proofs off-chain (save gas)
- ✅ Gate endpoints with x402 using on-chain reputation

**Program ID:** `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
**Version:** 1.0.0

---

## Quick Start

```bash
npm install
cp .env.example .env
npm run dev
```

Server runs on `http://localhost:3000`

---

## API Endpoints

### 1. GET /passport/:user_hash

Query on-chain reputation for any user.

**Example:**
```bash
curl http://localhost:3000/passport/deadbeef1234...
```

**Response:**
```json
{
  "ok": true,
  "passport": {
    "owner": "WalletAddress...",
    "tier": 3,
    "score": "15420",
    "epoch_count": 42,
    "badges": 7
  }
}
```

### 2. GET /channel/:mint/:channel_id

Inspect channel state ring buffer.

**Example:**
```bash
curl http://localhost:3000/channel/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v/example_channel
```

### 3. POST /verify-proof

Verify Merkle proof off-chain before submitting transaction.

**Example:**
```bash
curl -X POST http://localhost:3000/verify-proof \
  -H "Content-Type: application/json" \
  -d '{"leaf_hex":"...", "proof_hex_array":["..."], "root_hex":"..."}'
```

**Response:**
```json
{
  "ok": true,
  "valid": true
}
```

### 4. GET /premium

x402-gated endpoint requiring minimum reputation.

**Example:**
```bash
curl -H "X-Wallet-Pubkey: YourWalletAddress..." \
  http://localhost:3000/premium
```

**Response (Insufficient Rep):**
```json
{
  "ok": false,
  "code": 402,
  "message": "Payment Required - Insufficient Reputation",
  "requirement": "Tier >= 2 OR Score >= 1000"
}
```

---

## Configuration

**Environment Variables:**

```bash
PORT=3000
RPC_URL=https://api.mainnet-beta.solana.com
PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
MIN_TIER_PREMIUM=2
MIN_SCORE_PREMIUM=1000
```

---

## For Integrators

**What you can build:**

1. **Leaderboard UI** - Rank users by score/tier
2. **Claim Validator** - Pre-validate proofs before transactions
3. **Reputation-Gated Features** - Unlock features based on tier
4. **Analytics Dashboard** - Track channel activity
5. **Premium APIs** - Gate your APIs using x402 pattern

**PDA Derivation:**

```typescript
// PassportRegistry PDA
PublicKey.findProgramAddressSync(
  [Buffer.from("passport_owner"), userHash],
  programId
)[0];

// ChannelState PDA
PublicKey.findProgramAddressSync(
  [Buffer.from("channel_state"), mint.toBuffer(), streamerKey.toBuffer()],
  programId
)[0];

// Streamer Key (Keccak256)
const hash = keccak256(Buffer.from(`channel:${channel.toLowerCase()}`));
const streamerKey = new PublicKey(hash);
```

---

## Production Notes

1. **Signature Verification:** The `/premium` endpoint trusts headers in demo mode. Add `nacl.sign.detached.verify()` in production.
2. **Rate Limiting:** Add express-rate-limit to prevent abuse.
3. **Caching:** Cache passport lookups to reduce RPC load.
4. **Monitoring:** Add Prometheus metrics for observability.

---

## License

MIT OR Apache-2.0 (matches parent repo)
