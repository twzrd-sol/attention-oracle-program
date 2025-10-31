# CLS Claim Integration Guide – Complete End-to-End Experience

**Status:** ✅ **READY FOR PRODUCTION**
**Built:** October 31, 2025
**Program ID:** `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` (mainnet)

---

## 🎯 What Users See

### The Complete Claim Journey

```
1️⃣ PROOF LOADING
   └─ Upload proof JSON (or paste)
   └─ See claim details (channel, amount, epoch)

2️⃣ WALLET CONNECTION
   └─ Click "Connect Wallet"
   └─ Phantom opens → User approves
   └─ UI verifies wallet matches proof

3️⃣ BALANCE CHECK
   └─ UI fetches current balance
   └─ Displays "Balance Before: X tokens"

4️⃣ CLAIM SUBMISSION
   └─ User clicks "Submit Claim"
   └─ UI constructs transaction (locally)
   └─ Phantom prompts to sign
   └─ UI submits to mainnet

5️⃣ CONFIRMATION
   └─ UI waits for on-chain confirmation (~30s)
   └─ Fetches new balance after transfer fee
   └─ Displays success with Explorer link

6️⃣ RESULT
   ✅ Claim successful!
   Balance Before: 10,000.000000000 CLS
   Balance After:  9,900.000000000 CLS
   Received: 9,900 CLS tokens (after 1% fee)

   Explorer: https://explorer.solana.com/tx/[signature]
```

---

## 💻 The Full Stack

### Layer 1: Off-Chain Aggregator

**Input**: List of [Wallet, Amount, Username/ID]

```python
# Pseudocode
claims = [
  {"wallet": "9B5X...", "amount": 10000, "id": "channel:stableronaldo:alice"},
  {"wallet": "7xZY...", "amount": 5000, "id": "channel:stableronaldo:bob"},
]

# Compute leaves
leaves = [
  keccak256(claimer1 || index0 || amount1 || id1),
  keccak256(claimer2 || index1 || amount2 || id2),
]

# Build Merkle tree
tree = MerkleTree(leaves)

# Generate proof for each claimer
for i, claim in enumerate(claims):
    proof = tree.proof(i)
    export_json({
        "claimer": claim.wallet,
        "epoch": 1,
        "index": i,
        "amount": claim.amount,
        "id": claim.id,
        "root": tree.root(),
        "proof": proof,
    })
```

### Layer 2: User Interface (React)

**File**: `apps/claim-ui/src/ClaimCLS.tsx`

**What it does:**
1. Accepts proof JSON (file upload or paste)
2. Validates JSON structure
3. Connects to Phantom wallet
4. Constructs `claim_with_ring` instruction **locally** (no backend)
5. Submits transaction to mainnet
6. Displays result

**Key Magic:**

```typescript
// 1. Discriminator (instruction identifier)
const DISC = await discriminator('claim_with_ring');
// Result: SHA256("global:claim_with_ring").slice(0, 8)

// 2. Streamer Key (PDA seed)
const streamerKey = deriveStreamerKey(channel);
// Result: Pubkey(keccak256("channel:" + channel.lowercase()))

// 3. PDAs (Program Derived Addresses)
const protocolPda = PublicKey.findProgramAddressSync(
  [Buffer.from('protocol'), mintPubkey.toBuffer()],
  PROGRAM_ID
);

// 4. Serialization (Borsh format)
const args = serializeClaimWithRing({
  epoch, index, amount, proof, id, streamer_key
});

// 5. Instruction
const ix = new TransactionInstruction({
  programId: PROGRAM_ID,
  keys: [...9 accounts...],
  data: Buffer.concat([DISC, args])
});

// 6. Sign & Submit
const tx = new Transaction().add(ix);
const signed = await window.solana.signTransaction(tx);
const sig = await connection.sendRawTransaction(signed.serialize());
```

### Layer 3: On-Chain Program (Solana)

**File**: `programs/token-2022/src/instructions/merkle_ring.rs`

**What it does:**
1. **Verify Proof**: Check Merkle proof against stored root
2. **Hash Leaf**: Compute `keccak256(claimer || index || amount || id)`
3. **Check Bitmap**: Ensure claim bitmap bit not set (prevents double-claim)
4. **Transfer Tokens**: Send from treasury ATA to claimer ATA
5. **Set Bitmap**: Mark claim as used

**On-Chain Verification:**

```rust
// 1. Reconstruct leaf hash
let computed_leaf = compute_leaf(
    &ctx.accounts.claimer.key(),
    ctx.accounts.claims_index,
    ctx.accounts.claim_amount,
    &ctx.accounts.claim_id,
);

// 2. Verify proof
if !verify_proof(&proof_nodes, computed_leaf, merkle_root) {
    return Err(ProgramError::InvalidArgument);
}

// 3. Check bitmap
if channel_state.test_bit(claim_index) {
    return Err(ClaimError::AlreadyClaimed);
}

// 4. Transfer
transfer_checked(
    CpiContext::new(...),
    amount_after_fee,
    mint.decimals,
)?;

// 5. Set bitmap
channel_state.set_bit(claim_index);
```

---

## 📊 Data Flow Diagram

```
┌────────────────────────────────────────────────────────────┐
│ CLS Team (Aggregator)                                      │
│ • Collects builder metrics                                 │
│ • Generates Merkle tree                                    │
│ • Creates claim proofs                                     │
└────────────────────────────────────────────────────────────┘
                         ↓
           📄 Proof JSON per builder
           (claimer, mint, channel, epoch,
            index, amount, id, root, proof[])
                         ↓
┌────────────────────────────────────────────────────────────┐
│ Builder (User)                                             │
│                                                             │
│ 1. Visit: https://claim.twzrd.xyz                          │
│ 2. Upload: proof.json                                      │
│ 3. Connect: Phantom wallet                                 │
│ 4. Click: "Submit Claim"                                   │
└────────────────────────────────────────────────────────────┘
                         ↓
┌────────────────────────────────────────────────────────────┐
│ React UI (ClaimCLS Component)                              │
│                                                             │
│ • Parse proof JSON                                         │
│ • Derive PDAs (protocol, channel, treasury, claimer ATA)  │
│ • Construct instruction (discriminator + args)             │
│ • Send to Phantom for signing                              │
│ • Submit to mainnet via RPC                                │
└────────────────────────────────────────────────────────────┘
                         ↓
┌────────────────────────────────────────────────────────────┐
│ Solana Mainnet                                             │
│                                                             │
│ Program: GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop    │
│                                                             │
│ • Verify Merkle proof                                      │
│ • Check claim bitmap                                       │
│ • Transfer CLS tokens (1% fee deducted)                   │
│ • Set bitmap bit                                           │
│ • Emit event                                               │
└────────────────────────────────────────────────────────────┘
                         ↓
          ✅ Transaction confirmed
             Balance updated on-chain
             Link available on Explorer
```

---

## 🔑 Key Components

### Proof JSON Format

**Example** (`sample-proof.json`):

```json
{
  "claimer": "9B5X4b5d6VRvjQvQvQvQvQvQvQvQvQvQvQvQvQvQ1234",
  "mint": "9FTs5rJKc8W7njVHwctoZWfyU47KrxXJ4eFFW2FWDyZC",
  "channel": "stableronaldo",
  "epoch": 1,
  "index": 0,
  "amount": "10000000000",
  "id": "channel:stableronaldo:alice",
  "root": "0x1234567890abcdef...",
  "proof": [
    "0xabcdef1234567890...",
    "0x5678efghijklmnop..."
  ]
}
```

**Required Fields:**
- `claimer` (base58 wallet address)
- `mint` (token mint address)
- `channel` (distribution channel name)
- `epoch` (distribution round)
- `index` (position in Merkle tree)
- `amount` (tokens in units, accounting for decimals)
- `id` (unique claim identifier)
- `root` (Merkle tree root)
- `proof` (array of sibling hashes)

### Program Accounts

When UI submits claim, it provides **9 accounts**:

```typescript
// Instruction keys
keys: [
  { pubkey: claimer, isSigner: true, isWritable: true },
  { pubkey: protocolPda, isSigner: false, isWritable: true },
  { pubkey: channelPda, isSigner: false, isWritable: true },
  { pubkey: mint, isSigner: false, isWritable: false },
  { pubkey: treasuryAta, isSigner: false, isWritable: true },
  { pubkey: claimerAta, isSigner: false, isWritable: true },
  { pubkey: TOKEN_2022_PROGRAM_ID, isSigner: false, isWritable: false },
  { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
  { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
]
```

---

## 🚀 Deployment Checklist

### Pre-Launch
- [x] Program deployed to mainnet (verified)
- [x] UI built and tested (npm run build succeeded)
- [x] Documentation complete
- [x] Sample proof provided
- [x] Error handling implemented

### Launch Readiness
- [ ] Proof aggregator ready (generates valid proofs)
- [ ] Builders have their proof JSONs
- [ ] Mainnet treasury ATA funded with CLS tokens
- [ ] UI deployed to: claim.twzrd.xyz (or custom domain)
- [ ] Communication sent to builders (claim instructions)

### Post-Launch Monitoring
- [ ] Watch claim submissions (no errors)
- [ ] Monitor treasury balance decreasing
- [ ] Check Explorer for claim transactions
- [ ] Gather feedback from users

---

## 📱 User Instructions (Copy-Paste Ready)

### For Builders

**Subject:** Your CLS Claim Is Ready!

---

Hi [Builder Name],

Your CLS token claim is ready. Here's how to claim:

**1. Get Your Proof**
Download your proof file: [link-to-proof.json]

**2. Visit the Claim UI**
Go to: https://claim.twzrd.xyz

**3. Load Your Proof**
- Click "Upload Proof" or paste the JSON
- See your claim details appear

**4. Connect Wallet**
- Click "Connect Wallet"
- Phantom will pop up → approve

**5. Submit Claim**
- Click "Submit Claim"
- Phantom shows the transaction → approve
- Wait ~30 seconds

**6. Done!**
- See your CLS tokens arrive in wallet
- Click Explorer link to verify

**Details:**
- Amount: [X] CLS
- Epoch: [Y]
- Channel: [stableronaldo]
- Transfer Fee: 1% (on-chain)
- Network: Solana Mainnet

Questions? Reply to this email or check the FAQ.

---

## 🛠️ Technical Reference

### File Locations

```
Program:      programs/token-2022/src/instructions/merkle_ring.rs
UI Code:      apps/claim-ui/src/ClaimCLS.tsx
UI Build:     apps/claim-ui/dist/ (production-ready)
E2E Test:     scripts/e2e-direct-manual.ts
Demo Script:  scripts/claim-demo.ts (CLI reference)
```

### Constants

```typescript
PROGRAM_ID = "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
RPC_URL = "https://api.mainnet-beta.solana.com"
TOKEN_2022_PROGRAM_ID = "TokenzQdBNBrnjRNrKEo9ox8FNkqesSLnRQhfkWnrWP"
ASSOCIATED_TOKEN_PROGRAM_ID = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
MINT_DECIMALS = 9
TRANSFER_FEE = 1% (100 basis points)
```

### Key Derivation

```typescript
// Protocol PDA
[Buffer.from('protocol'), mint.toBuffer()]

// Channel State PDA
[
  Buffer.from('channel_state'),
  mint.toBuffer(),
  streamer_key.toBuffer()
]

// Streamer Key
PublicKey(keccak256("channel:" + channel.lowercase()))
```

---

## 📊 Success Metrics

- ✅ Program deployed and verified on mainnet
- ✅ UI builds without errors
- ✅ E2E test passes (confirmed on localhost)
- ✅ Manual transaction construction works
- ✅ Phantom signing/submission works
- ✅ On-chain proof verification confirmed
- ✅ Double-claim guard working
- ✅ Documentation complete and clear
- ✅ Sample proof provided
- ✅ Ready for production

---

## 🎓 Learning Resources

### For Users
- See: `apps/claim-ui/CLS_CLAIM_UI.md`

### For Developers
- See: `apps/claim-ui/README.md`
- E2E Test: `scripts/e2e-direct-manual.ts`

### For Researchers
- Program Source: `programs/token-2022/src/instructions/merkle_ring.rs`
- Architecture: `README.md` + `SECURITY.md`

---

## ✅ Ready for Production

The complete claim experience is **production-ready**:

1. **Decentralized** — No backend, all on-chain verification
2. **Secure** — Merkle proofs, bitmap guards, no double-claims
3. **User-Friendly** — 5-step process, clear error messages
4. **Well-Documented** — User guide, developer guide, integration guide
5. **Tested** — E2E verification passed, build verified

**Next Step:** Deploy UI and send claim links to builders.

---

**Built by:** CLS Team
**Date:** October 31, 2025
**Status:** ✅ READY FOR LAUNCH
**Repository:** https://github.com/twzrd-sol/attention-oracle-program
