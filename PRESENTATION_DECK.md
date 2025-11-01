# Verifiable Distribution Protocol

## I. The Coordination Failure

**Observation:** Every off-chain aggregation system (voting, contests, participation tracking, contribution measurement) faces the same bootstrapping problem.

**The Pattern:**
```
Off-chain measurement → Centralized database → Manual distribution → Trust requirement
```

**Why this fails:**
1. **Trust assumption:** Users must trust the aggregator won't manipulate results
2. **Custody risk:** Centralized claim servers become honeypots
3. **Verification gap:** No cryptographic proof linking measurement to settlement
4. **State explosion:** Per-address claim tracking doesn't scale

**Result:** Every project rebuilds the same infrastructure. No composability. No shared trust layer.

---

## II. First Principles Solution

**Constraint:** Separate measurement (subjective, off-chain) from settlement (objective, on-chain).

**Architecture:**
```
Measurement layer (off-chain)
    ↓ commitment (Merkle root)
Settlement layer (on-chain)
    ↓ verification (cryptographic proof)
Execution layer (Token-2022)
```

**Key insight:** The aggregator doesn't need to be trusted for custody. It only needs to commit to a root. Participants verify their own inclusion.

**Primitives required:**
1. Leaf binding: `hash(claimer_pubkey || index || amount || id)` prevents proof reuse
2. Ring buffer: Bitmap tracks claims per epoch, no per-address accounts
3. Time-lock: Grace period for claims before state cleanup
4. No backdoors: Even admin waits for time-lock

---

## III. Implementation

**On-chain state model:**

```rust
ProtocolState {
    admin: Pubkey,           // Configuration authority
    publisher: Pubkey,       // Can commit roots
}

ChannelState {
    mint: Pubkey,            // Distribution token
    operator: Pubkey,        // Namespace owner
    latest_epoch: u64,
}

EpochState {
    root: [u8; 32],          // Merkle commitment
    claim_count: u32,        // Expected claims
    timestamp: i64,          // For time-lock
    bitmap: [u8; N],         // Ring buffer (256 bits per 32 bytes)
}
```

**Claim verification:**
```rust
fn verify_claim(
    proof: &[[u8; 32]],
    claimer: Pubkey,
    index: u32,
    amount: u64,
    id: &str,
    root: [u8; 32],
) -> bool {
    let leaf = keccak256(claimer || index || amount || id);
    verify_merkle_proof(proof, leaf, root)
        && !is_claimed(bitmap, index)
}
```

**Gas optimization:**
Ring buffer uses 1 bit per claim. 256 claims = 32 bytes. Compare to per-address PDA model: 256 claims = 8KB+ of rent.

---

## IV. Security Properties

**Cryptographic guarantees:**
- Only the named wallet can claim (leaf binding)
- Each proof works exactly once (bitmap guard)
- No emergency admin override (time-lock invariant)

**Time-lock enforcement:**
```rust
const EPOCH_FORCE_CLOSE_GRACE_SECS: i64 = 604_800; // 7 days

// Even admin must wait
require!(
    clock.unix_timestamp >= epoch.timestamp + EPOCH_FORCE_CLOSE_GRACE_SECS,
    "Time-lock active"
);
```

**No rug vector:** Emergency close functions removed. State cleanup is the only admin function, and it's time-locked.

---

## V. Composability

**What this enables:**

1. **Trustless airdrops:** Aggregate snapshots off-chain, settle proofs on-chain
2. **Contest rewards:** Measure results off-chain, distribute cryptographically
3. **Contribution tracking:** DAO tooling for merit-based distribution
4. **Multi-party settlement:** Any off-chain data → on-chain claims

**Why Token-2022:**
- Transfer fees for sustainable economics
- Extension framework for future upgrades
- Native to Solana (no bridge risk)

**Comparison to alternatives:**

| Approach | Trust Model | State Cost | Verification |
|----------|-------------|------------|--------------|
| Centralized DB | Trust aggregator | O(1) | None |
| Per-address PDA | Trustless | O(n) | On-chain |
| Merkle + ring buffer | Trustless | O(1) | On-chain |

---

## VI. Current State

**Deployed:** Mainnet v1 at `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`

**Verified:**
- E2E cryptographic alignment (off-chain leaf = on-chain leaf)
- Proof verification (Merkle tree validation)
- Double-claim prevention (ring buffer guards)
- Time-lock enforcement (no admin bypass)

**Test results:**
- 10,000 tokens claimed → 9,900 received (1% transfer fee)
- Second claim attempt → `AlreadyClaimed` error
- Manual transaction construction (no Anchor IDL dependency)

**Repository:** https://github.com/twzrd-sol/attention-oracle-program

**License:** MIT (public good infrastructure)

---

## VII. Why This Matters

**Thesis:** Off-chain → on-chain bridges are primitives, not products.

Every measurement system needs this:
- Gaming (achievements, leaderboards)
- Content (engagement, contributions)
- Governance (voting, delegation)
- Reputation (credentials, attestations)

**Current state:** Each rebuilds the same infrastructure. Centralized claim servers. Trust assumptions. No composability.

**Inevitable state:** Shared primitive for verifiable distribution. Trustless. Composable. Economically sustainable.

This is infrastructure for the measurement layer of crypto.

Don't trust, verify.
