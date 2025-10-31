# Verifiable Distribution Protocol – Presentation Deck

## Slide 1: Thesis & Vision

**Verifiable Distribution Protocol: Trustless Claims on Solana**

The Problem:
- Off-chain aggregation (community participation, contest results, token distributions) lacks on-chain provenance
- Centralized claim servers introduce trust assumptions and bottlenecks
- No cryptographic proof that a claim is legitimate without re-executing off-chain logic

The Solution:
- Merkle tree rooted on-chain, hardened with leaf binding (claimer + index + amount + id)
- Ring buffer state management for per-epoch claims (no per-address state explosion)
- Token-2022 for transfer fee flexibility and extensibility
- E2E verified: off-chain leaf computation matches on-chain proof verification

Vision:
- Enable transparent, verifiable token distributions at scale
- Builders can aggregate claims (contests, milestones, community contributions) without intermediaries
- Claimer cryptographically proves eligibility without revealing others' claims

---

## Slide 2: Architecture

**Off-Chain → On-Chain Pipeline**

```
┌─ Off-Chain Aggregator ──────────────────────────┐
│                                                 │
│  Input: [Participant, Amount, Metadata]        │
│    ↓                                            │
│  compute_leaf(claimer, index, amount, id)      │
│    ↓ (keccak256)                               │
│  Build Merkle Tree (leaves → root)             │
│    ↓                                            │
│  Output: {root, epoch, proof[], claim_data}   │
│                                                 │
└─────────────────────────────────────────────────┘
           ↓
┌─ On-Chain Contract (Token-2022) ────────────────┐
│                                                 │
│  1. Publisher sets merkle root & epoch         │
│  2. Claimer submits: claim_with_ring           │
│     - epoch, index, amount, proof[], id        │
│  3. Verify:                                    │
│     - Leaf matches (hash(claimer, ix, amt, id))│
│     - Proof valid (verify all siblings)        │
│     - Hasn't claimed before (ring bit guard)   │
│  4. Transfer tokens (with transfer fee)        │
│                                                 │
└─────────────────────────────────────────────────┘
```

**Key Invariants:**
- Leaf binding: Only the wallet named in the claim can redeem it
- Ring state: Compact bitmap (256 claims per 32-byte slot)
- Time-lock: Epoch state locked for 7 days (EPOCH_FORCE_CLOSE_GRACE_SECS)
- No emergency backdoors: All close operations go through time-lock

---

## Slide 3: Security Hardening

**E2E Verified Cryptographic Alignment**

Off-Chain Leaf Computation:
```
leaf = keccak256( claimer_pubkey[32] || index_le_u32 || amount_le_u64 || id_utf8 )
```

On-Chain Verification:
```rust
pub fn compute_leaf(claimer: &Pubkey, index: u32, amount: u64, id: &str) -> [u8; 32] {
    // Same hashing, matches aggregator output
}
pub fn verify_proof(proof: &[[u8; 32]], leaf: [u8; 32], root: [u8; 32]) -> bool {
    // Merkle tree proof validation
}
```

**Hardening Patches Applied:**
1. Fixed `#[instruction]` annotation to include `id` parameter (was missing, broke PDA derivation)
2. Set correct `declare_id` to deployed program address (was placeholder)
3. Removed legacy emergency close functions (force_close_epoch_state variants)
4. Added `id: String` to the `#[instruction(...)]` attribute and enforce id length in the handler

**E2E Test Results:**
- ✅ Leaf hash alignment (off-chain = on-chain)
- ✅ Proof verification (Merkle tree valid)
- ✅ Token transfer (10,000 sent, 9,900 received after 1% fee)
- ✅ Double-claim guard (AlreadyClaimed error)
- ✅ Manual transaction construction (no Anchor IDL needed)

Transaction Signature: `4vXXRos8eUZW1nECn5LeL2tAcJsvP6LqUiie5ougRf19KkucaG2hEgVk3Cs7B6LE1DFcv2weaehsNaTeefYGQgRn`

---

## Slide 4: CLS "Proof of Builder" Rollout

**Companion Launch Stream (CLS) – Micro-Epoch Distribution**

Objective:
- Demonstrate live claim flow on devnet and mainnet
- Build builder community with verifiable token distribution
- Show ecosystem developers that claim process is transparent and trustless

Devnet Flow (Testnet Proof):
1. Initialize CLS mint (Token-2022 with 1% transfer fee)
2. Set publisher authority
3. Initialize channel for `example_channel` streamer
4. Publish micro-epoch root (e.g., ≥100 CLS split across two builders — Builder A and Builder B)
5. Fund treasury ATA
6. **Demo:** Submit claim_with_ring via manual script or CLI
7. Verify balance increase (after fee)

Mainnet Flow (Production Deployment):
1. Repeat devnet initialization on mainnet
2. Capture tx signatures for transparency
3. Announce CLS token address and claim instructions
4. Enable live claims for verified builders

Next Steps:
- Expand to additional content creators (streamers, educators, community leads)
- Integrate with aggregator API for automated proof generation
- Multi-epoch scheduling (weekly or monthly drops)
- Community governance over distribution parameters

---

## Slide 5: Demo & Q&A

**Live Demo (CLI / Minimal UI)**

1. Fetch proof JSON from aggregator API
2. Show proof structure: `{root, epoch, index, amount, id, proof[], claimer}`
3. Construct and sign `claim_with_ring` instruction (manual or via @solana/web3.js)
4. Submit to devnet, watch balance update
5. Attempt double-claim, show `AlreadyClaimed` error
6. Inspect on-chain state (epoch bitmap, treasury decrease)

Code Reference:
- Manual transaction builder: `scripts/e2e-direct-manual.ts`
- E2E test suite: `tests/e2e.verification.ts`
- Program source: `programs/token-2022/src/instructions/merkle_ring.rs`

Questions?
- How does this compare to other distribution models? → Fully trustless, no intermediary
- Can we customize the fee? → Yes, configurable per mint
- What's the scalability? → Ring buffer supports 256 claims per epoch, linear in epochs
- Can we pause or revoke claims? → Time-lock invariant ensures fairness; no backdoors
