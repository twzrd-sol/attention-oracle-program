# Backend Quick Start – Agent B

**Goal**: Implement `/api/verification-status` and `/api/claim-cls` for Portal v3

**Time**: 1-2 weeks

---

## 📋 Pre-Check

Before starting, ensure you have:

- [ ] Node.js 18+ installed
- [ ] PostgreSQL running (or SQLite)
- [ ] Solana RPC endpoint available
- [ ] Access to Attention Oracle program ID
- [ ] (Optional) Twitter API credentials
- [ ] (Optional) Discord API credentials

---

## 🚀 Getting Started (30 min)

### 1. Install Dependencies

```bash
cd gateway
npm install express cors dotenv pg-promise @solana/web3.js bs58
npm install --save-dev typescript @types/express @types/node ts-node
```

### 2. Create .env File

```bash
cp .env.example .env
```

Edit `.env`:

```bash
# Solana
SOLANA_RPC=https://api.mainnet-beta.solana.com
PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
FEEPAYER_PUBKEY=<optional>

# Database
DATABASE_URL=postgresql://user:password@localhost:5432/twzrd

# Server
PORT=5000
NODE_ENV=development
ALLOWED_ORIGINS=http://localhost:3000,http://localhost:5000
```

### 3. Create Database

```bash
# Using psql
createdb twzrd

# Run migrations
psql -d twzrd -f migrations/001_create_tables.sql

# Verify tables created
psql -d twzrd -c "\dt"
```

### 4. Initialize Skeleton Code

Copy provided files:

```bash
# Database setup
src/db.ts
src/db/index.ts

# API endpoints
src/api/verification-status.ts
src/api/claim-cls.ts
src/api/routes.ts

# On-chain
src/onchain/claim-transaction.ts

# Main app
src/app.ts
src/index.ts
```

### 5. Update index.ts

Create `src/index.ts`:

```typescript
import 'dotenv/config';
import { startServer } from './app';

const port = parseInt(process.env.PORT || '5000', 10);
startServer(port);
```

### 6. Add npm Scripts

Update `package.json`:

```json
{
  "scripts": {
    "dev": "ts-node src/index.ts",
    "build": "tsc",
    "start": "node dist/index.js",
    "test": "jest",
    "migrate": "psql -d twzrd -f migrations/001_create_tables.sql"
  }
}
```

### 7. Run Dev Server

```bash
npm run dev
```

Expected output:

```
╔═══════════════════════════════════════════════════════════╗
║  TWZRD Gateway - Portal v3 Ready                          ║
╠═══════════════════════════════════════════════════════════╣
║  Server running at http://localhost:5000
║  Portal v3: http://localhost:5000
║  API: http://localhost:5000/api/
╚═══════════════════════════════════════════════════════════╝
```

---

## 📝 Implementation Checklist

### Phase 1: Database & Endpoints (Day 1-2)

- [ ] Database migrations run
- [ ] Table creation verified (`psql \dt`)
- [ ] GET /api/verification-status implemented
  - [ ] Returns 200 for unknown wallet
  - [ ] Returns 400 for invalid wallet
  - [ ] Queries social_verification table
  - [ ] Returns correct response format
- [ ] POST /api/claim-cls implemented
  - [ ] Validates request body
  - [ ] Checks epoch exists
  - [ ] Checks verification satisfied
  - [ ] Checks one-claim-per-epoch
  - [ ] Returns 400/403/409 as appropriate

### Phase 2: On-Chain Integration (Day 3-5)

- [ ] buildClaimTransaction implemented
  - [ ] Option A: Anchor client with IDL, OR
  - [ ] Option B: Manual instruction construction
- [ ] PDA derivation working
- [ ] Transaction serialization to base64 working

### Phase 3: Verification Integration (Day 6-7)

- [ ] OAuth callbacks for Twitter follow
- [ ] OAuth callbacks for Discord join
- [ ] social_verification table updates
- [ ] Webhook listener for verification events (optional)

### Phase 4: Testing (Day 8)

- [ ] Unit tests passing
- [ ] Manual API tests passing
- [ ] Devnet end-to-end claim working
- [ ] Error handling tested

### Phase 5: Production Hardening (Day 9-10)

- [ ] Rate limiting added
- [ ] Logging configured
- [ ] Error monitoring set up
- [ ] Security review passed
- [ ] Performance optimized

---

## 🧪 Testing Endpoints (Manual)

### Test GET /api/verification-status

**Unknown wallet (should be all false):**

```bash
curl "http://localhost:5000/api/verification-status?wallet=Abcd123..."
```

Expected:

```json
{
  "twitterFollowed": false,
  "discordJoined": false,
  "passportTier": null,
  "lastVerified": null
}
```

**Invalid wallet (should be 400):**

```bash
curl "http://localhost:5000/api/verification-status?wallet=invalid"
```

Expected:

```json
{
  "error": "Invalid wallet public key (not valid base58)"
}
```

### Test POST /api/claim-cls

**Missing body (should be 400):**

```bash
curl -X POST http://localhost:5000/api/claim-cls
```

**Invalid epoch (should be 400):**

```bash
curl -X POST http://localhost:5000/api/claim-cls \
  -H "Content-Type: application/json" \
  -d '{"wallet": "So11111...", "epochId": 999}'
```

**Successful claim (should be 200 with transaction):**

```bash
# First: Setup test data
psql -d twzrd << EOF
INSERT INTO social_verification (wallet, twitter_followed, discord_joined)
  VALUES ('So11111111111111111111111111111111111111112', TRUE, TRUE);

INSERT INTO epochs (epoch_id, merkle_root, is_open)
  VALUES (0, 'aabbccdd...', TRUE);
EOF

# Then: Claim
curl -X POST http://localhost:5000/api/claim-cls \
  -H "Content-Type: application/json" \
  -d '{
    "wallet": "So11111111111111111111111111111111111111112",
    "epochId": 0
  }'
```

Expected:

```json
{
  "transaction": "AgAB...",
  "signature": null
}
```

---

## 🏗️ Database Setup (SQL)

If you want to manually set up tables without migrations:

```sql
-- Create social_verification
CREATE TABLE social_verification (
  wallet TEXT PRIMARY KEY,
  twitter_handle TEXT,
  twitter_followed BOOLEAN DEFAULT FALSE,
  discord_id TEXT,
  discord_joined BOOLEAN DEFAULT FALSE,
  passport_tier INTEGER,
  last_verified TIMESTAMPTZ DEFAULT NOW(),
  created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Create epochs
CREATE TABLE epochs (
  epoch_id INTEGER PRIMARY KEY,
  merkle_root TEXT NOT NULL,
  is_open BOOLEAN DEFAULT TRUE,
  created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Create cls_claims
CREATE TABLE cls_claims (
  id BIGSERIAL PRIMARY KEY,
  wallet TEXT NOT NULL,
  epoch_id INTEGER NOT NULL,
  amount BIGINT,
  tx_signature TEXT,
  tx_status VARCHAR(20) DEFAULT 'pending',
  created_at TIMESTAMPTZ DEFAULT NOW(),
  UNIQUE(wallet, epoch_id),
  FOREIGN KEY (wallet) REFERENCES social_verification(wallet)
);

-- Insert test data
INSERT INTO social_verification (wallet, twitter_followed, discord_joined)
VALUES ('Abcd1234567890abcd1234567890abcd1234567890abc', TRUE, TRUE);

INSERT INTO epochs (epoch_id, merkle_root, is_open)
VALUES (0, 'test_merkle_root_32_bytes_long____', TRUE);
```

---

## 🔑 Environment Variables

### Required

```bash
SOLANA_RPC=https://api.mainnet-beta.solana.com
PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
DATABASE_URL=postgresql://user:password@localhost:5432/twzrd
```

### Optional

```bash
PORT=5000
NODE_ENV=development
FEEPAYER_PUBKEY=<your fee payer pubkey>
ALLOWED_ORIGINS=http://localhost:3000,http://localhost:5000
TWITTER_API_KEY=...
TWITTER_API_SECRET=...
DISCORD_BOT_TOKEN=...
DISCORD_GUILD_ID=...
```

---

## 📚 Key Files to Implement

### Must Have

1. **src/db.ts** - Database connection (pg-promise)
   - Set up connection pool
   - Handle queries

2. **src/api/verification-status.ts** - GET endpoint
   - Validate wallet
   - Query database
   - Return response

3. **src/api/claim-cls.ts** - POST endpoint
   - Validate request
   - Check verification
   - Build transaction
   - Return base64

4. **src/onchain/claim-transaction.ts** - Transaction building
   - Implement buildClaimTransaction
   - Use Anchor or manual instruction building

5. **src/api/routes.ts** - Route setup
   - Register endpoints
   - Import handlers

6. **src/app.ts** - Express app
   - Middleware setup
   - Static serving
   - Error handling

### Nice to Have

1. **src/db/migrations.ts** - Run migrations programmatically
2. **src/auth/twitter.ts** - Twitter OAuth callback
3. **src/auth/discord.ts** - Discord OAuth callback
4. **src/onchain/listener.ts** - Listen for on-chain confirmation
5. **test/api.test.ts** - Unit tests

---

## 🐛 Common Issues

### Issue: "Cannot find module 'express'"

```bash
npm install express @types/express
```

### Issue: Database connection fails

```bash
# Check PostgreSQL is running
psql -U postgres -c "SELECT version();"

# Check DATABASE_URL format
# Should be: postgresql://user:password@localhost:5432/database
```

### Issue: "Epoch not found" error

```bash
# Insert test epoch
psql -d twzrd -c "INSERT INTO epochs (epoch_id, merkle_root, is_open) VALUES (0, 'test', TRUE);"
```

### Issue: "buildClaimTransaction not implemented"

```bash
# Implement either:
# 1. Use Anchor client with IDL
# 2. Manual TransactionInstruction construction
# See src/onchain/claim-transaction.ts for details
```

---

## 🚨 Security Checklist

Before deploying to production:

- [ ] Never log sensitive data (private keys, API keys)
- [ ] Use parameterized queries (no SQL injection)
- [ ] Validate all inputs (wallet format, epochId type)
- [ ] Rate limit endpoints
- [ ] Use HTTPS in production
- [ ] Keep secrets in .env (not in code)
- [ ] Never sign transactions on backend
- [ ] Add request/IP rate limiting
- [ ] Monitor error logs
- [ ] Add authentication if needed

---

## 🚀 Next Steps

1. **Implement verification-status.ts**
   - Already provided as skeleton
   - Just fill in database query logic

2. **Implement claim-cls.ts**
   - Already provided as skeleton
   - Add validation logic
   - Add database checks

3. **Implement buildClaimTransaction**
   - Choose Anchor OR manual approach
   - Test with devnet

4. **Add OAuth callbacks**
   - Twitter: POST /auth/twitter/callback
   - Discord: POST /auth/discord/callback

5. **Test end-to-end**
   - Run portal-v3 on localhost:3000
   - Run gateway on localhost:5000
   - Test claim flow

6. **Deploy to production**
   - Build: `npm run build`
   - Start: `npm run start`

---

## 📞 Questions?

Refer to:
- **BACKEND_SPEC.md** - Full specification with code examples
- **migrations/001_create_tables.sql** - Database schema
- **src/api/routes.ts** - Route definitions
- **src/onchain/claim-transaction.ts** - On-chain integration

---

**Status**: Ready to implement

**Owner**: Agent B

**Timeline**: 1-2 weeks
