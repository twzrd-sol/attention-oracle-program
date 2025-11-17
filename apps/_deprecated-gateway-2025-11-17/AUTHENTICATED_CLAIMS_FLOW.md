# Authenticated Claims Flow - The Correct Implementation

## 🎯 The Critical Insight

**The Previous Misunderstanding:**
The AI was searching for Solana wallet addresses in the merkle tree and trying to claim with private keys we already had. This is backwards.

**The Correct Flow:**
The merkle tree contains **Twitch user identity hashes**, not wallet addresses. Users log in with Twitch OAuth to prove their identity, then claim to **ANY** Solana wallet they choose.

---

## 📋 How It Actually Works

### 1. During the Stream (Aggregator Side - Already Built)
```
User watches stream → Aggregator captures Twitch ID
                   → Computes user_hash = keccak256(twitchId)
                   → Stores in sealed_participants with index
                   → Builds merkle tree from user_hashes
                   → Publishes root on-chain
```

### 2. Claiming Tokens (Gateway + Frontend - Just Built)

```
User visits gateway.twzrd.xyz
         ↓
"Login with Twitch" → OAuth flow
         ↓
Gateway creates JWT session with twitchId
         ↓
User calls: GET /api/claims/available
         ↓
Gateway computes user_hash from session.twitchId
         ↓
Gateway queries database for all epochs where this user_hash exists
         ↓
Returns list of claimable epochs
         ↓
User selects epoch to claim
         ↓
User calls: GET /api/claims/proof/:epoch/:channel
         ↓
Gateway generates merkle proof for that user_hash
         ↓
User connects Phantom/Backpack (ANY wallet)
         ↓
Frontend submits claim transaction with proof
         ↓
On-chain program verifies proof and transfers tokens
```

---

## 🚀 New API Endpoints

All endpoints require authentication via JWT session cookie (obtained from Twitch OAuth).

### 1. Get Available Claims
**Endpoint:** `GET /api/claims/available`

**Headers:**
```
Cookie: session=<jwt_token>
```

**Response:**
```json
{
  "twitchLogin": "yourUsername",
  "twitchDisplayName": "Your Display Name",
  "user_hash": "1af8e7e6e1904ed7...",
  "claims": [
    {
      "epoch": 1762308000,
      "channel": "marlon",
      "index": 42,
      "root": "0xdc8518d0cf98c015...",
      "sealedAt": 1762315442,
      "published": true,
      "estimatedAmount": "1024000000000"
    }
  ],
  "totalClaimable": 1
}
```

### 2. Get Proof for Specific Epoch
**Endpoint:** `GET /api/claims/proof/:epoch/:channel`

**Example:** `GET /api/claims/proof/1762308000/marlon`

**Headers:**
```
Cookie: session=<jwt_token>
```

**Response:**
```json
{
  "twitchLogin": "yourUsername",
  "twitchDisplayName": "Your Display Name",
  "channel": "marlon",
  "epoch": 1762308000,
  "index": 42,
  "root": "0xdc8518d0cf98c015e61d181831fce8dfcd391062c45fa7bb4196953ce3e5effa",
  "proof": [
    "0xacc507740d063fa57ab16547fec16b4f7432b5948e18e12a0aa691b00ff51aa5",
    "0xb12c7bd00de68d34b841c01404b4268aa87346acc9da95afae7ae262092c9ecb",
    ...
  ],
  "participantCount": 628,
  "user_hash": "1af8e7e6e1904ed7...",
  "instructions": {
    "step1": "Connect your Solana wallet (Phantom, Backpack, etc.)",
    "step2": "Submit this proof with a claim transaction",
    "step3": "Sign the transaction with your wallet",
    "note": "You can claim to ANY wallet - it does not need to match your Twitch account"
  }
}
```

### 3. Get Latest Claimable Epoch
**Endpoint:** `GET /api/claims/proof/latest?channel=:channel`

**Example:** `GET /api/claims/proof/latest?channel=marlon`

**Behavior:** Redirects to `/api/claims/proof/:epoch/:channel` for the most recent epoch where user participated.

---

## 🔐 Authentication Flow

### OAuth Login
**Endpoint:** `GET /oauth/twitch/login`

Redirects to Twitch OAuth, then back to callback.

**Callback:** `GET /oauth/twitch/callback`

Creates JWT session with:
```json
{
  "twitchId": "123456789",
  "twitchLogin": "username",
  "twitchDisplayName": "Display Name",
  "profileImage": "https://...",
  "accessToken": "...",
  "exp": 1234567890
}
```

Stored in httpOnly cookie named `session`.

### Check Session
**Endpoint:** `GET /oauth/twitch/session`

Returns current authenticated user or `{ authenticated: false }`.

### Logout
**Endpoint:** `POST /oauth/twitch/logout`

Clears session cookie.

---

## 💡 Key Technical Details

### User Hash Computation
```typescript
import { keccak_256 } from '@noble/hashes/sha3.js';

const twitchId = session.twitchId; // e.g., "123456789"
const user_hash = Buffer.from(
  keccak_256(Buffer.from(twitchId, 'utf8'))
).toString('hex');
```

**CRITICAL:** This must match exactly how the aggregator computes the user_hash when building the merkle tree.

### Database Schema
```sql
-- Sealed participants (frozen snapshot of who watched)
CREATE TABLE sealed_participants (
  epoch BIGINT NOT NULL,
  channel TEXT NOT NULL,
  idx INTEGER NOT NULL,  -- Position in merkle tree
  user_hash TEXT NOT NULL,  -- keccak256(twitchId)
  username TEXT,  -- Display name (optional)
  PRIMARY KEY (epoch, channel, idx)
);

-- Sealed epochs (merkle root published)
CREATE TABLE sealed_epochs (
  epoch BIGINT NOT NULL,
  channel TEXT NOT NULL,
  root TEXT NOT NULL,  -- Merkle root (hex)
  sealed_at BIGINT NOT NULL,
  published INTEGER DEFAULT 0,  -- 0 = not published, 1 = published on-chain
  PRIMARY KEY (epoch, channel)
);
```

### On-Chain Claim Format
The frontend needs to build a claim transaction with:
```typescript
{
  channel: string,
  epoch: number,
  index: number,  // User's position in tree
  amount: bigint,  // Tokens to claim (from proof response)
  id: string,  // Format: "twitch:{channel}:{user_hash}"
  proof: string[]  // Merkle siblings
}
```

---

## 🧪 Testing the Flow

### 1. Log in via Twitch
```bash
# Visit in browser
open http://localhost:8082/oauth/twitch/login
```

### 2. Get your claims
```bash
curl -X GET http://localhost:8082/api/claims/available \
  -H "Cookie: session=<your_jwt_token>" \
  --cookie-jar cookies.txt
```

### 3. Get proof for specific epoch
```bash
curl -X GET http://localhost:8082/api/claims/proof/1762308000/marlon \
  -H "Cookie: session=<your_jwt_token>"
```

### 4. Submit claim transaction
Use the proof from step 3 with the existing claim scripts:
```bash
echo '{
  "channel": "marlon",
  "epoch": 1762308000,
  "index": 42,
  "amount": 1024000000000,
  "id": "twitch:marlon:1af8e7e6e1904ed7...",
  "proof": [...],
  "root": "0x..."
}' > /tmp/my-claim.json

npx tsx scripts/claims/claim-direct.ts /tmp/my-claim.json
```

---

## ✅ What This Fixes

### Before (Broken)
- User had to manually pass their Twitch username as a query parameter
- No authentication - anyone could request anyone's proof
- Frontend had no way to discover what epochs user could claim
- Proofs were tied to specific wallets (backwards)

### After (Correct)
- User authenticates via Twitch OAuth (proves identity)
- Gateway computes user_hash from authenticated session
- User can discover ALL claimable epochs automatically
- User can claim to ANY Solana wallet (not tied to Twitch account)
- Secure: Only the authenticated user can get their own proofs

---

## 🎯 Next Steps for Frontend

1. **Implement OAuth login flow**
   - Redirect to `/oauth/twitch/login`
   - Handle callback and store session

2. **Fetch available claims**
   ```typescript
   const response = await fetch('/api/claims/available', {
     credentials: 'include'  // Include session cookie
   });
   const { claims } = await response.json();
   ```

3. **Display claims to user**
   - Show list of claimable epochs
   - Show estimated amounts
   - Show channel names

4. **Get proof when user clicks "Claim"**
   ```typescript
   const proof = await fetch(`/api/claims/proof/${epoch}/${channel}`, {
     credentials: 'include'
   });
   const proofData = await proof.json();
   ```

5. **Connect Solana wallet**
   - Use Phantom/Backpack adapter
   - Let user choose ANY wallet

6. **Submit claim transaction**
   - Build transaction with proofData
   - Sign with user's connected wallet
   - Submit to Solana

---

## 📊 Database Queries (For Reference)

### Find all epochs where a user participated
```sql
SELECT DISTINCT
  sp.epoch,
  sp.channel,
  sp.idx as index,
  se.root,
  se.sealed_at,
  se.published
FROM sealed_participants sp
JOIN sealed_epochs se
  ON sp.epoch = se.epoch
  AND sp.channel = se.channel
WHERE sp.user_hash = $1
ORDER BY sp.epoch DESC;
```

### Get participants for epoch (to build proof)
```sql
SELECT user_hash
FROM sealed_participants
WHERE epoch = $1
  AND channel = $2
ORDER BY idx ASC;
```

---

## 🔒 Security Considerations

1. **Rate Limiting:** All endpoints have rate limits (30-60 req/min)
2. **JWT Expiry:** Sessions expire after 1 hour
3. **HttpOnly Cookies:** Session cannot be accessed by JavaScript
4. **CORS:** Only allowed origins can access API
5. **No Wallet Exposure:** user_hash in responses is truncated

---

## 🎉 Success Criteria

- ✅ User can log in with Twitch
- ✅ User can see all their claimable epochs
- ✅ User can get merkle proof for any epoch
- ✅ Proof is valid for on-chain verification
- ✅ User can claim with ANY Solana wallet
- ✅ No manual username input required
- ✅ Fully authenticated and secure

This is the correct implementation. The invisible string is complete.
