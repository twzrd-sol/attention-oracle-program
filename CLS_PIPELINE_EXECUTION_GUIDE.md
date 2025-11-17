# CLS Pipeline: Ready-to-Execute Test Guide

**Status**: ✅ All Code Complete & Ready for Your Environment
**Goal**: 3 confirmed claims on Solana (devnet or mainnet) in 30 minutes
**Prerequisites**: DATABASE_URL, SOLANA_RPC, GATEWAY_URL set in your shell

---

## Pre-Flight Checklist

Before starting, ensure:

```bash
# 1. Environment variables set
echo $DATABASE_URL          # PostgreSQL connection
echo $SOLANA_RPC            # RPC endpoint (https://api.devnet.solana.com or mainnet)
echo $GATEWAY_URL           # Gateway base URL (http://localhost:5000)
echo $PROGRAM_ID            # GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
echo $MINT_PUBKEY           # Token mint (e.g., AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5)

# 2. Database has required tables
psql $DATABASE_URL -c "\dt" | grep -E "sealed_participants|weighted_participants|user_mapping|sealed_epochs|allocations|cls_claims"
# Should show all 6 tables

# 3. Gateway is running
curl -X POST $GATEWAY_URL/api/claim-cls \
  -H "Content-Type: application/json" \
  -d '{"wallet":"test","epochId":0}' 2>&1 | grep -q "error\|Invalid"
# Should get a validation error (not 404/connection refused)

# 4. RPC is responsive
curl -X POST $SOLANA_RPC \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getLatestBlockhash","params":[]}' | grep -q "result"
# Should return a blockhash
```

---

## The 6-Step Test Flow

### Step 1: Seed Synthetic Epoch (1 min)

**What it does**: Inserts 3 test users into sealed_participants/weighted_participants

```bash
cd /home/twzrd/milo-token
npx tsx scripts/test-cls-e2e-setup.ts
```

**Expected output**:
```
🧪 Setting up CLS end-to-end test data...

Channel: test-cls
Epoch: 424245

📝 Inserting user_mapping...
  ✓ alice-test → 0x...
  ✓ bob-test → 0x...
  ✓ charlie-test → 0x...

📝 Inserting sealed_participants...
  ✓ Index 0: alice-test
  ✓ Index 1: bob-test
  ✓ Index 2: charlie-test

📝 Inserting weighted_participants...
  ✓ alice-test: weight 10
  ✓ bob-test: weight 20
  ✓ charlie-test: weight 30

📝 Inserting sealed_epochs...
  ✓ Epoch 424245 for channel test-cls

🔑 Test Wallets & Keypairs (save for later):

wallet,epochs,keypair_path
<PUBKEY1>,424245,/tmp/test-cls-wallet-0.json
<PUBKEY2>,424245,/tmp/test-cls-wallet-1.json
<PUBKEY3>,424245,/tmp/test-cls-wallet-2.json
```

**Action**: Copy the wallet CSV output (last 3 lines) → will use in Step 4

---

### Step 2: Build Merkle Allocations (2 min)

**What it does**: Creates Merkle tree from weights, generates proofs, populates allocations table

```bash
npx tsx scripts/build-allocations-for-epoch.ts --channel test-cls --epoch 424245
```

**Expected output**:
```
🔨 Building allocations for test-cls epoch 424245...

  📊 Found 3 participants

  🌳 Building Merkle tree...
  ✅ Tree root: 0x<64-char-hex>

  💾 Inserting allocations...
    ✓ alice-test: 800000000000 tokens (weight 10)
    ✓ bob-test: 1600000000000 tokens (weight 20)
    ✓ charlie-test: 2400000000000 tokens (weight 30)
    ✓ sealed_epochs updated with root 0x<64-char-hex>

✅ Build complete!

   Summary:
   • Inserted: 3 allocations
   • Root: 0x<64-char-hex>
   • Ready for: npx tsx scripts/allocate-and-claim.ts --csv claims.csv
```

**Verify in DB**:
```sql
SELECT wallet, index, amount, id
FROM allocations
WHERE epoch_id = 424245
ORDER BY wallet;

-- Expected: 3 rows with amounts 800M, 1.6B, 2.4B tokens
```

---

### Step 3: Create Claims CSV (1 min)

**What it does**: Exports allocations as CSV for batch submission

Create file `scripts/claims.csv`:

```csv
wallet,epochs,keypair_path
<PUBKEY1>,424245,/tmp/test-cls-wallet-0.json
<PUBKEY2>,424245,/tmp/test-cls-wallet-1.json
<PUBKEY3>,424245,/tmp/test-cls-wallet-2.json
```

(Use the wallet output from Step 1)

**Verify**:
```bash
cat scripts/claims.csv
# Should show 3 wallets with 424245 and /tmp/test-cls-wallet-*.json paths
```

---

### Step 4: Submit Claims (15-20 min)

**What it does**: Signs transactions with test keypairs and submits to Solana

```bash
npx tsx scripts/allocate-and-claim.ts --csv scripts/claims.csv
```

**Expected output** (for each wallet):
```
===== Epoch 424245 / Wallet <PUBKEY1> =====

  Allocation:
   • index  = 0
   • amount = 800000000000
   • id     = twitch:test-cls:alice-test
   • proof  = 2 nodes

  ➜ POST /api/claim-cls
  ✅ Received unsigned transaction

  ➜ Submitting to Solana...
  ✅ Submitted. Signature: <TX_SIG_1>
     Explorer: https://explorer.solana.com/tx/<TX_SIG_1>

  ✅ Transaction confirmed on-chain

  📝 cls_claims updated to confirmed
```

**Timeline**:
- ~5 seconds per transaction (includes on-chain confirmation)
- 3 wallets = ~15-20 seconds total submission
- Then waits for finality confirmation before updating DB

**Three signatures will be printed** → copy these for Step 5 verification

---

### Step 5: Verify Database State (2 min)

```bash
psql $DATABASE_URL << 'EOF'
SELECT
  wallet,
  epoch_id,
  amount,
  tx_status,
  tx_signature,
  confirmed_at
FROM cls_claims
WHERE epoch_id = 424245
ORDER BY wallet;
EOF
```

**Expected**:
```
                    wallet                    | epoch_id |      amount      | tx_status |                  tx_signature                  |        confirmed_at
-----------------------------------------------+----------+------------------+-----------+------------------------------------------------+----------------------------
 <PUBKEY1> | 424245   | 800000000000     | confirmed | <SIG1>                                         | 2025-11-17 12:34:56+00
 <PUBKEY2> | 424245   | 1600000000000    | confirmed | <SIG2>                                         | 2025-11-17 12:34:57+00
 <PUBKEY3> | 424245   | 2400000000000    | confirmed | <SIG3>                                         | 2025-11-17 12:34:58+00
```

**Success criteria**:
- ✅ 3 rows with status='confirmed'
- ✅ 3 non-null tx_signature values
- ✅ confirmed_at within last 5 minutes

---

### Step 6: Verify On-Chain (5 min)

For each transaction signature, check:

**Via Solana CLI**:
```bash
# 1. Confirm finalization
solana confirm <TX_SIG_1> --url $SOLANA_RPC
# → "Finalized" (or "Confirmed" on devnet)

# 2. Check transaction details
solana transaction-history <WALLET_A> --url $SOLANA_RPC --limit 1
# → Should show the claim transaction
```

**Via Solana Explorer**:
```
https://explorer.solana.com/tx/<TX_SIG_1>?cluster=<devnet|mainnet>
```

Look for:
- **Program**: GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop (claim_open instruction)
- **Status**: "Success" ✅
- **From**: Treasury ATA
- **To**: Claimer ATA
- **Amount**: Matches claimed amount (800M, 1.6B, or 2.4B)

**Token balance check**:
```bash
# Before claim (would be 0)
solana token accounts --owner <PUBKEY1> --mint $MINT_PUBKEY --url $SOLANA_RPC

# After claim should show the transferred amount
```

---

## Failure Recovery

### If Step 2 fails: "No participants found"
```bash
# Verify Step 1 completed
psql $DATABASE_URL -c "SELECT * FROM sealed_participants WHERE channel='test-cls'"
# Should show 3 rows

# Re-run Step 1
npx tsx scripts/test-cls-e2e-setup.ts
```

### If Step 4 fails: "Invalid proof for epoch"
```bash
# Check sealed_epochs root matches allocations
psql $DATABASE_URL << 'EOF'
SELECT
  (SELECT root FROM sealed_epochs WHERE epoch=424245 AND channel='test-cls') as sealed_root,
  (SELECT root FROM l2_tree_cache WHERE epoch=424245 AND channel='test-cls') as tree_cache_root;
EOF

# Re-run Step 2 to sync root
npx tsx scripts/build-allocations-for-epoch.ts --channel test-cls --epoch 424245
```

### If Step 4 fails: "Wallet mismatch"
```bash
# Verify keypair files exist and match
ls -la /tmp/test-cls-wallet-*.json

# Test keypair validity
node -e "
const fs = require('fs');
const { Keypair } = require('@solana/web3.js');
const secret = JSON.parse(fs.readFileSync('/tmp/test-cls-wallet-0.json', 'utf8'));
const kp = Keypair.fromSecretKey(Uint8Array.from(secret));
console.log('Public key:', kp.publicKey.toBase58());
"

# Should match wallet from Step 1 output
```

### If Step 4 fails: "Gateway responded with 500"
```bash
# Check gateway logs
pm2 logs gateway | tail -50

# Restart gateway
pm2 restart gateway

# Retry Step 4
npx tsx scripts/allocate-and-claim.ts --csv scripts/claims.csv
```

---

## Success Metrics

| Step | Component | Success Criteria |
|------|-----------|-----------------|
| 1 | Test Setup | 3 users inserted, keypairs created |
| 2 | Allocation Build | 3 allocations with correct amounts, root updated |
| 3 | CSV Generation | 3 wallets, 424245 epochs, valid keypair paths |
| 4 | Claim Submission | 3 tx signatures returned, all confirmed on-chain |
| 5 | DB Verification | 3 cls_claims rows, status='confirmed' |
| 6 | On-Chain Verification | 3 tokens transferred, visible on explorer |

**Final Success**: All 6 steps complete, 3 confirmed claims visible in both DB and explorer.

---

## Timeline Breakdown

| Phase | Task | Time |
|-------|------|------|
| Setup | Verify env, check DB/gateway/RPC | 2 min |
| 1 | Seed test epoch | 1 min |
| 2 | Build allocations | 2 min |
| 3 | Create CSV | 1 min |
| 4 | Submit & confirm claims | 15-20 min |
| 5 | DB verification | 2 min |
| 6 | Explorer verification | 5 min |
| **Total** | | **30 min** |

---

## Files You'll Use

| File | Purpose | Step(s) |
|------|---------|---------|
| `scripts/test-cls-e2e-setup.ts` | Insert synthetic epoch | 1 |
| `scripts/build-allocations-for-epoch.ts` | Build Merkle tree | 2 |
| `scripts/claims.csv` | Batch submission input | 3, 4 |
| `scripts/allocate-and-claim.ts` | Submit claims | 4 |
| `CLS_E2E_TEST_RUNBOOK.md` | Detailed reference | All |
| `CLS_ALLOCATION_PIPELINE_SUMMARY.md` | Architecture reference | All |

---

## Data Schema Reference

### Input Tables (populated by Step 1)
```sql
sealed_participants(epoch, channel, idx, user_hash, username)
weighted_participants(channel, epoch, user_hash, weight)
user_mapping(user_hash, username, first_seen)
sealed_epochs(epoch, channel, root, sealed_at, published)
```

### Output Tables (populated by Step 2 & 4)
```sql
allocations(epoch_id, wallet, index, amount, id, proof_json)
cls_claims(wallet, epoch_id, amount, tx_status, tx_signature, confirmed_at)
```

---

## Command Cheat Sheet

```bash
# Pre-flight
env | grep DATABASE_URL
env | grep SOLANA_RPC
env | grep GATEWAY_URL

# Step 1
npx tsx scripts/test-cls-e2e-setup.ts

# Step 2
npx tsx scripts/build-allocations-for-epoch.ts --channel test-cls --epoch 424245

# Step 3
cat > scripts/claims.csv << 'EOF'
wallet,epochs,keypair_path
<PUBKEY1>,424245,/tmp/test-cls-wallet-0.json
<PUBKEY2>,424245,/tmp/test-cls-wallet-1.json
<PUBKEY3>,424245,/tmp/test-cls-wallet-2.json
EOF

# Step 4
npx tsx scripts/allocate-and-claim.ts --csv scripts/claims.csv

# Step 5
psql $DATABASE_URL -c "SELECT wallet, epoch_id, tx_status, tx_signature FROM cls_claims WHERE epoch_id=424245 ORDER BY wallet;"

# Step 6
# Open in browser or use solana CLI:
solana confirm <TX_SIG> --url $SOLANA_RPC
```

---

## Next Steps After Success

Once all 3 claims confirm:

1. **Document weight scheme**: What does weight=10 mean in your context? (engagement hours? bits? etc.)
2. **Real channel test**: Repeat with actual Twitch channel data (not synthetic weights)
3. **Scale test**: 100+ claims in single epoch to validate batch performance
4. **Automation**: Wire allocation builder into hourly tree-builder cronjob
5. **Monitoring**: Track claim success rate, proof validation failures, confirmation latency

---

## Architecture Reference

```
Sealed Data (from IRC aggregator)
    ↓
[sealed_participants] + [weighted_participants] + [user_mapping]
    ↓
build-allocations-for-epoch.ts
    ↓
    Merkle tree + proofs per wallet
    ↓
[allocations] table
    ↓
allocate-and-claim.ts (reads CSV)
    ↓
/api/claim-cls (gateway)
    ↓
    Local proof validation
    ↓
buildClaimTransaction
    ↓
Solana RPC
    ↓
claim_open instruction executed
    ↓
[cls_claims] updated to 'confirmed'
```

---

## Ready? Go!

```bash
# 1. Verify environment
env | grep -E "DATABASE_URL|SOLANA_RPC|GATEWAY_URL"

# 2. Run the test
npx tsx scripts/test-cls-e2e-setup.ts && \
  npx tsx scripts/build-allocations-for-epoch.ts --channel test-cls --epoch 424245 && \
  # (create claims.csv from output) && \
  npx tsx scripts/allocate-and-claim.ts --csv scripts/claims.csv

# 3. Verify
psql $DATABASE_URL -c "SELECT COUNT(*), COUNT(CASE WHEN tx_status='confirmed' THEN 1 END) FROM cls_claims WHERE epoch_id=424245;"
# Expected: 3, 3
```

**Good luck!** 🚀

