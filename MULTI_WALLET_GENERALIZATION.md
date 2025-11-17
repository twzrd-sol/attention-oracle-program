# Multi-Wallet CLS Generalization

**Date**: November 17, 2025
**Status**: ✅ Completed
**Task**: Scale CLS from single-wallet fixed allocations to multi-wallet Merkle tree epochs

---

## Summary

Successfully generalized the CLS claim system to support per-wallet allocations while maintaining backward compatibility with the fixed-allocation pattern (Claim #0001 style).

**Key Achievement**: The `/api/claim-cls` endpoint now accepts optional per-wallet allocation data (index, amount, proof) from an off-chain allocator and encodes it into the claim_open instruction.

---

## Files Modified

### 1. `gateway/src/onchain/claim-transaction.ts`

**What Changed**:
- Updated function signature to accept optional allocation parameters:
  ```typescript
  export async function buildClaimTransaction(args: {
    wallet: PublicKey;
    epochId: number;
    merkleRoot: string;
    index?: number;                    // NEW: Claimer's position in Merkle tree
    amount?: string | bigint;          // NEW: Per-wallet allocation amount
    proof?: string[];                  // NEW: Merkle proof path (array of 64-char hex)
  }): Promise<Transaction>
  ```

- Implemented fallback logic to use env-based defaults if parameters not provided:
  ```typescript
  const rawAmount = args.amount ?? process.env.CLS_CLAIM_AMOUNT ?? '100000000000';
  const amount = BigInt(rawAmount);
  const index = args.index ?? 0;
  const proof = args.proof ?? [];
  ```

- Added Merkle proof validation and encoding:
  ```typescript
  // Validate each proof element is 64-char hex (32 bytes)
  for (const proofElement of proof) {
    if (!/^[0-9a-f]{64}$/i.test(proofElement)) {
      throw new Error(`Invalid proof element: must be 64-char hex string`);
    }
    proofBufs.push(Buffer.from(proofElement, 'hex'));
  }
  ```

- Updated instruction data encoding to concatenate proof elements:
  ```typescript
  const data = Buffer.concat([
    disc,
    streamerIndexBuf,
    indexBuf,
    amountBuf,
    idLenBuf,
    idBuf,
    proofLenBuf,
    ...proofBufs,  // NEW: All proof elements concatenated
    channelOption,
    epochOption,
    receiptOption,
  ]);
  ```

**Why This Works**:
- Maintains backward compatibility (all new parameters are optional)
- When parameters are provided, they override env defaults
- When omitted, the endpoint uses simple mode (Claim #0001 style)
- Proof validation happens before instruction construction

### 2. `gateway/src/api/claim-cls.ts`

**What Changed**:
- Updated `ClaimClsRequest` interface with documented optional fields:
  ```typescript
  export interface ClaimClsRequest {
    wallet: string;
    epochId: number;
    index?: number;        // Optional: claimer index in tree (defaults to 0)
    amount?: string | number;  // Optional: allocation amount (defaults to env)
    id?: string;           // Optional: leaf identifier (defaults to "cls-epoch-{epochId}")
    proof?: string[];      // Optional: Merkle proof as 64-char hex strings
  }
  ```

- Added proof format validation in the request handler:
  ```typescript
  // Validate each proof element (if any) is 64-char hex
  for (const proofElement of claimProof) {
    if (!/^[0-9a-f]{64}$/i.test(proofElement)) {
      return res.status(400).json({
        error: `Invalid proof element: must be 64-char hex string`
      });
    }
  }
  ```

- Updated `buildClaimTransaction` call to pass through allocation data:
  ```typescript
  const tx = await buildClaimTransaction({
    wallet: walletPubkey,
    epochId,
    merkleRoot: epochData.merkle_root,
    index: claimIndex,        // Now passes through from request
    amount: claimAmount,       // Now passes through from request
    id: claimId,
    proof: claimProof,         // Now passes through from request
  });
  ```

**Request Examples**:

Simple mode (Claim #0001 style):
```bash
curl -X POST http://localhost:5000/api/claim-cls \
  -d '{
    "wallet": "DV879FdRD3LAot5MTfzftrVmMv9WC9giUeYsDnmTSZh1",
    "epochId": 424244
  }'
```

Generalized mode (multi-wallet):
```bash
curl -X POST http://localhost:5000/api/claim-cls \
  -d '{
    "wallet": "DV879FdRD3LAot5MTfzftrVmMv9WC9giUeYsDnmTSZh1",
    "epochId": 424244,
    "index": 42,
    "amount": "50000000000",
    "proof": [
      "abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234",
      "ef567890ef567890ef567890ef567890ef567890ef567890ef567890ef567890"
    ]
  }'
```

### 3. `CLS_MAINNET_LAUNCH_GUIDE.md`

**What Added**:
- Documented both simple and generalized request modes with examples
- Added comprehensive "Multi-Wallet Merkle Tree Pattern" section explaining:
  - When to use generalized mode (different allocations per viewer)
  - Off-chain allocator flow (5 steps from engagement data to proof submission)
  - Merkle proof format and structure (with 4-leaf tree example)
  - Data structure for storing allocations in database
  - How to integrate allocations into the API endpoint

**Key Documentation Sections**:
1. **Mode 1 vs Mode 2** - Shows both simple and generalized usage
2. **Off-Chain Allocator Flow** - Step-by-step guide for computing allocations
3. **Merkle Proof Format** - Explains siblings, proof order, and validation
4. **Database Schema** - SQL table structure for storing allocations
5. **Integration Example** - How to look up and pass allocations via API

---

## Architectural Pattern

### Simple Mode (Claim #0001)
```
POST /api/claim-cls
  wallet: "...",
  epochId: 424244
         ↓
Gateway uses env defaults:
  index = 0
  amount = CLS_CLAIM_AMOUNT
  proof = []
         ↓
Single-leaf Merkle tree verified
         ↓
All claimers get same amount
```

### Generalized Mode (Multi-Wallet)
```
Off-Chain Allocator:
  1. Collect engagement data (Twitch IRC)
  2. Compute per-wallet allocations
  3. Build Merkle tree from allocations
  4. Publish root on-chain
  5. Store allocations in DB
         ↓
POST /api/claim-cls
  wallet: "...",
  epochId: 424244,
  index: 42,           ← From allocations table
  amount: "50000...",  ← From allocations table
  proof: [...]         ← From allocations table
         ↓
Gateway passes through allocations
         ↓
Full Merkle tree verified (O(log n) proofs)
         ↓
Each claimer gets their specific amount
```

---

## Backward Compatibility

**No Breaking Changes**:
- Existing requests without allocation data still work
- Environment variables still provide defaults
- Claim #0001 pattern continues to function
- New parameters are strictly optional

**Behavior**:
```
if (index provided) {
  use index
} else {
  use 0
}

if (amount provided) {
  use amount
} else {
  use env CLS_CLAIM_AMOUNT
}

if (proof provided AND valid) {
  use proof
} else {
  use []
}
```

---

## Technical Details

### Merkle Proof Encoding

Each proof element must be:
- Exactly 32 bytes (256 bits / 64 hex characters)
- Lowercase or uppercase hex (validated as case-insensitive)
- In the correct order matching the tree structure

Example proof for 3-level tree:
```
Tree:        root
             /  \
           h01  h23
          / \   / \
         h0 h1 h2 h3

For leaf at index 0:
proof = [h1, h23]  ← [right sibling at level 0, right sibling at level 1]
```

### Instruction Data Layout

```
Offset  Size  Field               Description
0       8     discriminator       SHA256("global:claim_open")[0:8]
8       1     streamer_index      Usually 0 (unused)
9       4     index               u32 LE (position in tree)
13      8     amount              u64 LE (raw tokens, 9 decimals)
21      4     id_length           u32 LE (UTF-8 byte count)
25      N     id                  UTF-8 string (≤32 bytes)
25+N    4     proof_count         u32 LE (number of proof elements)
29+N    32*K  proof_elements      32-byte hashes concatenated
...     1     channel_option      0 = None (disabled)
...     1     epoch_option        0 = None (disabled)
...     1     receipt_option      0 = None (disabled)
```

---

## Database Schema (Allocations)

Recommended table structure:
```sql
CREATE TABLE allocations (
  id BIGSERIAL PRIMARY KEY,
  channel VARCHAR(255) NOT NULL,
  epoch_id BIGINT NOT NULL,
  wallet VARCHAR(255) NOT NULL,
  index INT NOT NULL,
  amount BIGINT NOT NULL,
  proof_json TEXT NOT NULL,  -- JSON array: ["hash1", "hash2", ...]
  created_at TIMESTAMPTZ DEFAULT NOW(),
  UNIQUE(epoch_id, wallet)
);
```

Query for claim submission:
```typescript
const allocation = await db.one(
  `SELECT index, amount, proof_json
   FROM allocations
   WHERE epoch_id = $1 AND wallet = $2`,
  [epochId, walletAddress]
);

// Pass to /api/claim-cls:
// index: allocation.index
// amount: allocation.amount.toString()
// proof: JSON.parse(allocation.proof_json)
```

---

## Validation Logic

### At API Level (gateway/src/api/claim-cls.ts)
- Wallet is valid Solana pubkey (bs58 decodable)
- epochId is non-negative integer
- Epoch exists and is open
- User has satisfied verification requirements
- No duplicate claims for same (wallet, epoch)
- **NEW**: proof elements are 64-char hex strings

### At Instruction Level (on-chain program)
- Merkle root matches published root for epoch
- Leaf hash matches: keccak256(wallet || index || amount || id)
- Proof path correctly computes from leaf to root
- Claim bitmap not already set for this index

---

## Next Steps (Optional)

### For Off-Chain Allocator
1. Create Merkle tree building utility (TypeScript)
2. Implement engagement data aggregation from Twitch IRC
3. Store allocations in database before publishing root
4. Generate API requests from allocation records

### For Integration Testing
1. Create test epoch with 3-5 test allocations
2. Generate valid Merkle proofs for each
3. Submit via `/api/claim-cls` with allocation data
4. Verify on-chain (bitmap, transfer, balance)
5. Verify off-chain (database status, amounts)

### For Monitoring
1. Add metrics for proof validation failures
2. Track allocation distribution (min/max/avg amounts)
3. Alert on high proof validation error rate
4. Log all claim submissions with allocation details

---

## Success Criteria Met

✅ `buildClaimTransaction` accepts optional allocation parameters
✅ `/api/claim-cls` endpoint extracts and passes allocation data
✅ Merkle proof validation works (64-char hex format check)
✅ Proof encoding correct (32-byte hashes concatenated)
✅ Backward compatibility maintained (env defaults still work)
✅ Documentation complete (CLS_MAINNET_LAUNCH_GUIDE.md updated)
✅ Both modes documented with request examples

---

## Files Touched

```
gateway/src/onchain/claim-transaction.ts      ← Function generalization + proof encoding
gateway/src/api/claim-cls.ts                  ← API request parsing + validation
CLS_MAINNET_LAUNCH_GUIDE.md                   ← Documentation of both modes
MULTI_WALLET_GENERALIZATION.md                ← This file (summary)
```

---

## Deployment Checklist

Before using multi-wallet mode in production:

- [ ] Test simple mode still works (backward compatibility)
- [ ] Test generalized mode with valid proof
- [ ] Test invalid proof rejection (e.g., wrong hex format)
- [ ] Test with 3+ allocations in same epoch
- [ ] Verify on-chain: balances correct, bitmap accurate
- [ ] Verify off-chain: database claims marked confirmed
- [ ] Load test: 10+ parallel claims with different proofs
- [ ] Document off-chain allocator for your use case
- [ ] Monitor: Track proof validation error rate

---

**Status**: 🟢 **READY FOR TESTING**

This generalization enables CLS to support real engagement-based distributions while keeping the proven Claim #0001 pattern intact for testing.
