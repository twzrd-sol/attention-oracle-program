# Target Architecture: Parallel Rebuild

**Status:** Strangler Fig Pattern - Legacy and New services run in parallel
**Decision Date:** 2025-11-18
**Program ID (Immutable):** `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
**SDK Version:** v0.2.1-clean

---

## 1. Stream Listener (The Edge)

### Role
Ingest Solana blockchain events from the Attention Oracle program.
Transform into normalized messages for consumption by aggregator.

### Current Status
- ✅ Running (legacy compiled JS, 6 days uptime)
- Location: `/home/twzrd/milo-token/stream-listener/dist/index.js`
- Outputs: NDJSON logs + HTTP push to gateway

### Target Status (New Build)
- Fresh TypeScript, clean SDK dependency
- Same input (Solana events), same output (NDJSON + queue)
- Stateless, horizontally scalable
- Better observability and logging

### Key Interfaces

**Input:**
```
Solana RPC WebSocket
  → Program subscription via @solana/web3.js
  → Filter for program events (GnGzNdsQM...)
```

**Output (Queue):**
```typescript
interface StreamEvent {
  timestamp: string;
  slot: number;
  signature: string;
  tx: ParsedTransaction;
  instruction: {
    program: "Attention Oracle";
    action: "claim" | "finalize" | "setPolicy";
    data: Record<string, unknown>;
  };
}

// Via BullMQ / Redis:
queue.add("stream:event", event, {
  attempts: 3,
  backoff: { type: "exponential", delay: 2000 }
});

// Via NDJSON log:
fs.appendFileSync(logPath, JSON.stringify(event) + "\n");
```

**Configuration:**
```env
ANCHOR_PROVIDER_URL=https://api.mainnet-beta.solana.com
AO_PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
REDIS_URL=redis://localhost:6379
BULLMQ_PREFIX=twzrd

# Optional: Push to external aggregator
AGGREGATOR_URL=http://localhost:8000
AGGREGATOR_TOKEN=<secret>
```

### Dependencies
- `@solana/web3.js` ^1.95.0
- `@attention-oracle/sdk` v0.2.1-clean (via GitHub tag)
- `bullmq` ^5.0.0 (queue)
- `pino` ^8.17.0 (logging)
- `dotenv` ^16.0.0

### Success Criteria
- [ ] Receives events within 1 block of settlement
- [ ] No duplicate event processing
- [ ] Queue depth < 100 (sustains Solana block rate)
- [ ] Logs rotate, no unbounded growth
- [ ] Graceful shutdown on SIGTERM

---

## 2. Aggregator (The Core)

### Role
1. Consume stream events from queue
2. Build Merkle trees per channel/epoch
3. Finalize roots and write to PostgreSQL
4. Coordinate settlement on-chain (via SDK CPI)

### Current Status
- 🟡 Running (legacy compiled JS, frequent restarts: 142 total)
- Location: `/home/twzrd/milo-token/apps/twzrd-aggregator/dist/server.js`
- Dependencies: PostgreSQL (production), SQLite (cache)

### Target Status (New Build)
- Fresh TypeScript/Rust, clean SDK dependency
- Same database schema (preserve existing data)
- Improved stability and observability
- Can run multiple instances (horizontally scalable)

### Key Interfaces

**Input (from Queue):**
```typescript
// Consume from BullMQ
const job = await claimsQueue.getNextJob();
const event = job.data as StreamEvent;

// Validate & store
await db.claims.insert({
  slot: event.slot,
  channel: event.instruction.channel,
  user_pubkey: event.instruction.user,
  claim_timestamp: event.timestamp,
  status: "pending"
});
```

**Processing:**
```typescript
// 1. Batch claims per epoch/channel
const claims = await db.claims.getUnfinalized();

// 2. Build Merkle tree
import { MerkleTree } from "@attention-oracle/sdk";
const tree = new MerkleTree(claims);
const root = tree.root();

// 3. Finalize (write PostgreSQL)
await db.roots.insert({
  epoch: epoch,
  channel: channel,
  root: root,
  count: claims.length,
  finalized_at: new Date(),
  status: "ready_for_settlement"
});

// 4. Emit event for settlement
eventBus.emit("root:finalized", { epoch, channel, root });
```

**Database Schema (PostgreSQL):**
```sql
-- Existing tables (preserve):
TABLE claims (
  id SERIAL PRIMARY KEY,
  channel VARCHAR,
  user_pubkey VARCHAR,
  slot BIGINT,
  claim_timestamp TIMESTAMP,
  status VARCHAR -- pending, included, settled
);

TABLE roots (
  id SERIAL PRIMARY KEY,
  epoch BIGINT,
  channel VARCHAR,
  root VARCHAR, -- hex string
  count INT,
  finalized_at TIMESTAMP,
  status VARCHAR -- ready_for_settlement, settled
);

-- New tables (add):
TABLE epochs (
  epoch BIGINT PRIMARY KEY,
  start_slot BIGINT,
  end_slot BIGINT,
  finalized_at TIMESTAMP
);

TABLE settlement_log (
  id SERIAL PRIMARY KEY,
  root_id INT REFERENCES roots(id),
  tx_signature VARCHAR,
  settled_at TIMESTAMP,
  status VARCHAR
);
```

**Configuration:**
```env
# Database
DATABASE_URL=postgresql://user:pass@localhost:5432/twzrd
# Redis for queue
REDIS_URL=redis://localhost:6379
BULLMQ_PREFIX=twzrd

# Solana
ANCHOR_PROVIDER_URL=https://api.mainnet-beta.solana.com
AO_PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
SETTLEMENT_KEYPAIR=~/.config/solana/settlement-key.json

# Aggregation policy
EPOCH_DURATION_SLOTS=432000  # ~3 days
FINALIZE_THRESHOLD_SECS=300   # Wait 5 min before finalizing
AUTO_PUBLISH=false             # During transition (set to true after validation)

# Monitoring
LOG_LEVEL=info
METRICS_ENDPOINT=http://localhost:9090/metrics
```

### Dependencies
- `@solana/web3.js` ^1.95.0
- `@attention-oracle/sdk` v0.2.1-clean
- `pg` ^8.11.0 (PostgreSQL driver)
- `bullmq` ^5.0.0 (queue consumer)
- `pino` ^8.17.0 (logging)
- `dotenv` ^16.0.0

### Success Criteria
- [ ] Processes queue at Solana block rate (no backlog)
- [ ] Merkle roots match legacy aggregator (byte-for-byte)
- [ ] PostgreSQL stays <1s query time (under load)
- [ ] Scales to 10k+ claims per epoch
- [ ] Zero data loss (crash-recovery tested)
- [ ] Can settle on-chain without errors

---

## 3. Settlement (Program Interaction)

### Role
Call on-chain program to publish finalized Merkle roots.
Update settlement_log with tx signatures.

### Current Status
- Embedded in legacy aggregator
- Called via CPI when root ready

### Target Status
- Separate worker that watches root finalization events
- Retries on failure, tracks on-chain state
- Atomic: only updates DB after confirmed on-chain

### Key Interface

```typescript
import { AttentionOracleClient } from "@attention-oracle/sdk";

async function settleRoot(root: RootRecord) {
  const client = new AttentionOracleClient(connection, PROGRAM_ID);

  try {
    const tx = await client.finalizeRoot({
      epoch: root.epoch,
      channel: root.channel,
      merkleRoot: Buffer.from(root.root, "hex"),
      count: root.count,
      payer: settlementKeypair
    });

    await db.settlement_log.insert({
      root_id: root.id,
      tx_signature: tx,
      status: "pending_confirmation"
    });

    // Wait for confirmation
    await connection.confirmTransaction(tx, "finalized");

    await db.settlement_log.update(tx, {
      status: "confirmed",
      settled_at: new Date()
    });
  } catch (err) {
    await db.settlement_log.insert({
      root_id: root.id,
      error_message: err.message,
      status: "failed"
    });
    throw err; // Retry upstream
  }
}
```

---

## 4. Gateway (API Layer)

### Role
HTTP/WebSocket API for clients to:
- Query claim status
- Verify Merkle proofs
- Poll for settlement confirmation
- Fetch historical data

### Current Status
- 🔴 Running but unstable (176 restarts in 2h)
- Location: `/home/twzrd/.pm2/ecosystem.config.js` → "gateway"

### Target Status
- Fresh TypeScript (Express/Fastify)
- Clean SDK dependency for verification
- Better error handling and monitoring
- OpenAPI documentation

### Key Endpoints

```
GET  /api/v1/claims/:userId
     → { status, proofs, merkle_root, settled }

POST /api/v1/verify
     { claim_id, merkle_path, root }
     → { valid: bool, settled: bool }

GET  /api/v1/epochs/:epoch/channels/:channel
     → { root, count, settled_at, tx_signature }

GET  /health
     → { status, database, queue, program }
```

### Configuration
```env
GATEWAY_PORT=3000
DATABASE_URL=postgresql://...
ANCHOR_PROVIDER_URL=https://api.mainnet-beta.solana.com
AO_PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
LOG_LEVEL=info
```

### Dependencies
- `express` or `fastify` (HTTP framework)
- `@attention-oracle/sdk` v0.2.1-clean
- `pg` ^8.11.0
- `pino` ^8.17.0

### Success Criteria
- [ ] <100ms latency for all endpoints
- [ ] Graceful degradation (if DB down, return cached state)
- [ ] OpenAPI spec auto-generated
- [ ] Can handle 1000 req/s (load test)

---

## 5. Integration Points (Strangler Fig)

### Phase 1: Parallel Run (Week 1-2)
```
Legacy Services          New Services
(Compiled)               (Fresh TS)
   │                         │
   ├─ stream-listener  ×  stream-listener-new
   │  (running)        │  (starting)
   │
   ├─ aggregator       ×  aggregator-new
   │  (PostgreSQL)     │  (same PostgreSQL)
   │
   ├─ gateway          ×  gateway-new
   │  (API)            │  (API on port 3001)
   │
   └─ (auto-publish)      (auto-publish: false)

Validation:
- Compare merkle roots byte-for-byte
- Verify claim counts match
- Check transaction volumes
```

### Phase 2: Cutover (Week 2-3)
```
1. Enable new gateway on port 3000 (after tests pass)
2. Drain legacy queue gracefully
3. Switch DNS/load-balancer to new gateway
4. Monitor old gateway metrics (tail off)
5. Archive legacy compiled binaries
6. Update PM2 ecosystem to point to new services
```

### Phase 3: Cleanup (Week 3-4)
```
1. Decommission legacy compiled services
2. Archive legacy_data_archive_*.tar.gz to cold storage
3. Update documentation
4. Release v0.3.0 with new code as OSS
```

---

## Data Preservation & Recovery

### Backup Schedule (During Transition)
```
Hourly:
  - PostgreSQL transaction log (WAL)
  - Queue snapshot (Redis BGSAVE)

Daily:
  - Full PostgreSQL dump (gzipped)
  - Archive NDJSON logs
  - Verify backups

On-demand (before any cutover):
  - Full snapshot + validation
  - DRI sign-off
```

### Fallback Plan
If new services fail:
1. Stop new services
2. Restart legacy services (from PM2 ecosystem)
3. Restore PostgreSQL from backup if needed
4. Verify claim integrity
5. Post-mortem on new code

---

## Success Metrics

| Metric | Legacy | Target | Timeline |
|--------|--------|--------|----------|
| Stream latency | <1s | <500ms | EOW |
| Aggregator restarts | 142 | <5 per week | By cutover |
| Gateway uptime | 92% | >99.9% | By cutover |
| Merkle root parity | N/A | 100% | Week 1 |
| Settlement latency | Variable | <10 min | By cutover |
| Log growth | 2.8GB/week | <500MB/week | Week 1 |

---

## Owner Assignments

- **Stream Listener:** [TBD - Backend Eng]
- **Aggregator:** [TBD - Backend Eng]
- **Settlement Worker:** [TBD - Backend Eng or Intern]
- **Gateway:** [TBD - Backend Eng]
- **Database/Schema:** [TBD - DBA or Eng Lead]
- **Monitoring/Observability:** [TBD]
- **DRI (Decision Rights):** [TBD - Founder/Tech Lead]

---

## Escape Hatch

If the rebuild stalls (>2 weeks with no progress):
1. Declare "continue with legacy" decision
2. Focus on stabilizing compiled services instead
3. Document operational procedures
4. Plan source code recovery independently

Otherwise: **Commit to parallel rebuild until parity achieved.**

Contact: dev@twzrd.xyz for status updates or blockers.
