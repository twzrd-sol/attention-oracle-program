# Backend Implementation Spec – Agent B

**Date**: November 15, 2025
**Scope**: Implement `/api/verification-status` and `/api/claim-cls` endpoints for Portal v3
**Status**: Specification Ready

---

## 📋 Overview

Portal v3 (React + Vite) is production-ready. Now implement two HTTP endpoints that:

1. **GET /api/verification-status?wallet=<pubkey>**
   - Returns Twitter follow + Discord join status
   - Enables Portal v3 to show verification badges

2. **POST /api/claim-cls**
   - Accepts wallet + epoch ID
   - Builds & returns base64-encoded Solana transaction
   - Enforces one-claim-per-epoch

---

## 🔗 API Contract (Final)

### GET /api/verification-status

**Query Parameters:**
```
wallet: string (base58 Solana pubkey)
```

**Response (200 OK):**
```json
{
  "twitterFollowed": true,
  "discordJoined": true,
  "passportTier": 2,
  "lastVerified": "2025-11-15T09:42:00.000Z"
}
```

**Responses:**
- `200` - Success (fields can be false even if status is OK)
- `400` - Bad request (invalid/missing wallet)
- `500` - Server error

**Notes:**
- `passportTier` can be null or omitted
- `lastVerified` is optional (ISO 8601 timestamp)
- If wallet not in DB yet, return false for both

---

### POST /api/claim-cls

**Request Body:**
```json
{
  "wallet": "So1anaPubKeyBase58...",
  "epochId": 7
}
```

**Response (200 OK):**
```json
{
  "transaction": "AgABBi0a2QsgzYK3...",
  "signature": null
}
```

**Error Responses:**
- `400` - Invalid request (bad wallet, bad epochId, epoch not found)
- `402` - Payment required (custom: wallet has insufficient balance or not verified)
- `403` - Forbidden (verification not satisfied)
- `409` - Conflict (already claimed for this epoch)
- `500` - Server error

**Notes:**
- `transaction` is base64-encoded unsigned/partially-signed Transaction
- `signature` is null (client signs and submits)
- Do NOT store in `cls_claims` yet; wait for client callback or on-chain confirmation

---

## 📊 Data Model

### Table: social_verification

Tracks off-chain verification status.

```sql
CREATE TABLE social_verification (
  wallet              TEXT PRIMARY KEY,
  twitter_handle      TEXT,
  twitter_followed    BOOLEAN NOT NULL DEFAULT FALSE,
  discord_id          TEXT,
  discord_joined      BOOLEAN NOT NULL DEFAULT FALSE,
  passport_tier       INTEGER,
  last_verified       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_social_verification_wallet ON social_verification(wallet);
CREATE INDEX idx_social_verification_discord_id ON social_verification(discord_id);
```

**Fields:**
- `wallet` - Base58 Solana pubkey (PK)
- `twitter_handle` - Optional Twitter username
- `twitter_followed` - Whether wallet owner follows @twzrd_xyz
- `discord_id` - Discord user ID (from OAuth callback)
- `discord_joined` - Whether user joined Discord server
- `passport_tier` - Optional tier level (0-6 or similar)
- `last_verified` - When verification was last updated
- `created_at` - When row was created

---

### Table: cls_claims

Tracks claims (enforces one-per-epoch-per-wallet).

```sql
CREATE TABLE cls_claims (
  id                  BIGSERIAL PRIMARY KEY,
  wallet              TEXT NOT NULL,
  epoch_id            INTEGER NOT NULL,
  amount              BIGINT,
  tx_signature        TEXT,
  tx_status           VARCHAR(20) DEFAULT 'pending',  -- pending, confirmed, failed
  created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  confirmed_at        TIMESTAMPTZ,
  UNIQUE(wallet, epoch_id),
  FOREIGN KEY (wallet) REFERENCES social_verification(wallet)
);

CREATE INDEX idx_cls_claims_wallet ON cls_claims(wallet);
CREATE INDEX idx_cls_claims_epoch ON cls_claims(epoch_id);
CREATE INDEX idx_cls_claims_signature ON cls_claims(tx_signature);
```

**Fields:**
- `id` - Auto-incrementing PK
- `wallet` - Base58 pubkey (links to social_verification)
- `epoch_id` - Which epoch this claim is for
- `amount` - Optional token amount claimed
- `tx_signature` - Transaction signature (populated after confirmation)
- `tx_status` - pending → confirmed or failed
- `created_at` - When request was made
- `confirmed_at` - When on-chain confirmed

---

### Table: epochs (Optional)

Stores epoch metadata (merkle root, status, etc.).

```sql
CREATE TABLE epochs (
  epoch_id            INTEGER PRIMARY KEY,
  merkle_root         TEXT NOT NULL,
  is_open             BOOLEAN NOT NULL DEFAULT TRUE,
  total_allocation    BIGINT,
  created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  closed_at           TIMESTAMPTZ
);

CREATE INDEX idx_epochs_is_open ON epochs(is_open);
```

**Fields:**
- `epoch_id` - Unique epoch number
- `merkle_root` - Merkle root for this epoch (on-chain)
- `is_open` - Whether this epoch accepts claims
- `total_allocation` - Optional total tokens for epoch
- `created_at` - Epoch start
- `closed_at` - When epoch was closed

---

## 🛠️ Implementation: GET /api/verification-status

### Express Route Handler

```typescript
import type { Request, Response } from 'express';
import bs58 from 'bs58';
import { db } from './db';

export async function getVerificationStatus(req: Request, res: Response) {
  try {
    // 1) Extract & validate wallet param
    const wallet = String(req.query.wallet || '').trim();
    if (!wallet) {
      return res.status(400).json({
        error: 'Missing wallet query parameter'
      });
    }

    // 2) Sanity check: valid base58 pubkey
    try {
      bs58.decode(wallet);
    } catch {
      return res.status(400).json({
        error: 'Invalid wallet public key (not valid base58)'
      });
    }

    // 3) Query social_verification table
    const row = await db.oneOrNone(
      `SELECT
        twitter_followed,
        discord_joined,
        passport_tier,
        last_verified
      FROM social_verification
      WHERE wallet = $1`,
      [wallet]
    );

    // 4) If no row, return false for both (user not yet verified)
    if (!row) {
      return res.json({
        twitterFollowed: false,
        discordJoined: false,
        passportTier: null,
        lastVerified: null
      });
    }

    // 5) Return status
    res.json({
      twitterFollowed: row.twitter_followed,
      discordJoined: row.discord_joined,
      passportTier: row.passport_tier,
      lastVerified: row.last_verified
    });
  } catch (err) {
    console.error('[getVerificationStatus] Error:', err);
    res.status(500).json({
      error: 'Internal server error'
    });
  }
}
```

### Integration with Express

```typescript
import express from 'express';
import { getVerificationStatus } from './api/verification-status';

const app = express();

app.get('/api/verification-status', getVerificationStatus);
```

---

## 🛠️ Implementation: POST /api/claim-cls

### Step 1: Validation Function

```typescript
import { PublicKey } from '@solana/web3.js';
import bs58 from 'bs58';

export interface ClaimValidationError {
  status: number;
  error: string;
  details?: string;
}

export async function validateClaimRequest(
  wallet: string,
  epochId: number,
  db: any
): Promise<ClaimValidationError | null> {
  // 1) Wallet format
  if (!wallet || typeof wallet !== 'string') {
    return { status: 400, error: 'Missing wallet' };
  }

  let pubkey: PublicKey;
  try {
    bs58.decode(wallet);
    pubkey = new PublicKey(wallet);
  } catch {
    return { status: 400, error: 'Invalid wallet public key' };
  }

  // 2) Epoch ID format
  if (typeof epochId !== 'number' || !Number.isInteger(epochId) || epochId < 0) {
    return { status: 400, error: 'Invalid epochId (must be non-negative integer)' };
  }

  // 3) Epoch exists & is open
  const epoch = await db.oneOrNone(
    'SELECT merkle_root, is_open FROM epochs WHERE epoch_id = $1',
    [epochId]
  );
  if (!epoch) {
    return { status: 400, error: 'Epoch not found' };
  }
  if (!epoch.is_open) {
    return { status: 400, error: 'Epoch is closed' };
  }

  // 4) Verification satisfied
  const sv = await db.oneOrNone(
    `SELECT twitter_followed, discord_joined
     FROM social_verification
     WHERE wallet = $1`,
    [wallet]
  );
  if (!sv || !sv.twitter_followed || !sv.discord_joined) {
    return {
      status: 403,
      error: 'Verification requirements not met',
      details: 'Must have followed @twzrd_xyz on Twitter and joined Discord'
    };
  }

  // 5) One claim per epoch per wallet
  const existing = await db.oneOrNone(
    'SELECT id FROM cls_claims WHERE wallet = $1 AND epoch_id = $2',
    [wallet, epochId]
  );
  if (existing) {
    return {
      status: 409,
      error: 'Already claimed for this epoch'
    };
  }

  return null; // All checks passed
}
```

### Step 2: Transaction Building

```typescript
import { Connection, PublicKey, Transaction, TransactionInstruction } from '@solana/web3.js';
import { Program, AnchorProvider } from '@coral-xyz/anchor';
import * as idl from '../idl/token-2022.json';

const PROGRAM_ID = new PublicKey(process.env.PROGRAM_ID!);
const RPC_URL = process.env.SOLANA_RPC!;
const connection = new Connection(RPC_URL, 'confirmed');

// If using Anchor:
// const provider = new AnchorProvider(connection, null, {});
// const program = new Program(idl as any, PROGRAM_ID, provider);

export async function buildClaimTransaction(args: {
  wallet: PublicKey;
  epochId: number;
  merkleRoot: string;
  db?: any; // optional, if you need to fetch additional data
}): Promise<Transaction> {
  const { wallet, epochId, merkleRoot } = args;

  // Option A: Use Anchor client (if you have IDL loaded)
  // const ix = await program.methods
  //   .claimWithRing(...)
  //   .accounts({ ... })
  //   .instruction();

  // Option B: Build instruction manually (placeholder)
  const ix: TransactionInstruction = {
    programId: PROGRAM_ID,
    keys: [
      { pubkey: wallet, isSigner: true, isWritable: false },
      // ... add remaining keys
    ],
    data: Buffer.alloc(0), // placeholder
  };

  // Create transaction
  const tx = new Transaction();
  tx.add(ix);

  // Set fee payer (optional)
  const feePayerKey = process.env.FEEPAYER_PUBKEY;
  if (feePayerKey) {
    tx.feePayer = new PublicKey(feePayerKey);
  }

  // Get recent blockhash
  const { blockhash, lastValidBlockHeight } =
    await connection.getLatestBlockhash('finalized');
  tx.recentBlockhash = blockhash;

  return tx;
}
```

### Step 3: POST Handler

```typescript
import type { Request, Response } from 'express';
import { PublicKey } from '@solana/web3.js';
import { validateClaimRequest } from './validate-claim';
import { buildClaimTransaction } from './build-claim-tx';

export async function postClaimCls(req: Request, res: Response) {
  try {
    const { wallet, epochId } = req.body || {};

    // 1) Validate everything
    const validationError = await validateClaimRequest(wallet, epochId, db);
    if (validationError) {
      return res.status(validationError.status).json({
        error: validationError.error,
        ...(validationError.details && { details: validationError.details })
      });
    }

    // 2) Build claim transaction
    const walletPubkey = new PublicKey(wallet);
    const epochData = await db.one(
      'SELECT merkle_root FROM epochs WHERE epoch_id = $1',
      [epochId]
    );

    const tx = await buildClaimTransaction({
      wallet: walletPubkey,
      epochId,
      merkleRoot: epochData.merkle_root
    });

    // 3) Serialize to base64
    const serialized = tx.serialize({ requireAllSignatures: false });
    const base64Tx = serialized.toString('base64');

    // 4) Return transaction
    res.json({
      transaction: base64Tx,
      signature: null
    });

    // NOTE: Do NOT insert into cls_claims yet.
    // Instead, listen for:
    // - Client callback with signed tx + signature
    // - Or watch blockchain for confirmation
    // Then update cls_claims status.
  } catch (err) {
    console.error('[postClaimCls] Error:', err);
    res.status(500).json({
      error: 'Internal server error'
    });
  }
}
```

### Integration

```typescript
import express from 'express';
import { postClaimCls } from './api/claim-cls';

const app = express();
app.use(express.json());

app.post('/api/claim-cls', postClaimCls);
```

---

## 🔄 Verification Flow Integration

### Option A: OAuth Callbacks (Recommended)

When user completes Twitter follow or Discord join:

```typescript
// POST /auth/twitter/callback
export async function twitterCallback(req: Request, res: Response) {
  const { wallet, handle } = req.body;

  await db.query(
    `INSERT INTO social_verification (wallet, twitter_handle, twitter_followed)
     VALUES ($1, $2, TRUE)
     ON CONFLICT (wallet) DO UPDATE SET
       twitter_followed = TRUE,
       twitter_handle = $2,
       updated_at = NOW()`,
    [wallet, handle]
  );

  res.json({ success: true });
}

// POST /auth/discord/callback
export async function discordCallback(req: Request, res: Response) {
  const { wallet, discordId } = req.body;

  await db.query(
    `INSERT INTO social_verification (wallet, discord_id, discord_joined)
     VALUES ($1, $2, TRUE)
     ON CONFLICT (wallet) DO UPDATE SET
       discord_joined = TRUE,
       discord_id = $2,
       updated_at = NOW()`,
    [wallet, discordId]
  );

  res.json({ success: true });
}
```

### Option B: On-Demand Verification (Fallback)

If OAuth not yet implemented, manually update or use an external service:

```typescript
// Example: Check Twitter API
async function checkTwitterFollower(handle: string): Promise<boolean> {
  // Call Twitter API with your credentials
  // Return whether wallet's associated handle follows @twzrd_xyz
  // This is placeholder; implement based on your Twitter integration
  return true;
}

// Example: Check Discord membership
async function checkDiscordMembership(userId: string): Promise<boolean> {
  // Call Discord API to check guild membership
  // This is placeholder; implement based on your Discord integration
  return true;
}
```

---

## 📝 Environment Variables

```bash
# Solana
SOLANA_RPC=https://api.mainnet-beta.solana.com
PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
FEEPAYER_PUBKEY=<optional fee payer address>

# Database
DATABASE_URL=postgresql://user:password@localhost:5432/twzrd

# External APIs
TWITTER_API_KEY=<your Twitter API key>
TWITTER_API_SECRET=<your Twitter API secret>
DISCORD_BOT_TOKEN=<your Discord bot token>
DISCORD_GUILD_ID=<your Discord guild ID>

# Security (optional)
API_SECRET_KEY=<random secret for signing/validation>
```

---

## 🧪 Testing Checklist

### Unit Tests

```typescript
// test/api/verification-status.test.ts
describe('GET /api/verification-status', () => {
  it('should return false for unknown wallet', async () => {
    const res = await request(app).get('/api/verification-status?wallet=Abcd...');
    expect(res.status).toBe(200);
    expect(res.body.twitterFollowed).toBe(false);
  });

  it('should return 400 for invalid wallet', async () => {
    const res = await request(app).get('/api/verification-status?wallet=invalid');
    expect(res.status).toBe(400);
  });

  it('should return verified status for registered wallet', async () => {
    // Insert test data
    await db.query('INSERT INTO social_verification ...');
    const res = await request(app).get('/api/verification-status?wallet=...');
    expect(res.status).toBe(200);
    expect(res.body.twitterFollowed).toBe(true);
  });
});

// test/api/claim-cls.test.ts
describe('POST /api/claim-cls', () => {
  it('should return 400 for missing wallet', async () => {
    const res = await request(app).post('/api/claim-cls').send({ epochId: 0 });
    expect(res.status).toBe(400);
  });

  it('should return 403 if not verified', async () => {
    const res = await request(app)
      .post('/api/claim-cls')
      .send({ wallet: '...', epochId: 0 });
    expect(res.status).toBe(403);
  });

  it('should return transaction for verified, unclaimed wallet', async () => {
    // Setup test data
    const res = await request(app)
      .post('/api/claim-cls')
      .send({ wallet: '...', epochId: 0 });
    expect(res.status).toBe(200);
    expect(res.body.transaction).toBeTruthy();
    expect(typeof res.body.transaction).toBe('string');
  });
});
```

### Manual Testing

1. Create test wallet
2. Insert into `social_verification` with both flags true
3. Insert test epoch with merkle root
4. Call POST /api/claim-cls
5. Decode returned base64 transaction
6. Verify accounts and data

---

## 🚨 Error Handling

| Status | Scenario | Response |
|--------|----------|----------|
| 200 | Success | `{ transaction, signature }` |
| 400 | Bad request (invalid wallet, missing epochId, epoch not found, closed) | `{ error: "..." }` |
| 403 | Verification not satisfied | `{ error: "...", details: "..." }` |
| 409 | Already claimed | `{ error: "Already claimed for this epoch" }` |
| 500 | Server error | `{ error: "Internal server error" }` |

---

## 📦 Dependencies

```json
{
  "dependencies": {
    "express": "^4.18.2",
    "@solana/web3.js": "^1.91.0",
    "@solana/spl-token": "^0.4.8",
    "@coral-xyz/anchor": "^0.30.1",
    "bs58": "^5.0.0",
    "pg-promise": "^11.5.0",
    "dotenv": "^16.3.1"
  }
}
```

---

## 🔐 Security Notes

1. **Validate all inputs** - wallet format, epochId type, etc.
2. **Rate limit** - Consider rate limiting these endpoints
3. **Database** - Use parameterized queries (never concatenate)
4. **Secrets** - Keep FEEPAYER_PUBKEY, API keys in .env
5. **CORS** - Configure appropriately for portal-v3 domain
6. **Logging** - Log errors for monitoring/debugging
7. **Transaction signing** - Never sign on backend (let client sign)

---

## 🚀 Deployment Checklist

- [ ] Database migrations run (tables created)
- [ ] Environment variables set
- [ ] Dependencies installed (`npm install`)
- [ ] TypeScript compiles (`npm run build`)
- [ ] Tests pass (`npm run test`)
- [ ] Endpoints respond (manual test)
- [ ] Error handling works
- [ ] Logging configured
- [ ] CORS headers set
- [ ] Rate limiting enabled (optional)
- [ ] Monitor errors in production

---

## 📞 Integration with Portal v3

Portal v3 will call:

```typescript
// 1) On wallet connect
const status = await fetch(
  `/api/verification-status?wallet=${publicKey.toBase58()}`
).then(r => r.json());

// 2) When user clicks "Claim CLS Tokens"
const response = await fetch('/api/claim-cls', {
  method: 'POST',
  body: JSON.stringify({ wallet: publicKey.toBase58(), epochId: 42 })
}).then(r => r.json());

// 3) Decode & send transaction
const tx = Transaction.from(Buffer.from(response.transaction, 'base64'));
const sig = await sendTransaction(tx, connection);
```

---

## 🔄 Future Enhancements

1. **Passport tiers** - Extend social_verification with passport lookup
2. **Merkle proofs** - Store/verify merkle proofs for claims
3. **Webhooks** - Listen for on-chain confirmation, update cls_claims status
4. **Analytics** - Track claims by epoch, verify success rate
5. **Rate limiting** - Prevent spam
6. **Caching** - Cache verification status with TTL

---

**Status**: Specification complete. Ready for implementation.

**Owner**: Agent B

**Timeline**: 1-2 weeks (depending on existing infrastructure)
