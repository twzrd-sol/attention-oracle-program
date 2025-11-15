# 🏗️ CLS Protocol - Technical Architecture Overview

**Version:** Mainnet v3 (Ring Buffer)
**Program ID:** `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
**Network:** Solana Mainnet
**Token Standard:** SPL Token-2022

---

## Table of Contents

1. [System Overview](#system-overview)
2. [On-Chain Architecture](#on-chain-architecture)
3. [Off-Chain Infrastructure](#off-chain-infrastructure)
4. [Data Flow](#data-flow)
5. [Key Algorithms](#key-algorithms)
6. [Security Model](#security-model)
7. [Integration Guide](#integration-guide)

---

## System Overview

### What is CLS?

The **Channel Loyalty System (CLS)** is a Solana-based protocol that rewards Twitch viewers with SPL Token-2022 tokens based on their participation in live streams. The system:

1. **Ingests** viewer participation data from Twitch streams (real-time)
2. **Seals** participation into hourly epochs (merkle trees)
3. **Publishes** merkle roots on-chain (Solana)
4. **Enables claiming** via merkle proof verification (SPL Token-2022 transfers)

### Architecture Philosophy

- **Hybrid on-chain/off-chain**: Computation happens off-chain, verification on-chain
- **Merkle proofs**: Efficient claiming without storing all participant data on-chain
- **Ring buffer**: Fixed-size channel state (10 epochs per channel) prevents state bloat
- **Token-2022**: Native transfer fees and modern SPL features

---

## On-Chain Architecture

### Program Structure (Anchor)

```
programs/token-2022/src/
├── lib.rs                 # Program entry point, instruction routing
├── constants.rs           # Seeds, sizes, limits
├── errors.rs             # Custom error codes
├── state.rs              # Account schemas
└── instructions/
    ├── mod.rs            # Instruction exports
    ├── protocol.rs       # Protocol initialization
    ├── merkle.rs         # Legacy epoch state (deprecated)
    ├── merkle_ring.rs    # Ring buffer publish (ACTIVE)
    ├── channel.rs        # Channel state management
    └── claim.rs          # Claim verification & execution
```

### Core Account Types

#### 1. ProtocolState (Singleton)

**Purpose:** Global protocol configuration (admin, publisher, fees)

**Seeds:** `["protocol", mint]` (mint-keyed for multi-token support)

**Schema:**
```rust
#[account]
pub struct ProtocolState {
    pub admin: Pubkey,           // Can update protocol config
    pub publisher: Pubkey,       // Can publish merkle roots
    pub mint: Pubkey,            // Token mint (Token-2022)
    pub treasury: Pubkey,        // Treasury PDA (receives fees)
    pub paused: bool,            // Emergency pause flag
    pub bump: u8,                // PDA bump seed
}
```

**Size:** 141 bytes (8 discriminator + 133 data)

**Access Control:**
- `admin`: Can update admin, publisher, pause state
- `publisher`: Can publish merkle roots (set_merkle_root_ring)
- Anyone: Can read state

---

#### 2. ChannelState (Ring Buffer)

**Purpose:** Stores last N epochs for a channel (circular buffer)

**Seeds:** `["channel_state", mint, streamer_key]`
- `streamer_key = keccak256("channel:" + channel_name.toLowerCase())`

**Schema:**
```rust
#[account(zero_copy)]
pub struct ChannelState {
    pub version: u8,                          // Schema version (1)
    pub bump: u8,                             // PDA bump
    pub mint: Pubkey,                         // Token mint
    pub streamer: Pubkey,                     // Streamer key (keccak hash)
    pub ring_head: u16,                       // Next slot to write (0-9)
    pub slots: [ChannelSlot; CHANNEL_RING_SLOTS], // 10 slots
}

#[zero_copy]
pub struct ChannelSlot {
    pub epoch: u64,                           // Unix timestamp (seconds)
    pub root: [u8; 32],                       // Merkle root
    pub total_claimable: u64,                 // Total tokens allocated
    pub claimed_amount: u64,                  // Amount claimed so far
    pub timestamp: i64,                       // Publish timestamp
    pub bitmap: [u8; CHANNEL_BITMAP_BYTES],  // Claimed bitmap (512 bytes)
}
```

**Size:** ~10,240 bytes per channel
- Header: 72 bytes
- 10 slots × 616 bytes each = 6,160 bytes
- Padding/alignment: ~4 KB total

**Ring Buffer Logic:**
```rust
// Publishing advances the ring head
let slot_idx = channel_state.ring_head as usize;
channel_state.slots[slot_idx] = new_slot;
channel_state.ring_head = (channel_state.ring_head + 1) % CHANNEL_RING_SLOTS;
```

---

#### 3. EpochState (Legacy - Deprecated)

**Purpose:** Old unbounded epoch storage (caused account bloat)

**Seeds:** `["epoch_state", epoch_le_bytes, streamer_key, mint]`

**Schema:**
```rust
#[account]
pub struct EpochState {
    pub epoch: u64,
    pub root: [u8; 32],
    pub claim_count: u32,
    pub mint: Pubkey,
    pub streamer: Pubkey,
    pub treasury: Pubkey,
    pub timestamp: i64,
    pub total_claimed: u32,
    pub closed: bool,
    pub claimed_bitmap: Vec<u8>,  // Dynamic size (PROBLEM!)
}
```

**Status:** Used by old claims, new publishes use ChannelState ring buffer

---

### Key Instructions

#### 1. initialize_protocol

**Purpose:** One-time protocol initialization

**Accounts:**
```rust
#[derive(Accounts)]
pub struct InitializeProtocol<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + std::mem::size_of::<ProtocolState>(),
        seeds = [PROTOCOL_SEED, mint.key().as_ref()],
        bump
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    pub mint: InterfaceAccount<'info, Mint>,
    pub system_program: Program<'info, System>,
}
```

**Handler:**
```rust
pub fn initialize_protocol(
    ctx: Context<InitializeProtocol>,
    publisher: Pubkey,
) -> Result<()> {
    let protocol = &mut ctx.accounts.protocol_state;
    protocol.admin = ctx.accounts.admin.key();
    protocol.publisher = publisher;
    protocol.mint = ctx.accounts.mint.key();
    protocol.treasury = treasury_pda;
    protocol.paused = false;
    protocol.bump = ctx.bumps.protocol_state;
    Ok(())
}
```

---

#### 2. set_merkle_root_ring (PRIMARY PUBLISH)

**Purpose:** Publish merkle root to ring buffer (creates/updates channel state)

**Accounts:**
```rust
#[derive(Accounts)]
pub struct SetMerkleRootRing<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = !protocol_state.paused @ ProtocolError::ProtocolPaused,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// CHECK: Validated via seeds
    #[account(mut)]
    pub channel_state: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}
```

**Handler:**
```rust
pub fn set_merkle_root_ring(
    ctx: Context<SetMerkleRootRing>,
    channel: String,
    epoch: u64,
    root: [u8; 32],
    total_claimable: u64,
) -> Result<()> {
    let protocol = &ctx.accounts.protocol_state;

    // Authorization
    authorize_publisher(protocol, &ctx.accounts.payer.key())?;

    // Derive streamer key
    let streamer_key = derive_streamer_key(&channel);

    // Verify PDA
    let (expected_pda, bump) = Pubkey::find_program_address(
        &[CHANNEL_STATE_SEED, protocol.mint.as_ref(), streamer_key.as_ref()],
        ctx.program_id
    );
    require_keys_eq!(expected_pda, ctx.accounts.channel_state.key());

    // Create account if needed
    if ctx.accounts.channel_state.owner != ctx.program_id {
        create_channel_account(/* ... */)?;
    }

    // Load zero-copy account
    let mut channel_data = ctx.accounts.channel_state.try_borrow_mut_data()?;
    let channel_state = ChannelState::from_bytes_mut(&mut channel_data)?;

    // Write to ring buffer
    let slot_idx = channel_state.ring_head as usize;
    let slot = &mut channel_state.slots[slot_idx];

    slot.epoch = epoch;
    slot.root = root;
    slot.total_claimable = total_claimable;
    slot.claimed_amount = 0;
    slot.timestamp = Clock::get()?.unix_timestamp;
    slot.bitmap.fill(0); // Clear bitmap

    // Advance ring head
    channel_state.ring_head = (channel_state.ring_head + 1) % CHANNEL_RING_SLOTS;

    msg!("Published: channel={} epoch={} root={:?}", channel, epoch, root);
    Ok(())
}
```

**Key Points:**
- Creates channel PDA on first publish (~0.04002 SOL rent)
- Overwrites oldest epoch when buffer is full (circular)
- No dynamic allocations (fixed 10KB per channel)

---

#### 3. claim_with_ring

**Purpose:** Verify merkle proof and transfer tokens to user

**Accounts:**
```rust
#[derive(Accounts)]
pub struct ClaimWithRing<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,

    #[account(
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// CHECK: Zero-copy channel state
    #[account(mut)]
    pub channel_state: AccountInfo<'info>,

    #[account(mut)]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = treasury,
        associated_token::token_program = token_program,
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = claimer,
        associated_token::mint = mint,
        associated_token::authority = claimer,
        associated_token::token_program = token_program,
    )]
    pub claimer_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}
```

**Handler:**
```rust
pub fn claim_with_ring(
    ctx: Context<ClaimWithRing>,
    epoch: u64,
    index: u32,
    amount: u64,
    proof: Vec<[u8; 32]>,
    id: String,
    streamer_key: Pubkey,
) -> Result<()> {
    // Load channel state (zero-copy)
    let mut channel_data = ctx.accounts.channel_state.try_borrow_mut_data()?;
    let channel_state = ChannelState::from_bytes_mut(&mut channel_data)?;

    // Find epoch in ring buffer
    let slot = channel_state.slots.iter_mut()
        .find(|s| s.epoch == epoch)
        .ok_or(ProtocolError::EpochNotFound)?;

    // Check bitmap (already claimed?)
    let byte_idx = (index / 8) as usize;
    let bit_idx = (index % 8) as u8;
    require!(
        (slot.bitmap[byte_idx] & (1 << bit_idx)) == 0,
        ProtocolError::AlreadyClaimed
    );

    // Compute leaf
    let leaf = compute_participation_leaf(
        &ctx.accounts.claimer.key(),
        &id,
        index,
        amount
    )?;

    // Verify merkle proof
    require!(
        verify_proof(&proof, slot.root, leaf),
        ProtocolError::InvalidProof
    );

    // Mark as claimed (set bit)
    slot.bitmap[byte_idx] |= 1 << bit_idx;
    slot.claimed_amount += amount;

    // Transfer tokens (Token-2022 CPI)
    let treasury_seeds = &[
        TREASURY_SEED,
        ctx.accounts.protocol_state.mint.as_ref(),
        &[ctx.accounts.protocol_state.treasury_bump]
    ];

    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.treasury_ata.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.claimer_ata.to_account_info(),
                authority: ctx.accounts.treasury.to_account_info(),
            },
            &[treasury_seeds]
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    msg!("Claimed: claimer={} epoch={} amount={}",
         ctx.accounts.claimer.key(), epoch, amount);
    Ok(())
}
```

**Merkle Proof Verification:**
```rust
fn verify_proof(proof: &[[u8; 32]], root: [u8; 32], leaf: [u8; 32]) -> bool {
    let mut computed_hash = leaf;

    for proof_element in proof.iter() {
        computed_hash = if computed_hash <= *proof_element {
            keccak::hashv(&[&computed_hash, proof_element]).to_bytes()
        } else {
            keccak::hashv(&[proof_element, &computed_hash]).to_bytes()
        };
    }

    computed_hash == root
}

fn compute_participation_leaf(
    claimer: &Pubkey,
    id: &str,
    index: u32,
    amount: u64,
) -> Result<[u8; 32]> {
    let mut hasher = Keccak::v256();
    hasher.update(claimer.as_ref());
    hasher.update(id.as_bytes());
    hasher.update(&index.to_le_bytes());
    hasher.update(&amount.to_le_bytes());

    let mut output = [0u8; 32];
    hasher.finalize(&mut output);
    Ok(output)
}
```

---

## Off-Chain Infrastructure

### Architecture Diagram

```
┌─────────────────┐
│  Twitch IRC     │
│  (chat/viewers) │
└────────┬────────┘
         │
         ▼
┌─────────────────────────┐
│  stream-listener        │ PM2 Process
│  - Connects to IRC      │
│  - Tracks viewers       │
│  - Emits events         │
└────────┬────────────────┘
         │ Redis Pub/Sub
         ▼
┌─────────────────────────┐
│  cls-worker-s0/s1       │ PM2 Processes
│  - Consumes events      │
│  - Writes PostgreSQL    │
└────────┬────────────────┘
         │
         ▼
┌──────────────────────────────┐
│  PostgreSQL (DigitalOcean)   │
│  - participation_events      │
│  - sealed_participants       │
│  - sealed_epochs            │
└────────┬─────────────────────┘
         │
         ▼
┌─────────────────────────┐
│  epoch-watcher          │ PM2 Process
│  - Monitors epoch close │
│  - Triggers sealing     │
└────────┬────────────────┘
         │
         ▼
┌─────────────────────────┐
│  tree-builder           │ PM2 Process
│  - Builds merkle trees  │
│  - Computes roots       │
│  - Stores in DB         │
└────────┬────────────────┘
         │
         ▼
┌─────────────────────────┐
│  cls-aggregator         │ PM2 Process
│  - Fetches sealed roots │
│  - Publishes on-chain   │
│  - Signs with publisher │
└────────┬────────────────┘
         │ Solana RPC
         ▼
┌─────────────────────────┐
│  Solana Mainnet         │
│  - Program: GnGzNds...  │
│  - ChannelState PDAs    │
└─────────────────────────┘
         │
         ▼
┌─────────────────────────┐
│  gateway (API)          │ PM2 Process
│  - Serves proofs        │
│  - Query interface      │
└────────┬────────────────┘
         │ HTTPS
         ▼
┌─────────────────────────┐
│  Claim UI (Netlify)     │
│  - User interface       │
│  - Wallet integration   │
│  - Proof submission     │
└─────────────────────────┘
```

---

### Service Breakdown

#### 1. stream-listener

**Language:** Node.js/TypeScript
**Purpose:** Connect to Twitch IRC, track viewer presence

**Key Code:**
```typescript
// apps/stream-listener/src/index.ts
import { ChatClient } from '@twurple/chat';

const client = new ChatClient({
  channels: channelsToMonitor,
  isAlwaysMod: false,
});

client.onMessage((channel, user, message) => {
  // Track chat participation
  redis.publish('cls:chat', JSON.stringify({
    channel: channel.slice(1), // Remove #
    user: user,
    timestamp: Date.now(),
    type: 'message'
  }));
});

// Periodic viewer list fetch
setInterval(async () => {
  for (const channel of channels) {
    const viewers = await fetchViewerList(channel);
    redis.publish('cls:viewers', JSON.stringify({
      channel,
      viewers,
      timestamp: Date.now()
    }));
  }
}, 30000); // Every 30 seconds
```

---

#### 2. cls-worker (Ingestion)

**Language:** Node.js/TypeScript
**Purpose:** Consume events, write to PostgreSQL

**Schema:**
```sql
CREATE TABLE participation_events (
    id BIGSERIAL PRIMARY KEY,
    channel VARCHAR(64) NOT NULL,
    username VARCHAR(64) NOT NULL,
    user_id VARCHAR(64),
    event_type VARCHAR(32) NOT NULL, -- 'view', 'chat', 'sub', etc.
    timestamp BIGINT NOT NULL, -- Unix milliseconds
    epoch BIGINT NOT NULL, -- Floored to hour boundary
    token_group VARCHAR(16) DEFAULT 'CLS',
    metadata JSONB
);

CREATE INDEX idx_participation_channel_epoch
    ON participation_events(channel, epoch, token_group);
```

**Key Code:**
```typescript
// apps/worker-v2/src/index.ts
redis.subscribe('cls:chat', 'cls:viewers');

redis.on('message', async (channel, message) => {
  const event = JSON.parse(message);
  const epoch = Math.floor(event.timestamp / 3600000) * 3600; // Hour boundary

  await pool.query(
    `INSERT INTO participation_events
     (channel, username, event_type, timestamp, epoch, token_group)
     VALUES ($1, $2, $3, $4, $5, $6)
     ON CONFLICT DO NOTHING`,
    [event.channel, event.user, 'view', event.timestamp, epoch, 'CLS']
  );
});
```

---

#### 3. epoch-watcher

**Language:** Node.js/TypeScript
**Purpose:** Detect epoch close, trigger sealing

**Key Code:**
```typescript
// apps/epoch-watcher/src/index.ts
setInterval(async () => {
  const now = Date.now();
  const currentEpoch = Math.floor(now / 3600000) * 3600;
  const previousEpoch = currentEpoch - 3600;

  // Find channels with unsealed previous epoch
  const channels = await pool.query(`
    SELECT DISTINCT channel
    FROM participation_events
    WHERE epoch = $1
      AND channel NOT IN (
        SELECT channel FROM sealed_epochs WHERE epoch = $1
      )
  `, [previousEpoch]);

  for (const { channel } of channels.rows) {
    // Trigger sealing
    await aggregator.sealEpoch(previousEpoch, channel);
  }
}, 60000); // Check every minute
```

---

#### 4. tree-builder (Merkle)

**Language:** Node.js/TypeScript
**Purpose:** Build merkle trees from participation data

**Key Code:**
```typescript
// apps/twzrd-aggregator/src/merkle.ts
import { MerkleTree } from 'merkletreejs';
import { keccak256 } from 'js-sha3';

export function buildMerkleTree(
  participants: Array<{
    username: string;
    wallet: string | null;
    amount: bigint;
  }>
): { tree: MerkleTree; root: string; proofs: Map<string, string[]> } {

  // Sort participants by username (deterministic)
  const sorted = participants.sort((a, b) =>
    a.username.localeCompare(b.username)
  );

  // Compute leaves
  const leaves = sorted.map((p, index) => {
    const leaf = keccak256.create();
    if (p.wallet) {
      leaf.update(Buffer.from(bs58.decode(p.wallet))); // Wallet pubkey
    } else {
      leaf.update(Buffer.from([0; 32])); // Default pubkey for unnamed
    }
    leaf.update(Buffer.from(p.username, 'utf8')); // Username (ID)
    leaf.update(Buffer.from(new Uint32Array([index]).buffer)); // Index
    leaf.update(Buffer.from(new BigUint64Array([p.amount]).buffer)); // Amount
    return Buffer.from(leaf.digest());
  });

  // Build tree (keccak256 hash function)
  const tree = new MerkleTree(leaves, (data) => {
    return Buffer.from(keccak256.create().update(data).digest());
  }, {
    sortPairs: true, // Canonical ordering
    hashLeaves: false, // Already hashed
  });

  const root = tree.getRoot().toString('hex');

  // Generate proofs for all leaves
  const proofs = new Map();
  leaves.forEach((leaf, index) => {
    const proof = tree.getProof(leaf).map(p => p.data.toString('hex'));
    proofs.set(sorted[index].username, proof);
  });

  return { tree, root, proofs };
}
```

**Sealing Flow:**
```typescript
// apps/twzrd-aggregator/src/db-pg.ts
async sealEpoch(
  epoch: number,
  channel: string,
  computeRoot: (users: string[]) => string,
  tokenGroup: string = 'CLS'
) {
  const client = await this.maintenancePool.connect();

  try {
    await client.query('BEGIN');

    // Check if already sealed
    const existing = await client.query(
      `SELECT 1 FROM sealed_epochs
       WHERE epoch = $1 AND channel = $2 AND token_group = $3`,
      [epoch, channel, tokenGroup]
    );

    if (existing.rows.length > 0) {
      await client.query('ROLLBACK');
      return;
    }

    // Fetch participants
    const result = await client.query(
      `SELECT DISTINCT username, user_id, COUNT(*) as score
       FROM participation_events
       WHERE epoch = $1 AND channel = $2 AND token_group = $3
       GROUP BY username, user_id
       ORDER BY username ASC`,
      [epoch, channel, tokenGroup]
    );

    if (result.rows.length === 0) {
      await client.query('ROLLBACK');
      return;
    }

    // Assign amounts (simple: 1 token per participant)
    const participants = result.rows.map((row, index) => ({
      username: row.username,
      index,
      amount: 1_000_000_000n, // 1 token (9 decimals)
      wallet: null, // Resolved later via gateway
    }));

    // Build merkle tree
    const { tree, root, proofs } = buildMerkleTree(participants);

    // Write sealed_participants
    for (const p of participants) {
      await client.query(
        `INSERT INTO sealed_participants
         (epoch, channel, token_group, username, index, amount, proof)
         VALUES ($1, $2, $3, $4, $5, $6, $7)`,
        [epoch, channel, tokenGroup, p.username, p.index, p.amount.toString(), proofs.get(p.username)]
      );
    }

    // Write sealed_epochs
    await client.query(
      `INSERT INTO sealed_epochs
       (epoch, channel, token_group, root, participant_count, published)
       VALUES ($1, $2, $3, $4, $5, false)`,
      [epoch, channel, tokenGroup, root, participants.length]
    );

    await client.query('COMMIT');
    console.log(`Sealed: ${channel} epoch ${epoch} (${participants.length} participants)`);

  } catch (err) {
    await client.query('ROLLBACK');
    throw err;
  } finally {
    client.release();
  }
}
```

---

#### 5. cls-aggregator (Publisher)

**Language:** Node.js/TypeScript
**Purpose:** Publish merkle roots on-chain

**Key Code:**
```typescript
// apps/twzrd-aggregator/src/auto-publish.ts
import { Connection, Keypair, PublicKey } from '@solana/web3.js';

const connection = new Connection(RPC_URL);
const payer = Keypair.fromSecretKey(
  JSON.parse(fs.readFileSync(PAYER_KEYPAIR, 'utf8'))
);

async function publishLoop() {
  while (true) {
    // Fetch unpublished epochs
    const unpublished = await db.query(`
      SELECT epoch, channel, root, participant_count
      FROM sealed_epochs
      WHERE published = false
        AND token_group = 'CLS'
      ORDER BY epoch ASC
      LIMIT 10
    `);

    for (const row of unpublished.rows) {
      try {
        // Check if channel is initialized
        const channelPda = deriveChannelPda(row.channel);
        const accountInfo = await connection.getAccountInfo(channelPda);

        if (!accountInfo && PUBLISH_REQUIRE_INITIALIZED) {
          console.log(`Skipping ${row.channel} (not initialized, strict mode)`);
          continue;
        }

        // Publish on-chain
        await publishRootRing(
          row.channel,
          row.epoch,
          row.root,
          row.participant_count
        );

        // Mark as published
        await db.query(
          `UPDATE sealed_epochs
           SET published = true, published_at = NOW()
           WHERE epoch = $1 AND channel = $2`,
          [row.epoch, row.channel]
        );

        console.log(`Published: ${row.channel} epoch ${row.epoch}`);

      } catch (err) {
        console.error(`Failed to publish ${row.channel}:`, err);
      }
    }

    await sleep(60000); // Check every minute
  }
}

async function publishRootRing(
  channel: string,
  epoch: number,
  root: string,
  participantCount: number
) {
  const streamerKey = deriveStreamerKey(channel);
  const channelPda = deriveChannelPda(channel);

  // Build instruction
  const ix = await program.methods
    .setMerkleRootRing(
      channel,
      new BN(epoch),
      Buffer.from(root, 'hex'),
      new BN(participantCount * 1_000_000_000) // Total claimable
    )
    .accounts({
      payer: payer.publicKey,
      protocolState: protocolPda,
      channelState: channelPda,
      systemProgram: SystemProgram.programId,
    })
    .instruction();

  // Send transaction
  const tx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 300_000 }),
    ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 50_000 }),
    ix
  );

  const sig = await sendAndConfirmTransaction(connection, tx, [payer]);
  return sig;
}
```

**Key Derivation:**
```typescript
function deriveStreamerKey(channel: string): PublicKey {
  const hash = keccak256
    .create()
    .update('channel:')
    .update(channel.toLowerCase())
    .digest();
  return new PublicKey(hash);
}

function deriveChannelPda(channel: string): PublicKey {
  const streamerKey = deriveStreamerKey(channel);
  const [pda] = PublicKey.findProgramAddressSync(
    [
      Buffer.from('channel_state'),
      MINT.toBuffer(),
      streamerKey.toBuffer(),
    ],
    PROGRAM_ID
  );
  return pda;
}
```

---

#### 6. gateway (Proof API)

**Language:** Node.js/TypeScript (Fastify)
**Purpose:** Serve merkle proofs to users

**Endpoints:**

**GET /proof**
```typescript
// apps/gateway/src/routes/proof.ts
app.get('/proof', async (req, reply) => {
  const { user, channel, epoch } = req.query;

  // Query sealed participant
  const result = await db.query(
    `SELECT index, amount, proof
     FROM sealed_participants
     WHERE username = $1
       AND channel = $2
       AND epoch = $3
       AND token_group = 'CLS'`,
    [user, channel, epoch]
  );

  if (result.rows.length === 0) {
    return reply.code(404).send({ error: 'Proof not found' });
  }

  const row = result.rows[0];

  // Resolve wallet (optional - can be done client-side)
  const wallet = await resolveWallet(user); // Twitch OAuth -> Solana

  return {
    claimer: wallet || '11111111111111111111111111111111',
    mint: MINT.toBase58(),
    channel,
    epoch: parseInt(epoch),
    index: row.index,
    amount: row.amount,
    id: user, // Username
    root: await getRoot(channel, epoch),
    proof: row.proof, // JSON array of hex strings
  };
});
```

**GET /claim-root**
```typescript
app.get('/claim-root', async (req, reply) => {
  const { channel, epoch } = req.query;

  const result = await db.query(
    `SELECT root, participant_count
     FROM sealed_epochs
     WHERE channel = $1 AND epoch = $2 AND token_group = 'CLS'`,
    [channel, epoch]
  );

  if (result.rows.length === 0) {
    return reply.code(404).send({ error: 'Epoch not found' });
  }

  return {
    root: '0x' + result.rows[0].root,
    participantCount: result.rows[0].participant_count,
  };
});
```

---

## Data Flow

### End-to-End: Viewer → Claim

```
1. INGESTION (Real-time)
   Viewer watches stream
   ↓
   stream-listener detects presence
   ↓
   Redis event published
   ↓
   cls-worker writes to participation_events table

2. SEALING (Hourly)
   Epoch closes (e.g., 12:00:00 → 12:59:59 ends)
   ↓
   epoch-watcher detects unsealed epoch
   ↓
   tree-builder queries participants, builds merkle tree
   ↓
   Writes to sealed_participants + sealed_epochs

3. PUBLISHING (Automated)
   cls-aggregator polls for unpublished epochs
   ↓
   Checks if channel is initialized (strict mode)
   ↓
   Builds Solana transaction (set_merkle_root_ring)
   ↓
   Signs with publisher keypair (87d5...ufdy)
   ↓
   Sends to Solana RPC
   ↓
   Marks epoch as published in database

4. CLAIMING (User-initiated)
   User opens claim UI
   ↓
   Connects Phantom wallet
   ↓
   UI fetches proof from gateway API (/proof?user=X&channel=Y&epoch=Z)
   ↓
   UI builds claim_with_ring transaction
   ↓
   User signs with wallet
   ↓
   Transaction sent to Solana
   ↓
   Program verifies merkle proof
   ↓
   Tokens transferred from treasury to user ATA
```

---

## Key Algorithms

### 1. Merkle Tree Construction

**Purpose:** Efficiently commit to large participant sets with ~10KB root

**Algorithm:**
```typescript
// Pseudocode
function buildTree(participants: Participant[]): MerkleTree {
  // 1. Sort by username (deterministic ordering)
  const sorted = participants.sort((a, b) => a.username.localeCompare(b.username));

  // 2. Compute leaves
  const leaves = sorted.map((p, index) => {
    return keccak256(
      p.wallet_pubkey +  // 32 bytes
      p.username +        // Variable UTF-8
      uint32(index) +     // 4 bytes
      uint64(amount)      // 8 bytes
    );
  });

  // 3. Build tree (bottom-up)
  let level = leaves;
  while (level.length > 1) {
    const nextLevel = [];
    for (let i = 0; i < level.length; i += 2) {
      const left = level[i];
      const right = i + 1 < level.length ? level[i + 1] : left; // Duplicate if odd

      // Sort pair (canonical ordering)
      const [a, b] = left < right ? [left, right] : [right, left];
      const parent = keccak256(a + b);
      nextLevel.push(parent);
    }
    level = nextLevel;
  }

  return { root: level[0], leaves };
}
```

**Proof Generation:**
```typescript
function getProof(tree: MerkleTree, leafIndex: number): Hash[] {
  const proof = [];
  let index = leafIndex;
  let level = tree.leaves;

  while (level.length > 1) {
    const isRightNode = index % 2 === 1;
    const siblingIndex = isRightNode ? index - 1 : index + 1;

    if (siblingIndex < level.length) {
      proof.push(level[siblingIndex]);
    }

    index = Math.floor(index / 2);
    level = computeNextLevel(level);
  }

  return proof;
}
```

**Proof Verification (On-Chain):**
```rust
fn verify_proof(proof: &[[u8; 32]], root: [u8; 32], leaf: [u8; 32]) -> bool {
    let mut computed_hash = leaf;

    for proof_element in proof.iter() {
        // Canonical ordering: smaller hash goes first
        if computed_hash <= *proof_element {
            computed_hash = keccak256(&[computed_hash, proof_element]);
        } else {
            computed_hash = keccak256(&[proof_element, computed_hash]);
        }
    }

    computed_hash == root
}
```

**Complexity:**
- Tree construction: O(n log n) - sort + build
- Proof generation: O(log n) - height of tree
- Proof verification: O(log n) - on-chain
- Storage: O(n) off-chain, O(1) on-chain (just root)

---

### 2. Ring Buffer (Channel State)

**Purpose:** Fixed-size storage, prevents state bloat

**Implementation:**
```rust
pub const CHANNEL_RING_SLOTS: usize = 10;

#[zero_copy]
pub struct ChannelState {
    pub ring_head: u16, // Next write position (0-9)
    pub slots: [ChannelSlot; CHANNEL_RING_SLOTS],
}

// Publishing
fn publish_epoch(channel_state: &mut ChannelState, epoch_data: ChannelSlot) {
    let slot_idx = channel_state.ring_head as usize;

    // Overwrite oldest slot
    channel_state.slots[slot_idx] = epoch_data;

    // Advance head (circular)
    channel_state.ring_head = (channel_state.ring_head + 1) % CHANNEL_RING_SLOTS as u16;
}

// Claiming
fn find_epoch(channel_state: &ChannelState, epoch: u64) -> Option<&ChannelSlot> {
    channel_state.slots.iter().find(|slot| slot.epoch == epoch)
}
```

**Characteristics:**
- **Fixed size:** 10 epochs × 616 bytes = 6.16 KB per channel
- **No reallocation:** Zero-copy, memcpy safe
- **Eviction:** Oldest epoch overwritten when buffer full
- **Claim window:** ~10 hours (for hourly epochs)

**Trade-offs:**
- ✅ Predictable rent (0.04002 SOL per channel, never grows)
- ✅ Fast writes (no dynamic allocation)
- ❌ Limited claim window (old epochs evicted)
- ❌ Users must claim within ~10 epochs

---

### 3. Bitmap Claiming

**Purpose:** Track which participants have claimed (space-efficient)

**Implementation:**
```rust
pub const CHANNEL_BITMAP_BYTES: usize = 512; // 512 bytes = 4096 bits
pub const CHANNEL_MAX_CLAIMS: usize = CHANNEL_BITMAP_BYTES * 8; // 4096 participants/epoch

#[zero_copy]
pub struct ChannelSlot {
    pub bitmap: [u8; CHANNEL_BITMAP_BYTES], // 512 bytes
    // ... other fields
}

// Check if already claimed
fn is_claimed(slot: &ChannelSlot, index: u32) -> bool {
    let byte_idx = (index / 8) as usize;
    let bit_idx = (index % 8) as u8;
    (slot.bitmap[byte_idx] & (1 << bit_idx)) != 0
}

// Mark as claimed
fn mark_claimed(slot: &mut ChannelSlot, index: u32) {
    let byte_idx = (index / 8) as usize;
    let bit_idx = (index % 8) as u8;
    slot.bitmap[byte_idx] |= 1 << bit_idx;
}
```

**Limits:**
- Max 4096 participants per epoch (per channel)
- 512 bytes per bitmap (fixed)
- O(1) claim check (single bit read)
- No external lookups required

**Handling >4096 participants:**
- Current: Limit enforced at sealing (top 4096 by score)
- Future: Multiple rings or pagination

---

## Security Model

### On-Chain Security

#### 1. Authorization

**Protocol Admin:**
- Can update protocol config
- Can pause/unpause
- Can transfer admin role
- **Cannot** steal tokens (treasury PDA controlled by program)

**Publisher:**
- Can publish merkle roots
- **Cannot** update config
- **Cannot** pause protocol
- **Cannot** directly transfer tokens

**Claimer:**
- Can claim tokens with valid proof
- **Cannot** claim twice (bitmap enforcement)
- **Cannot** claim without proof
- **Cannot** claim from other users

**Code:**
```rust
fn authorize_publisher(protocol: &ProtocolState, signer: &Pubkey) -> Result<()> {
    let is_admin = *signer == protocol.admin;
    let is_publisher = protocol.publisher != Pubkey::default()
                       && *signer == protocol.publisher;
    require!(is_admin || is_publisher, ProtocolError::Unauthorized);
    Ok(())
}
```

#### 2. Treasury Security

**PDA-Controlled:**
```rust
// Treasury is a PDA, not a keypair
let (treasury_pda, bump) = Pubkey::find_program_address(
    &[b"treasury", mint.as_ref()],
    program_id
);

// Only program can sign for treasury
let treasury_seeds = &[
    b"treasury",
    mint.as_ref(),
    &[bump]
];

// Transfer requires PDA signer
transfer_checked(
    CpiContext::new_with_signer(
        token_program,
        TransferChecked { from: treasury_ata, to: user_ata, authority: treasury_pda, mint },
        &[treasury_seeds] // PDA signer seeds
    ),
    amount,
    decimals
)?;
```

**Implications:**
- No private key for treasury (cannot be stolen)
- Only program logic can move funds
- Funds safe even if admin compromised

#### 3. Merkle Proof Security

**Prevents:**
- ❌ Claiming without participation (invalid proof)
- ❌ Claiming more than allocated (amount in leaf)
- ❌ Claiming twice (bitmap)
- ❌ Claiming as different user (wallet in leaf)

**Relies on:**
- Publisher honesty (merkle root correctness)
- Keccak256 collision resistance
- Off-chain proof generation integrity

**Attack Vectors:**
- ✅ Malicious publisher: Mitigated by admin control (can rotate publisher)
- ✅ Front-running claims: Not possible (each user has unique proof)
- ✅ Replay attacks: Bitmap prevents double-claim
- ⚠️ Publisher collusion: Admin must be trusted (future: DAO/multisig)

---

### Off-Chain Security

#### 1. Database Integrity

**Protections:**
- SSL connections (required)
- Managed PostgreSQL (DigitalOcean)
- Row-level locking (sealing transactions)
- Indexes on critical queries

**Risks:**
- Database compromise → can generate fake proofs (but can't publish)
- Need: Regular backups, audit logs

#### 2. Publisher Key Security

**Current Setup:**
```
Keypair: /home/twzrd/.config/solana/oracle-authority.json
Permissions: 0600 (owner read/write only)
Balance: 1.459 SOL
Used by: cls-aggregator PM2 process
```

**Protections:**
- File permissions (restricted)
- Server access control (SSH keys)
- Balance monitoring (alerts on low balance)

**Risks:**
- Server compromise → attacker can publish fake roots
- Mitigation: Monitoring, regular rotation, multisig (future)

#### 3. API Security (Gateway)

**Current:**
- Read-only API (no mutations)
- Rate limiting (TBD)
- CORS (configured)

**Needed:**
- API key authentication
- Rate limiting per IP/user
- DDoS protection (Cloudflare)

---

## Integration Guide

### For Developers

#### 1. Claiming Tokens (Client-Side)

**Step 1: Fetch Proof**
```typescript
const response = await fetch(
  `https://api.twzrd.xyz/proof?user=${username}&channel=${channel}&epoch=${epoch}`
);
const proof = await response.json();

// Proof format:
{
  claimer: "7xKX...abc", // Solana pubkey
  mint: "AAHd...GWN5",
  channel: "xqc",
  epoch: 1762495200,
  index: 42,
  amount: "1000000000", // 1 token (9 decimals)
  id: "username",
  root: "0xabcd...",
  proof: ["0x1234...", "0x5678..."] // Merkle proof
}
```

**Step 2: Build Transaction**
```typescript
import {
  Connection,
  PublicKey,
  Transaction,
  TransactionInstruction
} from '@solana/web3.js';
import { getAssociatedTokenAddressSync } from '@solana/spl-token';
import { keccak_256 } from 'js-sha3';

const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const TOKEN_2022_PROGRAM = new PublicKey('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb');

// Derive accounts
const mint = new PublicKey(proof.mint);
const claimer = wallet.publicKey;

const [protocolState] = PublicKey.findProgramAddressSync(
  [Buffer.from('protocol'), mint.toBuffer()],
  PROGRAM_ID
);

const [treasury] = PublicKey.findProgramAddressSync(
  [Buffer.from('treasury'), mint.toBuffer()],
  PROGRAM_ID
);

const streamerKey = deriveStreamerKey(proof.channel);
const [channelState] = PublicKey.findProgramAddressSync(
  [Buffer.from('channel_state'), mint.toBuffer(), streamerKey.toBuffer()],
  PROGRAM_ID
);

const treasuryAta = getAssociatedTokenAddressSync(mint, treasury, true, TOKEN_2022_PROGRAM);
const claimerAta = getAssociatedTokenAddressSync(mint, claimer, false, TOKEN_2022_PROGRAM);

// Serialize instruction data
const discriminator = await computeDiscriminator('claim_with_ring');
const data = serializeClaimData({
  epoch: proof.epoch,
  index: proof.index,
  amount: BigInt(proof.amount),
  proof: proof.proof.map(p => Buffer.from(p.slice(2), 'hex')),
  id: proof.id,
  streamer_key: streamerKey.toBuffer(),
});

const instruction = new TransactionInstruction({
  programId: PROGRAM_ID,
  keys: [
    { pubkey: claimer, isSigner: true, isWritable: true },
    { pubkey: protocolState, isSigner: false, isWritable: false },
    { pubkey: channelState, isSigner: false, isWritable: true },
    { pubkey: mint, isSigner: false, isWritable: true },
    { pubkey: treasury, isSigner: false, isWritable: false },
    { pubkey: treasuryAta, isSigner: false, isWritable: true },
    { pubkey: claimerAta, isSigner: false, isWritable: true },
    { pubkey: TOKEN_2022_PROGRAM, isSigner: false, isWritable: false },
    { pubkey: ASSOCIATED_TOKEN_PROGRAM, isSigner: false, isWritable: false },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
  ],
  data: Buffer.concat([discriminator, data]),
});

const tx = new Transaction().add(instruction);
const signature = await wallet.sendTransaction(tx, connection);
```

**Helper Functions:**
```typescript
function deriveStreamerKey(channel: string): PublicKey {
  const hash = keccak_256
    .create()
    .update('channel:')
    .update(channel.toLowerCase())
    .digest();
  return new PublicKey(hash);
}

async function computeDiscriminator(name: string): Promise<Buffer> {
  const data = new TextEncoder().encode(`global:${name}`);
  const hash = await crypto.subtle.digest('SHA-256', data);
  return Buffer.from(hash).slice(0, 8);
}

function serializeClaimData(args: ClaimArgs): Buffer {
  const buffers: Buffer[] = [];

  // epoch (u64)
  const epochBuf = Buffer.alloc(8);
  epochBuf.writeBigUInt64LE(BigInt(args.epoch));
  buffers.push(epochBuf);

  // index (u32)
  const indexBuf = Buffer.alloc(4);
  indexBuf.writeUInt32LE(args.index);
  buffers.push(indexBuf);

  // amount (u64)
  const amountBuf = Buffer.alloc(8);
  amountBuf.writeBigUInt64LE(args.amount);
  buffers.push(amountBuf);

  // proof (Vec<[u8; 32]>)
  const proofLenBuf = Buffer.alloc(4);
  proofLenBuf.writeUInt32LE(args.proof.length);
  buffers.push(proofLenBuf);
  args.proof.forEach(node => buffers.push(node));

  // id (String)
  const idBytes = Buffer.from(args.id, 'utf8');
  const idLenBuf = Buffer.alloc(4);
  idLenBuf.writeUInt32LE(idBytes.length);
  buffers.push(idLenBuf);
  buffers.push(idBytes);

  // streamer_key (Pubkey)
  buffers.push(args.streamer_key);

  return Buffer.concat(buffers);
}
```

#### 2. Publishing Roots (Server-Side)

**Prerequisites:**
- Publisher keypair with SOL balance
- Access to sealed merkle roots (database/API)

**Code:**
```typescript
import { Connection, Keypair, Transaction, TransactionInstruction } from '@solana/web3.js';

const connection = new Connection('https://api.mainnet-beta.solana.com');
const publisher = Keypair.fromSecretKey(/* ... */);

async function publishRoot(
  channel: string,
  epoch: number,
  root: string, // Hex string
  totalClaimable: bigint
) {
  const streamerKey = deriveStreamerKey(channel);

  const [protocolState] = PublicKey.findProgramAddressSync(
    [Buffer.from('protocol'), mint.toBuffer()],
    PROGRAM_ID
  );

  const [channelState] = PublicKey.findProgramAddressSync(
    [Buffer.from('channel_state'), mint.toBuffer(), streamerKey.toBuffer()],
    PROGRAM_ID
  );

  // Serialize instruction
  const discriminator = await computeDiscriminator('set_merkle_root_ring');
  const data = Buffer.concat([
    discriminator,
    Buffer.from(channel, 'utf8'),
    Buffer.from(new BigUint64Array([BigInt(epoch)]).buffer),
    Buffer.from(root, 'hex'),
    Buffer.from(new BigUint64Array([totalClaimable]).buffer),
  ]);

  const ix = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: publisher.publicKey, isSigner: true, isWritable: true },
      { pubkey: protocolState, isSigner: false, isWritable: true },
      { pubkey: channelState, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });

  const tx = new Transaction().add(ix);
  const sig = await sendAndConfirmTransaction(connection, tx, [publisher]);
  return sig;
}
```

---

## Performance & Scalability

### Current Limits

**On-Chain:**
- Channels initialized: 15 (no theoretical limit)
- Participants per epoch: 4,096 (bitmap size)
- Epochs per channel: 10 (ring buffer)
- Transaction size: ~1.2 KB (fits in 1232 byte limit)

**Off-Chain:**
- Database: 100K+ participants per hour (tested)
- API: 1000+ req/sec (Fastify, not load tested)
- Publisher: 10-20 tx/min (RPC rate limits)

### Scalability Improvements

**Short-term:**
- Batch publishing (multiple channels per tx)
- Increase bitmap to 1024 bytes (8,192 participants)
- Multiple publishers (round-robin)

**Long-term:**
- Sharding by channel (separate databases)
- CDNN for proof serving (Cloudflare)
- L2 aggregation (publish batches to L1)

---

## Monitoring & Observability

### Key Metrics

**On-Chain:**
- Channels initialized
- Epochs published per hour
- Claims per hour
- Treasury balance
- Publisher balance

**Off-Chain:**
- Participation events ingested
- Epochs sealed per hour
- Unpublished epoch backlog
- Database size/growth
- API latency (p50, p95, p99)

### Logs

**PM2 Logs:**
```bash
pm2 logs cls-aggregator  # Publisher logs
pm2 logs gateway         # API logs
pm2 logs cls-worker-s0   # Ingestion logs
```

**Database Queries:**
```sql
-- Unpublished backlog
SELECT channel, COUNT(*)
FROM sealed_epochs
WHERE published = false
GROUP BY channel;

-- Recent claims
SELECT COUNT(*)
FROM sealed_participants
WHERE proof_used_at > NOW() - INTERVAL '1 hour';

-- Top channels by participants
SELECT channel, SUM(participant_count) as total
FROM sealed_epochs
GROUP BY channel
ORDER BY total DESC
LIMIT 10;
```

---

## Deployment Checklist

### New Channel

- [ ] Channel appears in participation_events
- [ ] Epoch sealed (sealed_epochs row)
- [ ] Merkle tree built (sealed_participants rows)
- [ ] Channel PDA initialized (~0.04 SOL)
- [ ] First epoch published (on-chain)
- [ ] Gateway serves proofs (/proof endpoint)
- [ ] Users can claim (UI working)

### System Upgrade

- [ ] Test on devnet
- [ ] Audit code changes
- [ ] Deploy program update (if needed)
- [ ] Restart PM2 services
- [ ] Monitor logs for errors
- [ ] Verify publishing continues
- [ ] Check claim flow

---

## Contact & Resources

**Documentation:**
- This file: `/home/twzrd/milo-token/TECHNICAL_ARCHITECTURE.md`
- Wallet map: `/home/twzrd/milo-token/WALLET_MAP.md`
- Session summary: `/home/twzrd/milo-token/SESSION_SUMMARY_2025-11-07.md`

**Code Repositories:**
- Program: `/home/twzrd/milo-token/programs/token-2022/`
- Aggregator: `/home/twzrd/milo-token/apps/twzrd-aggregator/`
- Gateway: `/home/twzrd/milo-token/apps/gateway/`
- Claim UI: `/home/twzrd/milo-token/apps/claim-ui/`

**On-Chain:**
- Program: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- Explorer: https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

---

Last Updated: 2025-11-07 20:00 UTC
