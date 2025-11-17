# CLS End-to-End Test Runbook

**Objective**: Validate the complete CLS claim pipeline in 30 minutes
- Insert mock sealed participants (3 test users with weights 10, 20, 30)
- Generate Merkle allocations from weights
- Batch submit claims via gateway
- Verify on-chain and in database

**Target**: 3 successful claims on mainnet (or devnet)

---

## Prerequisites

```bash
# Environment variables
export DATABASE_URL="postgresql://user:pass@localhost/attention_oracle"
export SOLANA_RPC="https://api.mainnet-beta.solana.com"
export GATEWAY_URL="http://localhost:5000"
export PROGRAM_ID="GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
export MINT_PUBKEY="AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5"

# Ensure these tables exist:
# - sealed_participants (epoch, channel, idx, user_hash, username)
# - weighted_participants (channel, epoch, user_hash, weight)
# - user_mapping (user_hash, username, first_seen)
# - sealed_epochs (epoch, channel, root, sealed_at, published)
# - allocations (epoch_id, wallet, index, amount, id, proof_json)
# - cls_claims (wallet, epoch_id, tx_status, tx_signature, confirmed_at)

# Gateway running
pm2 list | grep gateway
# If not running: cd gateway && npm run dev
```

---

## Step 1: Create Test Data (2 min)

```bash
cd /home/twzrd/milo-token

npx tsx scripts/test-cls-e2e-setup.ts
```

**Output**: Inserts 3 test users (alice-test, bob-test, charlie-test) with weights 10, 20, 30.

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
<WALLET_A>,424245,/tmp/test-cls-wallet-0.json
<WALLET_B>,424245,/tmp/test-cls-wallet-1.json
<WALLET_C>,424245,/tmp/test-cls-wallet-2.json

✅ Test data inserted successfully!

Next steps:
1. Run: npx tsx scripts/build-allocations-for-epoch.ts --channel test-cls --epoch 424245
2. Create claims.csv with the wallets above
3. Run: npx tsx scripts/allocate-and-claim.ts --csv claims.csv
```

**Copy the wallet addresses** for the next step.

---

## Step 2: Build Allocations (3 min)

Build the Merkle tree and allocations from sealed participants:

```bash
npx tsx scripts/build-allocations-for-epoch.ts --channel test-cls --epoch 424245
```

**Output**:

```
🔨 Building allocations for test-cls epoch 424245...

  📊 Found 3 participants

  🌳 Building Merkle tree...
  ✅ Tree root: 0x...

  💾 Inserting allocations...
    ✓ alice-test: 800000000000 tokens (weight 10)
    ✓ bob-test: 1600000000000 tokens (weight 20)
    ✓ charlie-test: 2400000000000 tokens (weight 30)
    ✓ sealed_epochs updated with root 0x...

✅ Build complete!

   Summary:
   • Inserted: 3 allocations
   • Root: 0x...
   • Ready for: npx tsx scripts/allocate-and-claim.ts --csv claims.csv
```

**Verify in DB**:

```sql
SELECT epoch_id, wallet, index, amount, id, proof_json
FROM allocations
WHERE epoch_id = 424245
ORDER BY wallet;

-- Expected output: 3 rows with
-- - alice: amount 800000000000 (weight 10 * 80 * 10^9)
-- - bob: amount 1600000000000 (weight 20 * 80 * 10^9)
-- - charlie: amount 2400000000000 (weight 30 * 80 * 10^9)

SELECT root FROM sealed_epochs WHERE epoch = 424245 AND channel = 'test-cls';
-- Should match the root printed above
```

---

## Step 3: Generate Claims CSV (1 min)

```bash
npx tsx scripts/generate-claims-csv.ts --epoch 424245 --output claims.csv
```

**Output**:

```
📋 Generating claims.csv for epoch 424245...

  Found 3 wallets

✅ Written 3 claims to claims.csv

Preview:
wallet,epochs,keypair_path
<WALLET_A>,424245,/tmp/test-cls-wallet-0.json
<WALLET_B>,424245,/tmp/test-cls-wallet-1.json
<WALLET_C>,424245,/tmp/test-cls-wallet-2.json

Next: npx tsx scripts/allocate-and-claim.ts --csv claims.csv
```

**Verify**:

```bash
cat claims.csv
# Should show 3 wallets with epoch 424245 and keypair paths
```

---

## Step 4: Submit Claims (15-20 min)

Submit all 3 claims via the gateway:

```bash
npx tsx scripts/allocate-and-claim.ts --csv claims.csv
```

**Output** (for each claim):

```
===== Epoch 424245 / Wallet <WALLET_A> =====

  Allocation:
   • index  = 0
   • amount = 800000000000
   • id     = twitch:test-cls:alice-test
   • proof  = 2 nodes

  ➜ POST /api/claim-cls
  ✅ Received unsigned transaction

  ➜ Submitting to Solana...
  ✅ Submitted. Signature: <SIG_A>
     Explorer: https://explorer.solana.com/tx/<SIG_A>

  ✅ Transaction confirmed on-chain

  📝 cls_claims updated to confirmed
```

**Expected outcomes**:
- 3 transaction signatures (one per wallet)
- All transactions show "confirmed" on explorer
- Transaction logs should include "TransferTokensWithFee" event

**If a claim fails**:
- Check gateway is running: `curl http://localhost:5000/api/claim-cls` → 405 (expected, POST only)
- Check keypair files exist: `ls -la /tmp/test-cls-wallet-*.json`
- Check MINT_PUBKEY and treasury ATA initialized: `npx tsx scripts/init-gng-treasury-ata.ts`
- Check allocation proof is valid: look at proof_json in allocations table

---

## Step 5: Verify Results (5 min)

### On-Chain Verification

For each signature, check:

```bash
# 1. Check transaction status
solana confirm <SIG_A> --url https://api.mainnet-beta.solana.com
# → Should show "Finalized"

# 2. Check token transfer in transaction logs
curl -X POST https://api.mainnet-beta.solana.com -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getTransaction","params":["<SIG_A>"]}'
# Look for "TransferTokensWithFee" or "Transfer" events

# 3. Check claimer token balance
solana token accounts --owner <WALLET_A> --mint <MINT_PUBKEY> -um
# → Balance should increase by claimed amount
```

### Database Verification

```sql
-- Check cls_claims status
SELECT wallet, epoch_id, tx_status, tx_signature, amount, confirmed_at
FROM cls_claims
WHERE epoch_id = 424245
ORDER BY wallet;

-- Expected: 3 rows with status='confirmed' and non-null tx_signature

-- Check allocations were consumed
SELECT epoch_id, wallet, amount, proof_json
FROM allocations
WHERE epoch_id = 424245
ORDER BY wallet;

-- Expected: 3 rows with amounts 800M, 1.6B, 2.4B tokens
```

### Summary Query

```sql
-- All-in-one verification
SELECT
  a.wallet,
  a.amount,
  c.tx_status,
  c.tx_signature,
  c.confirmed_at
FROM allocations a
LEFT JOIN cls_claims c
  ON a.epoch_id = c.epoch_id AND a.wallet = c.wallet
WHERE a.epoch_id = 424245
ORDER BY a.wallet;

-- Expected: 3 rows, all with status='confirmed'
```

---

## Troubleshooting

### Allocation Builder Fails

**Problem**: `Require --channel <name>` error

**Fix**: Use exact argument format:
```bash
npx tsx scripts/build-allocations-for-epoch.ts --channel test-cls --epoch 424245
# OR
npx tsx scripts/build-allocations-for-epoch.ts -c test-cls -e 424245
```

**Problem**: `No participants found`

**Fix**: Check sealed_participants table:
```sql
SELECT * FROM sealed_participants WHERE channel = 'test-cls' AND epoch = 424245;
-- Should show 3 rows
```

### Claims Submit Fail

**Problem**: `Gateway responded with 400: Invalid proof for epoch`

**Fix**:
1. Check allocations table has correct root:
   ```sql
   SELECT proof_json FROM allocations WHERE epoch_id = 424245 LIMIT 1;
   -- Should be valid JSON array of 64-char hex strings
   ```

2. Check sealed_epochs root matches:
   ```sql
   SELECT a.epoch_id, COUNT(*) as allocs, se.root
   FROM allocations a
   LEFT JOIN sealed_epochs se ON se.epoch = a.epoch_id AND se.channel = 'test-cls'
   WHERE a.epoch_id = 424245
   GROUP BY a.epoch_id, se.root;
   ```

3. Re-run build-allocations (idempotent):
   ```bash
   npx tsx scripts/build-allocations-for-epoch.ts --channel test-cls --epoch 424245
   ```

**Problem**: `Wallet mismatch: wallet=... but keypair pubkey=...`

**Fix**: Keypair files must match wallet addresses. Check:
```bash
# Read keypair and verify public key
node -e "const k = require('fs').readFileSync('/tmp/test-cls-wallet-0.json'); const pk = new (require('@solana/web3.js').Keypair)(Uint8Array.from(JSON.parse(k))).publicKey.toBase58(); console.log(pk);"

# Should match wallet from claims.csv
```

### Transactions Don't Confirm

**Problem**: Transaction submitted but never confirms

**Fix**:
1. Check RPC is responsive:
   ```bash
   curl -X POST $SOLANA_RPC \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","id":1,"method":"getLatestBlockhash","params":[]}'
   ```

2. Check wallet has SOL for fees:
   ```bash
   solana balance <WALLET_A> --url $SOLANA_RPC
   # Should be > 0.01 SOL
   ```

3. Check program is deployed:
   ```bash
   solana program show $PROGRAM_ID --url $SOLANA_RPC
   ```

---

## Timeline Expectations

| Step | Task | Time | Output |
|------|------|------|--------|
| 1 | Insert test data | 2 min | 3 users, sealed_participants, weights |
| 2 | Build allocations | 3 min | Merkle tree, root, 3 allocation rows |
| 3 | Generate CSV | 1 min | claims.csv with 3 wallets |
| 4 | Submit claims | 15-20 min | 3 transaction signatures |
| 5 | Verify results | 5 min | DB confirmations, explorer checks |
| **Total** | | **30 min** | **3 verified claims** |

---

## Success Criteria

✅ All 3 test users inserted into database
✅ Merkle tree root calculated and stored in sealed_epochs
✅ 3 allocations rows with correct amounts (800M, 1.6B, 2.4B)
✅ 3 claims submitted to /api/claim-cls and confirmed
✅ 3 transaction signatures visible on explorer
✅ 3 cls_claims rows marked as "confirmed" in database
✅ Claimer token balances increased by claimed amounts

---

## Next Steps After Success

1. **Scale to real channel**: Replace `test-cls` with real Twitch channel name
2. **Real engagement data**: Feed actual weighted_participants from IRC aggregator
3. **Automate**: Wire allocation builder into hourly tree-builder cronjob
4. **Monitor**: Track claim confirmation rates and proof validation errors
5. **Document**: Update allocator README with real-world weight schemes

---

## Key Files

| File | Purpose |
|------|---------|
| `scripts/test-cls-e2e-setup.ts` | Insert test sealed_participants/weights |
| `scripts/build-allocations-for-epoch.ts` | Transform sealed data → allocations |
| `scripts/generate-claims-csv.ts` | Export allocations as CSV for submission |
| `scripts/allocate-and-claim.ts` | Batch claim submission via gateway |
| `CLS_MAINNET_LAUNCH_GUIDE.md` | Production deployment guide |
| `CLS_E2E_TEST_RUNBOOK.md` | This file |

