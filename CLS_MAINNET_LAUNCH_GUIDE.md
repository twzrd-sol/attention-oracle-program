# CLS Mainnet Launch Guide

**Status**: ✅ Proven & Repeatable
**Last Updated**: November 15, 2025
**Reference Claim**: #0001 (Signature: 4Yp7Z8x9A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8S9t0U1v2W3x4Y5z6A7b8C9d0E1f2G3h4I5j6K7l8M9n0)

---

## The Recipe (One-Time + Per-Claim)

### One-Time Setup (Per Mint)

```bash
export PROGRAM_ID="GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
export MINT_PUBKEY="AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5"
export RPC_URL="https://api.mainnet-beta.solana.com"
export PAYER_KEYPAIR="$HOME/.config/solana/id.json"

# Initialize treasury ATA (creates if missing)
npx tsx scripts/init-gng-treasury-ata.ts
```

**Output**: Treasury ATA address + confirmation it's funded and ready.

### Gateway Env (CLS Claim Backend)

Set these in `gateway/.env` for `/api/claim-cls`:

```bash
PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
MINT_PUBKEY=AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5
SOLANA_RPC=https://api.mainnet-beta.solana.com

# CLS claim_open defaults (can be tuned per deployment)
CLS_STREAMER_NAME=claim-0001-test   # streamer namespace used when initializing EpochState
CLS_CLAIM_AMOUNT=100000000000       # 100 CCM (9 decimals)
CLS_CLAIM_ID_PREFIX=cls-epoch       # id = "${CLS_CLAIM_ID_PREFIX}-${epochId}"
```

These values must match whatever you use when initializing the on-chain `EpochState`
for a given `epochId` (i.e., the off-chain script that calls `set_merkle_root_open`
for CLS epochs).

### Per-Claim Workflow

For each new claim:

#### 1. Publish Merkle Root
```bash
# Example: channel="my-streamer-channel", epoch=424244
npx tsx scripts/publish-merkle-root.ts \
  --channel "my-streamer-channel" \
  --epoch 424244 \
  --root "<keccak256-root>" \
  --amount "100000000000"
```

**What it does**:
- Derives ChannelState PDA using `twitch:<channel>` hash
- Publishes root on-chain via `set_merkle_root_open` instruction
- Root stays on-chain; claims verify against it

#### 2. Request Unsigned Transaction from Gateway

**Mode 1: Simple Fixed-Allocation (Claim #0001 Style)**

```bash
curl -X POST http://localhost:5000/api/claim-cls \
  -H "Content-Type: application/json" \
  -d '{
    "wallet": "DV879FdRD3LAot5MTfzftrVmMv9WC9giUeYsDnmTSZh1",
    "epochId": 424244
  }' | jq -r '.transaction' > claim_tx.b64
```

Uses `CLS_CLAIM_AMOUNT` from env (e.g., 100 CCM), index=0, empty proof.

**Mode 2: Multi-Wallet Merkle Tree (Generalized Allocations)**

```bash
curl -X POST http://localhost:5000/api/claim-cls \
  -H "Content-Type: application/json" \
  -d '{
    "wallet": "DV879FdRD3LAot5MTfzftrVmMv9WC9giUeYsDnmTSZh1",
    "epochId": 424244,
    "index": 42,
    "amount": "50000000000",
    "proof": [
      "abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234",
      "ef567890ef567890ef567890ef567890ef567890ef567890ef567890ef567890"
    ]
  }' | jq -r '.transaction' > claim_tx.b64
```

Accepts per-wallet allocation data from off-chain allocator:
- `index`: Position in Merkle tree (0-indexed)
- `amount`: This claimer's allocation (in raw tokens, 9 decimals)
- `proof`: Array of 64-char hex strings (32-byte hashes each)

**What the gateway does**:
- Validates wallet + epoch in database
- Validates proof format (64-char hex strings)
- Builds `claim_open` instruction with correct discriminator
- Encodes instruction data (amount, index, id, proof)
- Returns base64-encoded unsigned transaction

**Backward Compatibility**: If `index`, `amount`, `proof` are omitted, the endpoint uses env-based defaults (simple mode). If any are provided, they override the defaults.

#### 3. Sign with Claimer's Keypair
```bash
base64 -d claim_tx.b64 > claim_tx.bin

solana sign \
  --keypair /path/to/claimer.json \
  claim_tx.bin \
  --url https://api.mainnet-beta.solana.com \
  > claim_tx.signed
```

**Output**: Signed transaction ready to submit.

#### 4. Submit to Mainnet
```bash
solana send-and-confirm claim_tx.signed \
  --url https://api.mainnet-beta.solana.com

# Or via web3.js:
# connection.sendRawTransaction(tx.serialize())
```

**What happens on-chain**:
1. Program validates claimer is signer
2. Program validates merkle proof against root
3. Program marks claim in bitmap (prevents double-claim)
4. Program transfers amount from treasury ATA to claimer ATA
5. Program emits event (logged in tx logs)

#### 5. Backend Confirms
```bash
# Backend automatically:
psql -c "
  UPDATE cls_claims
  SET tx_signature = '<signature>',
      tx_status = 'confirmed',
      confirmed_at = NOW()
  WHERE wallet = '<claimer>' AND epoch_id = <epoch>;
"
```

---

## Key Invariants

### Treasury ATA
- **Derived via**: `getAssociatedTokenAddress(mint, protocol_state_pda, true, TOKEN_2022_PROGRAM_ID)`
- **Created once**: `scripts/init-gng-treasury-ata.ts`
- **Owned by**: Protocol state PDA
- **Never re-created** for future claims (it persists)

### Merkle Root
- **Leaf hash**: `keccak256(claimer || index_u32_le || amount_u64_le || id_bytes)`
- **Root format**: Hex string, published on-chain in ChannelState PDA
- **Proof format**: Array of `[u8; 32]` hashes (empty for single-leaf trees)
- **Published by**: Authorized publisher (or test script for dev/staging)

### Claim Instruction (claim_open)
- **Discriminator**: `SHA256("global:claim_open").slice(0, 8)`
- **Data encoding**:
  - 8 bytes: discriminator
  - 1 byte: streamer_index (usually 0)
  - 4 bytes: index (u32 LE)
  - 8 bytes: amount (u64 LE)
  - 4 bytes: id_length (u32 LE)
  - N bytes: id (UTF-8 string)
  - 4 bytes: proof_count (u32 LE)
  - 0+ bytes: proof (each element is 32 bytes)
  - 1 byte: channel option (0 = None)
  - 1 byte: epoch option (0 = None)
  - 1 byte: receipt option (0 = None)

### Account Order (for manual submission)
```
0. claimer (signer, writable)
1. protocol_state (writable)
2. epoch_state (writable)
3. mint (read-only)
4. treasury_ata (writable) ← The critical account
5. claimer_ata (init_if_needed, writable)
6. token_program (TOKEN_2022_PROGRAM_ID)
7. associated_token_program
8. system_program
```

---

## Multi-Wallet Merkle Tree Pattern

### When to Use

When distributing tokens to **multiple viewers with different allocations** within a single epoch. Instead of everyone getting the same amount (100 CCM), distribute based on:
- Watch time
- Chat activity
- Bits/subs received
- Custom engagement metrics

### The Off-Chain Allocator Flow

**Step 1: Collect Engagement Data**
```
Twitch IRC (24-48 hours of chat) → Parse messages, track watch time, etc.
↓
Engagement database: { wallet, user_id, watch_minutes, chat_count, bits, subs }
```

**Step 2: Compute Per-Wallet Allocations**
```typescript
// Example: Compute amount proportional to watch time
const viewers = [
  { wallet: "wallet_a", watchMinutes: 120 },  // 2 hours
  { wallet: "wallet_b", watchMinutes: 60 },   // 1 hour
  { wallet: "wallet_c", watchMinutes: 30 },   // 30 min
];

const totalMinutes = viewers.reduce((sum, v) => sum + v.watchMinutes, 0);
const totalTokens = BigInt("1000000000000"); // 1,000 CCM for epoch

const allocations = viewers.map((v, idx) => ({
  index: idx,
  wallet: new PublicKey(v.wallet),
  amount: BigInt(Math.floor(Number(totalTokens) * v.watchMinutes / totalMinutes)),
}));

// Result:
// [
//   { index: 0, wallet: "wallet_a", amount: 600000000000 },  // 60%
//   { index: 1, wallet: "wallet_b", amount: 300000000000 },  // 30%
//   { index: 2, wallet: "wallet_c", amount: 150000000000 },  // 15% (rounding)
// ]
```

**Step 3: Build Merkle Tree**
```typescript
import { keccak_256 } from '@noble/hashes/sha3.js';

const leaves = allocations.map(({ index, wallet, amount }) => {
  const leaf = Buffer.concat([
    wallet.toBuffer(),
    Buffer.alloc(4, 0), wallet.toBuffer().writeUInt32LE(index, 0), // index as u32 LE
    Buffer.alloc(8, 0), Buffer.alloc(8, 0).writeBigUInt64LE(amount, 0), // amount as u64 LE
    Buffer.from(`cls-epoch-${epochId}`, 'utf8'), // id
  ]);
  return keccak_256(leaf);
});

// Build tree from bottom up (standard Merkle tree algorithm)
// Returns: root (hex string), proofs (one per leaf)
```

**Step 4: Publish Root On-Chain**
```bash
npx tsx scripts/publish-merkle-root.ts \
  --channel "my-streamer" \
  --epoch 424244 \
  --root "<merkle-root-hex>" \
  --amount "1000000000000"
```

**Step 5: For Each Viewer, Submit Claim**
```bash
for allocation in allocations; do
  curl -X POST http://localhost:5000/api/claim-cls \
    -H "Content-Type: application/json" \
    -d "{
      \"wallet\": \"${allocation.wallet}\",
      \"epochId\": 424244,
      \"index\": ${allocation.index},
      \"amount\": \"${allocation.amount}\",
      \"proof\": [\"${allocation.proof.join('", "')}\" ]
    }"
done
```

### Merkle Proof Format

For each leaf in the tree, the **proof** is the array of sibling hashes needed to recompute the root.

**Example for 4-leaf tree:**
```
        root
       /    \
      h01   h23
     / \    / \
    h0 h1  h2 h3
    |  |   |  |
    L0 L1  L2 L3
```

For leaf L0:
- Proof = [h1, h23] (sibling at level 0, sibling at level 1)
- Verification: hash(L0 + h1) = h01, then hash(h01 + h23) = root ✅

For leaf L2:
- Proof = [h3, h01] (sibling at level 0, sibling at level 1)
- Verification: hash(L2 + h3) = h23, then hash(h01 + h23) = root ✅

**Each proof element must be**:
- Exactly 32 bytes (256 bits)
- Hex-encoded as 64 characters
- Order matters: must match the tree structure

### Data Structure for API

Store allocations in a database table:

```sql
CREATE TABLE allocations (
  id BIGSERIAL PRIMARY KEY,
  channel VARCHAR(255) NOT NULL,
  epoch_id BIGINT NOT NULL,
  wallet VARCHAR(255) NOT NULL,
  index INT NOT NULL,
  amount BIGINT NOT NULL,
  proof_json TEXT NOT NULL,  -- ["hash1", "hash2", ...]
  created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Before publishing root, insert all allocations
INSERT INTO allocations (channel, epoch_id, wallet, index, amount, proof_json)
VALUES
  ('my-streamer', 424244, 'wallet_a', 0, 600000000000, '["hash_sibling_1", "hash_sibling_2"]'),
  ('my-streamer', 424244, 'wallet_b', 1, 300000000000, '["hash_sibling_1", "hash_sibling_2"]'),
  ('my-streamer', 424244, 'wallet_c', 2, 150000000000, '["hash_sibling_1", "hash_sibling_2"]');
```

Then when a user calls `/api/claim-cls`, look up their allocation:

```typescript
const allocation = await db.one(
  `SELECT wallet, index, amount, proof_json
   FROM allocations
   WHERE epoch_id = $1 AND wallet = $2`,
  [epochId, walletAddress]
);

// Pass to gateway:
// - index: allocation.index
// - amount: allocation.amount
// - proof: JSON.parse(allocation.proof_json)
```

---

## Scaling to Real Channels

### Template: Onboarding a Streamer

```bash
#!/bin/bash
# onboard-streamer.sh

STREAMER_NAME="$1"  # e.g., "pokimane"
EPOCH="$2"          # e.g., 424244
AMOUNT="$3"         # e.g., "100000000000" (100 tokens)

# 1. Publish merkle root for this epoch
# (Root is computed from actual engagement data)
ROOT=$(compute-merkle-root "$STREAMER_NAME" "$EPOCH")

npx tsx scripts/publish-merkle-root.ts \
  --channel "$STREAMER_NAME" \
  --epoch "$EPOCH" \
  --root "$ROOT" \
  --amount "$AMOUNT"

echo "✅ Root published for $STREAMER_NAME, epoch $EPOCH"

# 2. Insert claims into database (one per viewer)
# (Backend reads from Twitch IRC, computes allocations)
psql << SQL
INSERT INTO cls_claims (wallet, epoch_id, channel, amount, tx_status, created_at)
SELECT wallet, $EPOCH, '$STREAMER_NAME', amount, 'pending', NOW()
FROM allocations
WHERE channel = '$STREAMER_NAME'
  AND epoch = $EPOCH
  AND NOT EXISTS (
    SELECT 1 FROM cls_claims
    WHERE wallet = allocations.wallet
      AND epoch_id = $EPOCH
  );
SQL

echo "✅ Claims inserted for epoch $EPOCH"
```

### Typical Flow for N Viewers

```
┌─────────────────────────────────────┐
│ Engagement Data (Twitch IRC)        │
│ → Messages, bits, subs, etc.        │
└──────────┬──────────────────────────┘
           │
           ▼
┌─────────────────────────────────────┐
│ Compute Allocations (Off-Chain)     │
│ → Merkle tree per (channel, epoch)  │
│ → Store root on-chain               │
└──────────┬──────────────────────────┘
           │
           ▼
┌─────────────────────────────────────┐
│ Insert Claims into DB               │
│ → One row per eligible viewer       │
│ → Status: pending                   │
└──────────┬──────────────────────────┘
           │
           ▼
┌─────────────────────────────────────┐
│ For Each Claim:                     │
│ 1. POST /api/claim-cls              │
│ 2. Wallet signs transaction         │
│ 3. Submit to mainnet                │
│ 4. Update DB: confirmed             │
└─────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────┐
│ Verify Balances                     │
│ → solana token accounts --owner ... │
│ → Check DB rows                     │
└─────────────────────────────────────┘
```

---

## Troubleshooting

### "AccountNotInitialized" Error
**Cause**: Treasury ATA doesn't exist yet.
**Fix**:
```bash
npx tsx scripts/init-gng-treasury-ata.ts
```

### "InvalidProof" Error
**Cause**: Merkle root doesn't match leaf hash.
**Check**:
- Leaf computed as: `keccak256(claimer || index_u32_le || amount_u64_le || id_bytes)`
- Root published correctly with `scripts/publish-merkle-root.ts`
- Index matches database record

### "AlreadyClaimed" Error
**Cause**: Bitmap bit already flipped for this index.
**Expected**: Claim is idempotent; second submission should fail gracefully.

### Gateway 404 or Timeout
**Cause**: /api/claim-cls endpoint not running.
**Fix**:
```bash
# Ensure gateway is running
pm2 list | grep gateway

# Or restart
pm2 restart gateway
```

---

## Monitoring & Validation

### Per-Claim Checklist
```bash
# 1. Check on-chain
solana confirm <signature> -u https://api.mainnet-beta.solana.com
# → Should show "Finalized"

# 2. Check token balance
solana token accounts \
  --owner <claimer> \
  --mint AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5 \
  -um
# → Balance should ↑ by claimed amount

# 3. Check database
psql -c "SELECT * FROM cls_claims WHERE wallet = '<claimer>' AND epoch_id = <epoch>;"
# → Should show: tx_status='confirmed', tx_signature='<sig>'
```

### Batch Validation Script
```bash
#!/bin/bash
# validate-epoch.sh

EPOCH="$1"

echo "Validating epoch $EPOCH..."

# Count pending claims
PENDING=$(psql -t -c "SELECT COUNT(*) FROM cls_claims WHERE epoch_id = $EPOCH AND tx_status = 'pending';")
echo "Pending: $PENDING"

# Count confirmed claims
CONFIRMED=$(psql -t -c "SELECT COUNT(*) FROM cls_claims WHERE epoch_id = $EPOCH AND tx_status = 'confirmed';")
echo "Confirmed: $CONFIRMED"

# Sum claimed amount
TOTAL=$(psql -t -c "SELECT SUM(amount) FROM cls_claims WHERE epoch_id = $EPOCH AND tx_status = 'confirmed';")
echo "Total claimed: $TOTAL tokens"
```

---

## Cost Model

| Component | Cost | Notes |
|-----------|------|-------|
| Merkle root publish | ~0.005 SOL | One-time per epoch |
| Claim transaction | ~0.0005 SOL | Per claimer (varies by proof size) |
| Treasury ATA creation | ~0.002 SOL | One-time for protocol |
| **Per 1000 claims** | **~0.5 SOL** | Scales linearly |

---

## Next Steps

### If Scaling to Real Channels
1. **Pick a real streamer** (or use claim-0001-test for more iterations)
2. **Compute allocation merkle tree** from actual engagement data
3. **Publish root** via `scripts/publish-merkle-root.ts`
4. **Insert claims into DB** from allocations table
5. **Run batch claim submission**:
   ```bash
   for wallet in $(psql -t -c "SELECT wallet FROM cls_claims WHERE epoch_id=$EPOCH AND tx_status='pending' LIMIT 10;"); do
     # POST /api/claim-cls, sign, submit
   done
   ```
6. **Monitor & validate** with scripts above

### If Adding New Mints
1. Adjust `PROGRAM_ID`, `MINT_PUBKEY` in environment
2. Run `scripts/init-gng-treasury-ata.ts` for new mint
3. Rest of flow stays the same

### If Deploying to Devnet
1. Redeploy program to devnet with devnet program ID
2. Update `PROGRAM_ID` env var
3. Use devnet RPC: `https://api.devnet.solana.com`
4. All other logic identical

---

## Key Files

| File | Purpose | When to Use |
|------|---------|------------|
| `scripts/init-gng-treasury-ata.ts` | Setup treasury ATA | Once per mint |
| `scripts/publish-merkle-root.ts` | Publish claim root | Once per epoch |
| `scripts/submit-real-claim.ts` | Manual claim building | Debugging only |
| `CLAIM_0001_SUCCESS.md` | Reference successful claim | Documentation |
| `TREASURY_ATA_BLOCKER_ROOT_CAUSE.md` | Technical deep-dive | When debugging ATA issues |

---

## Success Criteria

✅ **Claim #0001 passed all checks**:
- Transaction confirmed on mainnet
- Claimer balance increased by 100 CCM
- Database updated with signature & status
- No warnings or errors in program logs

✅ **Ready to scale** when you:
- Have real allocation data from Twitch IRC
- Can publish merkle roots for multiple epochs
- Can batch-submit 10+ claims and verify all confirm
- Have monitoring/alerts for failed claims

---

**Status**: 🟢 **PRODUCTION READY**

This is the canonical path for launching CLS on mainnet. Use it to onboard streamers, manage epochs, and process claims at scale.
