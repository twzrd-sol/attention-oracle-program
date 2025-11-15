# 🎯 CLS Protocol - Architecture Quick Reference

> **For full details, see:** [TECHNICAL_ARCHITECTURE.md](./TECHNICAL_ARCHITECTURE.md) (1700 lines)

---

## 30-Second Overview

**CLS (Channel Loyalty System)** = Twitch viewers earn SPL Token-2022 tokens via merkle-proof claims

**Flow:**
```
Viewer watches stream → Events ingested → Hourly epochs sealed →
Merkle roots published on-chain → Users claim with proofs
```

**Key Innovation:** Ring buffer storage (fixed 10KB per channel) prevents state bloat

---

## Core Components

### On-Chain (Solana Program)

**Program ID:** `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`

**3 Account Types:**

1. **ProtocolState** (Singleton - 141 bytes)
   - Admin, Publisher, Mint, Treasury
   - Seeds: `["protocol", mint]`

2. **ChannelState** (Per-channel - 10KB fixed)
   - Ring buffer: 10 epochs × 542 bytes each
   - Seeds: `["channel_state", mint, keccak256("channel:" + name)]`
   - **Critical:** Overwrites oldest epoch when full

3. **EpochState** (Legacy - deprecated, dynamic size)
   - Old unbounded storage (caused bloat)
   - New claims use ChannelState ring buffer

**3 Key Instructions:**

1. `set_merkle_root_ring` - Publish merkle root (creates channel if needed)
2. `claim_with_ring` - Verify proof + transfer tokens
3. `initialize_channel` - Pre-create channel PDA (optional)

---

### Off-Chain (Node.js Services)

**6 PM2 Processes:**

1. **stream-listener** - Connects to Twitch IRC, tracks viewers
2. **cls-worker-s0/s1** - Consumes events, writes PostgreSQL
3. **epoch-watcher** - Detects hour boundaries, triggers sealing
4. **tree-builder** - Builds merkle trees, stores proofs
5. **cls-aggregator** - Publishes roots on-chain (auto-loop)
6. **gateway** - API serves proofs to users

**Database (PostgreSQL):**
```
participation_events → sealed_participants → sealed_epochs
                                           ↓
                                    Published on-chain
```

---

## Key Data Structures

### ChannelSlot (Rust)

```rust
#[zero_copy]
#[repr(C, packed)]
pub struct ChannelSlot {
    pub epoch: u64,                          // Unix timestamp (seconds)
    pub root: [u8; 32],                      // Merkle root
    pub claim_count: u16,                    // Claims so far
    pub claimed_bitmap: [u8; 512],          // 4096 bits (max participants)
}
// Total: 542 bytes per slot × 10 slots = 5,420 bytes
```

### ChannelState (Rust)

```rust
#[account(zero_copy)]
pub struct ChannelState {
    pub version: u8,                         // Schema version
    pub bump: u8,                            // PDA bump
    pub mint: Pubkey,                        // 32 bytes
    pub streamer: Pubkey,                    // 32 bytes (keccak hash)
    pub latest_epoch: u64,                   // Most recent published
    pub slots: [ChannelSlot; 10],           // 5,420 bytes
}
// Total: ~10KB (8 disc + 74 header + 5,420 slots + padding)
```

**Ring Buffer Logic:**
```rust
fn publish(channel: &mut ChannelState, epoch: u64, root: [u8; 32]) {
    let slot_idx = (epoch as usize) % 10;  // Modulo 10 (circular)
    channel.slots[slot_idx].reset(epoch, root);
    channel.latest_epoch = epoch;
}
```

---

## Critical Algorithms

### 1. Merkle Tree (Off-Chain)

**Leaf Computation:**
```typescript
function computeLeaf(wallet: PublicKey, username: string, index: u32, amount: u64): [u8; 32] {
  return keccak256(
    wallet.toBytes() +          // 32 bytes
    Buffer.from(username) +     // Variable UTF-8
    uint32LE(index) +           // 4 bytes
    uint64LE(amount)            // 8 bytes
  );
}
```

**Tree Building:**
- Sort participants by username (deterministic)
- Compute leaves
- Build tree bottom-up (keccak256 hash pairs)
- Store root + proofs in database

### 2. Proof Verification (On-Chain)

```rust
fn verify_proof(proof: &[[u8; 32]], root: [u8; 32], leaf: [u8; 32]) -> bool {
    let mut computed = leaf;
    for sibling in proof {
        computed = if computed <= *sibling {
            keccak256(&[computed, sibling])
        } else {
            keccak256(&[sibling, computed])
        };
    }
    computed == root
}
```

**Complexity:** O(log n) - typically 10-20 hashes for 1000s of participants

### 3. Bitmap Claiming (On-Chain)

```rust
fn is_claimed(slot: &ChannelSlot, index: u32) -> bool {
    let byte = (index / 8) as usize;
    let bit = (index % 8) as u8;
    (slot.claimed_bitmap[byte] & (1 << bit)) != 0
}

fn mark_claimed(slot: &mut ChannelSlot, index: u32) {
    let byte = (index / 8) as usize;
    let bit = (index % 8) as u8;
    slot.claimed_bitmap[byte] |= 1 << bit;
    slot.claim_count += 1;
}
```

**Limits:**
- 512 bytes = 4,096 bits = max 4,096 participants per epoch
- O(1) check and update

---

## Integration Examples

### Client-Side Claim (TypeScript)

```typescript
// 1. Fetch proof from gateway
const proof = await fetch(`https://gateway.twzrd.xyz/proof?user=alice&channel=xqc&epoch=1762495200`)
  .then(r => r.json());

// 2. Derive accounts
const streamerKey = keccak256("channel:" + proof.channel.toLowerCase());
const [channelState] = PublicKey.findProgramAddressSync(
  [Buffer.from("channel_state"), mint.toBuffer(), streamerKey],
  PROGRAM_ID
);

// 3. Build claim instruction
const ix = await program.methods
  .claimWithRing(
    new BN(proof.epoch),
    proof.index,
    new BN(proof.amount),
    proof.proof.map(p => Buffer.from(p.slice(2), 'hex')),
    proof.id,
    new PublicKey(streamerKey)
  )
  .accounts({
    claimer: wallet.publicKey,
    protocolState,
    channelState,
    mint,
    treasury,
    treasuryAta,
    claimerAta,
    tokenProgram,
    associatedTokenProgram,
    systemProgram,
  })
  .instruction();

// 4. Send transaction
const tx = new Transaction().add(ix);
const sig = await wallet.sendTransaction(tx, connection);
```

### Server-Side Publish (TypeScript)

```typescript
// Fetch sealed epoch from database
const { root, participant_count } = await db.query(
  `SELECT root, participant_count FROM sealed_epochs
   WHERE channel = $1 AND epoch = $2 AND published = false`,
  [channel, epoch]
).then(r => r.rows[0]);

// Publish on-chain
const streamerKey = keccak256("channel:" + channel.toLowerCase());
const [channelState] = PublicKey.findProgramAddressSync(
  [Buffer.from("channel_state"), mint.toBuffer(), streamerKey],
  PROGRAM_ID
);

const ix = await program.methods
  .setMerkleRootRing(
    channel,
    new BN(epoch),
    Buffer.from(root, 'hex'),
    new BN(participant_count * 1_000_000_000) // Total claimable (9 decimals)
  )
  .accounts({
    payer: publisher.publicKey,
    protocolState,
    channelState,
    systemProgram,
  })
  .instruction();

const sig = await sendAndConfirmTransaction(connection, new Transaction().add(ix), [publisher]);
```

---

## Current Deployment

### Mainnet Stats (2025-11-07)

- **Channels initialized:** 15
- **Unpublished epochs:** 85 (across 8 recently initialized channels)
- **Publishing rate:** ~1 epoch/minute (automated)
- **Publisher balance:** 1.459 SOL
- **Cost per channel init:** 0.04002 SOL (rent)
- **Cost per publish:** 0.00001 SOL (fees only)

### Wallets

1. **87d5...ufdy** (oracle-authority.json) - Publisher/Payer
   - Auto-publishes merkle roots
   - Pays channel init rent
   - Balance: 1.459 SOL

2. **2pHjZ...ZZaD** (id.json) - Protocol Admin
   - On-chain admin authority
   - Can update config/pause
   - Balance: ~0.00 SOL (needs funding)

3. **AmMf...CsBv** (admin-keypair.json) - Legacy Admin
   - Maintenance operations
   - Balance: 0.085 SOL

### Services Status

All PM2 processes online:
```bash
cls-aggregator    ✓ Online (publishes roots)
gateway           ✓ Online (serves proofs)
cls-worker-s0/s1  ✓ Online (ingests events)
epoch-watcher     ✓ Online (triggers sealing)
tree-builder      ✓ Online (builds merkle trees)
```

---

## Security Model

### On-Chain

**Authorization Hierarchy:**
```
Admin (can update config, pause, rotate publisher)
  ↓
Publisher (can publish merkle roots)
  ↓
Claimer (can claim with valid proof)
```

**Treasury Security:**
- Treasury is a PDA (no private key)
- Only program can sign for treasury (with seeds)
- Funds safe even if admin/publisher compromised

**Claim Security:**
- Merkle proof prevents claiming without participation
- Bitmap prevents double-claiming
- Amount baked into leaf (can't inflate)
- Wallet pubkey in leaf (can't claim as different user)

### Off-Chain

**Database:**
- SSL required (DigitalOcean managed)
- Sealed participants immutable after publish
- Regular backups (TBD)

**Publisher Key:**
- File permissions: 0600 (owner only)
- SSH access restricted
- Balance monitoring (low balance alerts)

**Attack Vectors:**
- ✅ Malicious publisher → Admin can rotate
- ✅ Database compromise → Can't publish without publisher key
- ✅ Front-running claims → Each user has unique proof
- ⚠️ Publisher collusion → Need trusted admin (future: multisig/DAO)

---

## Performance & Limits

### Current Constraints

| Metric | Limit | Notes |
|--------|-------|-------|
| Participants per epoch | 4,096 | Bitmap size (512 bytes × 8) |
| Epochs per channel | 10 | Ring buffer slots |
| Transaction size | ~1.2 KB | Fits in 1232 byte limit |
| Claim window | ~10 hours | Before eviction (hourly epochs) |
| Publish rate | ~60/hour | RPC rate limits |
| Database capacity | 100K+ events/hour | Tested |

### Scalability Roadmap

**Short-term:**
- Increase bitmap to 1024 bytes (8,192 participants)
- Batch publishing (multiple channels per tx)
- Multiple publishers (load balancing)

**Long-term:**
- L2 aggregation (publish batches to L1)
- Sharded databases (per channel/region)
- CDN for proof serving

---

## Common Operations

### Check Channel Status

```bash
# On-chain
solana account <channel_pda> --url mainnet-beta

# Off-chain (database)
psql -c "SELECT * FROM sealed_epochs WHERE channel='xqc' AND published=false"
```

### Initialize New Channel

```bash
# Option 1: Publish first epoch (auto-creates)
PUBLISH_REQUIRE_INITIALIZED=false npx tsx scripts/publish-root-mainnet.ts <channel> <epoch>

# Option 2: Pre-initialize (pays rent upfront)
npx tsx scripts/init-channel.ts <channel>
```

### Manual Publish

```bash
cd /home/twzrd/milo-token
env AGGREGATOR_URL=http://127.0.0.1:8080 \
    PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
    ADMIN_KEYPAIR=/home/twzrd/.config/solana/oracle-authority.json \
    npx tsx scripts/publish-root-mainnet.ts <channel> <epoch>
```

### Check Unpublished Backlog

```sql
SELECT channel, COUNT(*) as unpublished
FROM sealed_epochs
WHERE published = false
GROUP BY channel
ORDER BY unpublished DESC;
```

---

## Monitoring

### Key Metrics

**On-Chain:**
- Channels initialized (query program accounts)
- Epochs published per hour (transaction history)
- Claims per hour (transaction history)
- Treasury balance (`solana balance <treasury_ata>`)

**Off-Chain:**
- Participation events ingested (PostgreSQL count)
- Epochs sealed per hour (sealed_epochs inserts)
- Unpublished backlog (sealed_epochs WHERE published=false)
- Publisher balance (low balance alert)

### Logs

```bash
# Service logs
pm2 logs cls-aggregator  # Publishing
pm2 logs gateway         # API requests
pm2 logs cls-worker-s0   # Ingestion

# Database queries
psql -c "SELECT channel, epoch, published FROM sealed_epochs ORDER BY epoch DESC LIMIT 10"
```

---

## Troubleshooting

### Issue: Epochs not publishing

**Check:**
1. Publisher balance (`solana balance 87d5...ufdy`)
2. Aggregator logs (`pm2 logs cls-aggregator`)
3. Strict mode (`PUBLISH_REQUIRE_INITIALIZED=true`?)
4. Channel initialized (`solana account <channel_pda>`)

**Fix:**
- Fund publisher wallet if low
- Restart aggregator: `pm2 restart cls-aggregator`
- Initialize channel if needed
- Check RPC status

### Issue: Claims failing

**Check:**
1. Epoch published on-chain (`solana account <channel_pda>`)
2. Proof valid (gateway `/proof` endpoint)
3. User has SOL for fees
4. Treasury has tokens

**Fix:**
- Wait for epoch to publish
- Verify proof format
- Fund user wallet
- Check treasury balance

### Issue: Database bloat

**Check:**
1. participation_events table size
2. sealed_participants table size
3. Old epochs (>30 days)

**Fix:**
- Archive old participation_events
- Vacuum database: `VACUUM FULL`
- Consider partitioning tables

---

## Resources

**Full Documentation:**
- [TECHNICAL_ARCHITECTURE.md](./TECHNICAL_ARCHITECTURE.md) - Complete 1700-line guide
- [WALLET_MAP.md](./WALLET_MAP.md) - Wallet roles and locations
- [KEYPAIR_AUDIT.md](./scripts/KEYPAIR_AUDIT.md) - Script safety audit
- [SESSION_SUMMARY_2025-11-07.md](./SESSION_SUMMARY_2025-11-07.md) - Latest changes

**Code:**
- Program: `/home/twzrd/milo-token/clean-hackathon/programs/token-2022/`
- Services: `/home/twzrd/milo-token/apps/`
- Scripts: `/home/twzrd/milo-token/scripts/`

**On-Chain:**
- Program: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- Explorer: https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
- Mint: `AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5`

---

**Last Updated:** 2025-11-07 20:15 UTC
