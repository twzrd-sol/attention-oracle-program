# CLS Devnet Validation Checklist

**Target**: Validate full end-to-end CLS pipeline before mainnet
**Timeline**: ~30 minutes
**Environment**: Devnet (not mainnet)

---

## Pre-Flight (5 min)

- [ ] Devnet RPC reachable
  ```bash
  curl -X POST https://api.devnet.solana.com \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","id":1,"method":"getLatestBlockhash","params":[]}'
  # Should return a blockhash
  ```

- [ ] Dev database accessible
  ```bash
  psql $DATABASE_URL -c "SELECT version();"
  # Should show PostgreSQL version
  ```

- [ ] Gateway running on devnet
  ```bash
  curl -X POST $GATEWAY_URL/api/claim-cls \
    -H "Content-Type: application/json" \
    -d '{"wallet":"test","epochId":0}' 2>&1 | grep "error\|Invalid"
  # Should get validation error (not 404)
  ```

- [ ] Test wallet has SOL
  ```bash
  solana balance --url https://api.devnet.solana.com
  # Should have > 1 SOL (for test transactions)
  ```

---

## Test Execution (25 min)

### Step 1: Seed Test Epoch (2 min)
```bash
npx tsx scripts/test-cls-e2e-setup.ts
```
- [ ] 3 users inserted into sealed_participants
- [ ] Weights (10, 20, 30) in weighted_participants
- [ ] Keypair files created in /tmp/test-cls-wallet-*.json
- [ ] CSV output printed (copy for Step 3)

### Step 2: Build Allocations (2 min)
```bash
npx tsx scripts/build-allocations-for-epoch.ts --channel test-cls --epoch 424245
```
- [ ] Merkle tree built successfully
- [ ] 3 allocations inserted (epochs_id=424245)
- [ ] Root updated in sealed_epochs
- [ ] Amounts correct:
  - alice-test: 800,000,000,000 (weight 10 × 80 × 10^9)
  - bob-test: 1,600,000,000,000 (weight 20 × 80 × 10^9)
  - charlie-test: 2,400,000,000,000 (weight 30 × 80 × 10^9)

**Verify in DB**:
```sql
SELECT wallet, amount, proof_json FROM allocations
WHERE epoch_id=424245 ORDER BY wallet;
```
- [ ] 3 rows
- [ ] Proof is valid JSON array (64-char hex strings)

### Step 3: Create CSV (1 min)
Create `scripts/claims.csv`:
```csv
wallet,epochs,keypair_path
<PUBKEY1>,424245,/tmp/test-cls-wallet-0.json
<PUBKEY2>,424245,/tmp/test-cls-wallet-1.json
<PUBKEY3>,424245,/tmp/test-cls-wallet-2.json
```

- [ ] File created and readable
- [ ] 3 wallet addresses (from Step 1)
- [ ] Keypair paths exist

### Step 4: Submit Claims (15-20 min)
```bash
npx tsx scripts/allocate-and-claim.ts --csv scripts/claims.csv
```

**Per-claim verification**:
- [ ] "Received unsigned transaction" message
- [ ] "Submitted" message with signature
- [ ] "Transaction confirmed on-chain" message
- [ ] "cls_claims updated to confirmed" message

**After all 3**:
- [ ] 3 transaction signatures printed
- [ ] Copy signatures for Step 5

---

## Post-Flight Verification (5 min)

### Step 5: Database Verification
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

**Verification**:
- [ ] 3 rows returned
- [ ] All rows have tx_status='confirmed'
- [ ] All rows have non-null tx_signature
- [ ] All rows have confirmed_at set (within last 5 minutes)
- [ ] Amounts match allocations (800M, 1.6B, 2.4B)

### Step 6: Explorer Verification
For each tx_signature:
```
https://explorer.solana.com/tx/<SIGNATURE>?cluster=devnet
```

**Per-transaction**:
- [ ] Status shows "✅ Success"
- [ ] Program ID: GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
- [ ] Instruction: claim_open
- [ ] Can see token transfer event

**Optional: View transaction details**:
```bash
solana confirm <SIGNATURE> --url https://api.devnet.solana.com
```
- [ ] Shows "Finalized" or "Confirmed"

### Step 7: Epoch Report
```bash
npx tsx scripts/cls-epoch-report.ts --epoch 424245
```

**Output should show**:
- [ ] Total Allocated: 4,800,000,000,000 tokens
- [ ] Total Wallets: 3
- [ ] Confirmed: 3 / 3 (100.0%)
- [ ] Pending: 0
- [ ] Failed: 0
- [ ] Unclaimed: 0

---

## Summary Report

**Print this after Step 7**:
```bash
cat << 'EOF'
✅ CLS Devnet Validation Complete

Summary:
- Test Data: 3 users (weights 10/20/30)
- Allocations: 4.8B total tokens
- Claims Submitted: 3
- Claims Confirmed: 3 (100%)
- Explorer: All visible and finalized
- Database: All statuses updated

Next Steps:
1. Document any issues found
2. Review chain of custody (sealed → allocated → claimed)
3. When ready, repeat on mainnet with real data
4. Set up monitoring and alerting
EOF
```

---

## Troubleshooting

### "No participants found" (Step 2)
```bash
# Verify Step 1 completed
psql $DATABASE_URL -c \
  "SELECT COUNT(*) FROM sealed_participants WHERE channel='test-cls';"
# Should show 3

# Re-run Step 1
npx tsx scripts/test-cls-e2e-setup.ts
```

### "Invalid proof for epoch" (Step 4)
```bash
# Check root mismatch
psql $DATABASE_URL << 'EOF'
SELECT
  (SELECT root FROM sealed_epochs WHERE epoch=424245) as sealed_root,
  (SELECT root FROM l2_tree_cache WHERE epoch=424245) as cache_root;
EOF

# Sync root
npx tsx scripts/build-allocations-for-epoch.ts --channel test-cls --epoch 424245

# Retry claims
npx tsx scripts/allocate-and-claim.ts --csv scripts/claims.csv
```

### "Gateway responded with 500" (Step 4)
```bash
# Check gateway logs
pm2 logs gateway | tail -50

# Restart
pm2 restart gateway

# Retry
npx tsx scripts/allocate-and-claim.ts --csv scripts/claims.csv
```

### "Wallet mismatch" (Step 4)
```bash
# Verify keypair files exist and are correct
ls -la /tmp/test-cls-wallet-*.json

# Check the public key in wallet field matches the keypair
node -e "
const fs = require('fs');
const { Keypair } = require('@solana/web3.js');
const secret = JSON.parse(fs.readFileSync('/tmp/test-cls-wallet-0.json'));
const kp = Keypair.fromSecretKey(Uint8Array.from(secret));
console.log('Public key:', kp.publicKey.toBase58());
"

# Should match the wallet in claims.csv
```

---

## Success Criteria

| Step | Criterion | Status |
|------|-----------|--------|
| 1 | 3 test users created | ☐ |
| 2 | 3 allocations with correct amounts | ☐ |
| 3 | CSV created with valid paths | ☐ |
| 4 | 3 claims submitted and confirmed | ☐ |
| 5 | 3 cls_claims rows status='confirmed' | ☐ |
| 6 | All 3 txs visible on explorer as "Success" | ☐ |
| 7 | Epoch report shows 100% claim rate | ☐ |

**Overall**: 🟢 **PASS** when all checkmarks are filled

---

## Timeline

| Phase | Task | Time |
|-------|------|------|
| Pre-Flight | Verify environment | 5 min |
| Step 1 | Seed test epoch | 2 min |
| Step 2 | Build allocations | 2 min |
| Step 3 | Create CSV | 1 min |
| Step 4 | Submit claims | 15-20 min |
| Step 5 | DB verification | 2 min |
| Step 6 | Explorer verification | 3 min |
| Step 7 | Epoch report | 1 min |
| **Total** | | **30-35 min** |

---

## Post-Devnet Next Steps

Once devnet validates successfully:

1. **Document Findings**
   - Any issues discovered
   - Latency observations
   - Proof validation patterns

2. **Prepare for Mainnet**
   - Switch to mainnet RPC
   - Use real test wallets (with SOL)
   - Use real MINT_PUBKEY

3. **Real Channel Test**
   - Replace "test-cls" with real Twitch channel
   - Use real engagement weights from IRC aggregator
   - Validate with creator

4. **Set Up Monitoring**
   ```bash
   npx tsx scripts/cls-epoch-report.ts --epoch <id> --channel <channel>
   # Can run anytime to get epoch status
   ```

5. **Operational Runbook**
   - Document weight calculation scheme
   - Creator onboarding process
   - Claim failure recovery

---

## Useful Queries

**Check all allocations for epoch**:
```sql
SELECT wallet, index, amount, id
FROM allocations
WHERE epoch_id = 424245
ORDER BY wallet;
```

**Check all claims for epoch**:
```sql
SELECT wallet, tx_status, tx_signature, confirmed_at
FROM cls_claims
WHERE epoch_id = 424245
ORDER BY wallet;
```

**Get epoch summary**:
```bash
npx tsx scripts/cls-epoch-report.ts --epoch 424245
```

**Check Merkle root consistency**:
```sql
SELECT
  se.epoch,
  se.root as sealed_root,
  ltc.root as cache_root,
  CASE WHEN se.root = ltc.root THEN '✅' ELSE '❌' END as match
FROM sealed_epochs se
LEFT JOIN l2_tree_cache ltc ON se.epoch = ltc.epoch AND se.channel = ltc.channel
WHERE se.epoch = 424245;
```

---

## Command Cheat Sheet

```bash
# Pre-flight
env | grep -E "DATABASE_URL|SOLANA_RPC|GATEWAY_URL"

# Step 1
npx tsx scripts/test-cls-e2e-setup.ts

# Step 2
npx tsx scripts/build-allocations-for-epoch.ts -c test-cls -e 424245

# Step 3
cat > scripts/claims.csv << 'EOF'
wallet,epochs,keypair_path
...
EOF

# Step 4
npx tsx scripts/allocate-and-claim.ts --csv scripts/claims.csv

# Step 5
psql $DATABASE_URL -c "SELECT * FROM cls_claims WHERE epoch_id=424245;"

# Step 6
# Copy each tx_signature and visit:
# https://explorer.solana.com/tx/<SIGNATURE>?cluster=devnet

# Step 7
npx tsx scripts/cls-epoch-report.ts --epoch 424245
```

---

**Ready?** Start at Pre-Flight and work through each step. All green checks = successful devnet validation. 🚀

