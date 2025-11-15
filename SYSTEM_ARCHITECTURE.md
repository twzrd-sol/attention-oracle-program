# MILO-TWZRD System Architecture - First Principles

**Last Updated**: November 12, 2025
**Status**: Production (Live on Mainnet)
**Network**: Solana Mainnet
**Launch Window**: 24-48 hours

---

## 1. INFRASTRUCTURE LAYER

### 1.1 Compute Resources (DigitalOcean)

**Droplet (Primary)**
- Name: `ubuntus2vcpu4gb120gbintelnyc3012025110300-s-8vcpu-32gb-640gb-intel-nyc3-01`
- Specs: 8vCPU / 32GB RAM / 640GB SSD
- Region: NYC3
- IP: 68.183.154.144
- OS: Ubuntu 22.04 LTS
- **Role**: Runs ALL off-chain workers, aggregators, APIs, and claim system

### 1.2 Managed Databases (DigitalOcean)

**PostgreSQL Cluster: `twzrd-prod-postgres`**
- Version: PostgreSQL 16
- Specs: 8GB RAM / 160GB Disk
- Region: NYC3
- Primary Connection: `postgresql://doadmin:...@twzrd-prod-postgres-do-user-21113270-0.f.db.ondigitalocean.com:25061/twzrd-oracle-pool?sslmode=require`
- **Primary DB**: `twzrd-oracle-pool` (default aggregator target)
- **Purpose**: Off-chain data aggregation (channel participation, sealed epochs, tree roots)

**Redis Cluster: `twzrd-bullmq-redis`**
- Version: Valkey 8
- Specs: 1GB RAM / 10GB Disk
- Region: NYC3
- **Purpose**: Job queue for workers (BullMQ)

### 1.3 Local PostgreSQL (Embedded)

**Instance**: Unix socket at `/var/run/postgresql`
- Version: PostgreSQL 14 (Ubuntu system package)
- **Databases**:
  - `twzrd`: Main local DB used by claim API
  - `twzrd_oracle`: Secondary/test DB

---

## 2. DATABASES - DETAILED INVENTORY

### 2.1 DigitalOcean Managed PostgreSQL (`twzrd-prod-postgres`)

#### Database: `twzrd-oracle-pool`
**Primary purpose**: Off-chain data aggregation & sealing

**Key Tables**:
```
channel_participation    2,738,092 rows  (Oct 14-30)  [user_hash, channel, epoch]
sealed_epochs           180+ rows       (Historical)   [epoch, channel, merkle_root]
sealed_participants     318,538 rows    (Unmapped)    [epoch, channel, user_hash]
viewer_snapshots        2,710 rows      (Quality data)
suppression_list        ???             (Blocked users)
suppression_log         ???             (Audit trail)
user_signals            8,629,376 rows  (3GB+)        (Attention signals)
```

**Aggregator Connection**:
- Used by `cls-aggregator` (PM2 ID: 33)
- Used by `cls-worker-s0/s1` (PM2 IDs: 34-35)
- Used by `tree-builder` (PM2 ID: 10)

---

### 2.2 Local PostgreSQL (`twzrd` database)

#### Purpose:
**Claim system source of truth** - Contains eligibility data and is read by the claim API

**Key Tables**:
```
claimable_allocations   6,410 rows      [9,318 unique users ready to claim]
sealed_participants     961,564 rows    [With 174,805 mapped usernames]
user_mapping            9,618 rows      [hash → username]
twitch_users            7 rows          (Legacy test data)
twitch_events_raw       418 rows        (IRC collector test data)
user_signals            (Empty in local version)
user_wallets            3 rows          (Manual test data)
```

**Current State**:
- ✅ 9,318 users staged for claims
- ✅ 2.25M CHAT tokens allocated
- ✅ 174,805 rows have mapped usernames (vs 5,529 before backfill)

**Claim API Connection**:
```javascript
// Fallback logic in clean-hackathon/api/server.ts
if (DATABASE_URL set) {
  Connect to DigitalOcean PostgreSQL with SSL
} else {
  Connect to local Unix socket at /var/run/postgresql (no SSL)
}
```

---

### 2.3 DigitalOcean Redis (`twzrd-bullmq-redis`)

**Purpose**: Async job queue for worker tasks

**Queues** (inferred from worker patterns):
- `channel-discovery`: Find active channels
- `epoch-seal`: Seal historical epochs
- `tree-build`: Build Merkle trees
- `participation-aggregate`: Aggregate channel stats

**Current Usage**: Backing `cls-worker-s0` and `cls-worker-s1`

---

## 3. PM2 WORKERS - RUNNING PROCESSES

### 3.1 Online (Active)

| ID  | Name | Version | Uptime | Restarts | Memory | Purpose |
|-----|------|---------|--------|----------|--------|---------|
| 33  | `cls-aggregator` | 1.0.0 | 17h | 15 | 189.8 MB | Aggregate channel participation; build merkle trees for epochs |
| 34  | `cls-worker-s0` | 0.1.0 | 18m | 5 | 76.5 MB | Process channel participation jobs from BullMQ |
| 35  | `cls-worker-s1` | 0.1.0 | 18m | 5 | 78.0 MB | Process channel participation jobs from BullMQ |
| 27  | `epoch-watcher` | 1.0.0 | 2D | 1 | 50.3 MB | Monitor sealed epochs; publish on-chain |
| 10  | `tree-builder` | 1.0.0 | 4D | 4 | 66.8 MB | Hourly tree sealing; merkle root generation |
| 1   | `stream-listener` | 1.0.0 | 4D | 4 | 79.3 MB | Twitch API listener (legacy) |
| 8   | `gateway` | 1.0.0 | 4D | 567 | 102.1 MB | API Gateway / RPC Proxy |
| 16  | `off-chain-monitor` | N/A | 2D | 2 | 66.5 MB | Health monitoring of aggregator |

### 3.2 Offline (Stopped)

| ID  | Name | Reason |
|-----|------|--------|
| 36  | `cls-discovery` | Stopped (channel discovery disabled) |

### 3.3 NOT Running (But Should Be for Live Data)

- **IRC Collector** (`scripts/twitch-irc-collector.ts`)
  - Status: Stopped
  - Last ran: Nov 12 00:28 UTC (test run)
  - Purpose: Real-time chat collection from Twitch IRC
  - **Critical for launch**: Needed to map new usernames

---

## 4. ON-CHAIN COMPONENTS (Solana Mainnet)

### 4.1 Smart Programs

**CLS Token Program**
- Program ID: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- Flavor: `MILO_OPEN`
- Mint: `AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5`
- Network: Solana Mainnet
- Status: ✅ Live & Tested
- **Program Location**: `clean-hackathon/programs/token-2022/`

**Recent Instruction** (added):
- `close_channel_state` - Capital recovery on program close

### 4.2 Claim Flow (On-Chain)

```
User claims in UI
    ↓
API transfers CHAT from mint to user Phantom wallet
    ↓
Mint authority sign (controlled by program)
    ↓
Transaction published to Mainnet
    ↓
Solana confirms
    ↓
UI shows success + confetti
```

---

## 5. APPLICATION LAYER - SERVICES

### 5.1 Claim API (`clean-hackathon/api/`)

**Location**: `clean-hackathon/api/server.ts` (TypeScript, Express.js)
**Port**: 3000 (localhost) / Running via ts-node
**Database**: Local PostgreSQL (twzrd via Unix socket by default)

**Endpoints**:
```
POST   /auth/twitch/login        - Twitch OAuth
GET    /api/eligibility          - Check user claims (requires Twitch login)
POST   /api/claim                - Mint and transfer tokens to user wallet
GET    /api/debug/claim          - Debug endpoint (ENABLE_DEBUG=true)
GET    /claim-v2                 - Claim UI (HTML)
```

**Key Files**:
- `server.ts` - Main server (claim logic)
- `claim.ts` - Claim transaction builder
- `eligibility.ts` - Check user eligibility
- `auth-twitch.ts` - Twitch OAuth handler

**Status**: ✅ Live & Tested

---

### 5.2 Aggregator Service (`apps/twzrd-aggregator/`)

**Purpose**: Collect participation data → seal epochs → build merkle trees

**Modules**:
- `src/index.ts` - Main entry point
- `src/workers/tree-builder.ts` - Hourly epoch sealing
- `src/workers/discovery.ts` - Channel discovery
- `src/aggregator.ts` - Data aggregation logic

**Database**: DigitalOcean PostgreSQL (`twzrd-oracle-pool`)
**Queue**: BullMQ (via Redis)

**Flow**:
```
Twitch API / IRC Events
    ↓
channel_participation table (insert)
    ↓
[Hourly] tree-builder seals epoch
    ↓
sealed_epochs + sealed_participants (merkle proof)
    ↓
[Async] epoch-watcher publishes roots on-chain
```

**Status**: ✅ Online (PM2 ID: 33)

---

### 5.3 Workers (`apps/worker-v2/`)

**Purpose**: Process async jobs from BullMQ queue

**Workers**:
- `cls-worker-s0` (PM2 ID: 34)
- `cls-worker-s1` (PM2 ID: 35)

**Current Jobs**: Channel participation aggregation

**Status**: ✅ Online (recently restarted, low uptime = 18 minutes)

---

### 5.4 Gateway (`apps/gateway/`)

**Purpose**: RPC proxy / API gateway

**Status**: ✅ Online (PM2 ID: 8, 4 days uptime)

---

## 6. DATA COLLECTION LAYER

### 6.1 Twitch Data Sources

**Active**:
1. `stream-listener` - Legacy Twitch API integration
2. `cls-aggregator` - Pulls participation from DB

**Inactive**:
1. `twitch-irc-collector.ts` - **NEEDS TO BE STARTED FOR LIVE DATA**
   - Location: `scripts/twitch-irc-collector.ts`
   - Collects: Real-time chat from IRC
   - Output: `twitch_events_raw` table + NDJSON exports

---

## 7. SCRIPTS & UTILITIES

### 7.1 Critical Backfill Scripts

| Script | Purpose | Status |
|--------|---------|--------|
| `backfill-usernames.ts` | Populate user_mapping from user_signals | ✅ Available |
| `backfill-usernames.sql` | SQL version of backfill | ✅ Executed Nov 12 |
| `backfill-usernames-v2.sql` | Updated SQL backfill | ✅ Available |

### 7.2 Data Export Scripts

| Script | Purpose | Output |
|--------|---------|--------|
| `generate-channel-audit.ts` | Audit all channels | `channel_audit.csv` |
| `export-channel-status.ts` | Channel status report | CSV |
| `build-master-depth.ts` | Depth scoring | `L3_master_depth.csv` |
| `inventory-comprehensive.ts` | Full inventory | JSON/CSV |

### 7.3 On-Chain Scripts

| Script | Purpose |
|--------|---------|
| `create-chat-mint-mainnet.ts` | Initialize CHAT mint |
| `l3-publish-root-raw.ts` | Publish merkle roots |
| `l3-set-authority-raw.ts` | Set program authority |

---

## 8. CURRENT DATA STATE

### 8.1 Ready for Launch
- ✅ **9,318 users** staged in `claimable_allocations` (local DB)
- ✅ **2.25M CHAT tokens** allocated
- ✅ **Mainnet mint** deployed and verified
- ✅ **Claim API** functional and tested
- ✅ **Claim UI** at `/claim-v2` with confetti animation

### 8.2 Historical Data (Offline)
- Last live data: October 30, 2025
- Epochs collected: 179 unique
- Unique participants: 2,448 (mapped) + 283K (unmapped hashes)
- Channels: 50+

### 8.3 Data Gap Issues
| Issue | Impact | Mitigation |
|-------|--------|-----------|
| No new data since Oct 30 | Can't stage new users | Start IRC collector NOW |
| 283K unmapped hashes | Lost ~96.7% of participation | Collect new usernames via login |
| Database split (local vs DigitalOcean) | Confusion over source of truth | Align aggregator & API to same DB |

---

## 9. NETWORK CONNECTIVITY

### 9.1 Solana RPC Endpoints
```
Primary:  https://solana-mainnet.api.Helius.io/ (Helius)
Fallback: https://mainnet.helius-rpc.com/ (Helius)
WebSocket: wss://mainnet.helius-rpc.com/ (for on-chain events)
```

### 9.2 Database Connectivity
```
DigitalOcean:  twzrd-prod-postgres (PostgreSQL 16, SSL required)
Local:         /var/run/postgresql (Unix socket, no SSL)
Redis:         twzrd-bullmq-redis (Valkey 8)
```

---

## 10. LAUNCH CHECKLIST

### Critical Path (Must Do)

- [ ] **Start IRC Collector** - `npx tsx scripts/twitch-irc-collector.ts`
- [ ] **Verify API** - Test /claim-v2 flow with test user
- [ ] **Restart Workers** - `pm2 restart all` to clear any stale state
- [ ] **Monitor Data Flow** - Ensure `twitch_events_raw` gets new rows

### Optional Pre-Launch Tuning

- [ ] Start `cls-discovery` (currently stopped)
- [ ] Align aggregator to local DB for consistency (currently on DigitalOcean)
- [ ] Run comprehensive audit script

---

## 11. KNOWN ISSUES

| Issue | Severity | Status | Fix |
|-------|----------|--------|-----|
| No live data since Oct 30 | CRITICAL | Unfixed | Start IRC collector |
| Database split (2 systems) | HIGH | Unfixed | Migrate aggregator to local DB or vice versa |
| cls-discovery stopped | MEDIUM | Unfixed | `pm2 restart cls-discovery` |
| Workers restarting (5 restarts each) | MEDIUM | Acceptable | Monitor for stability |
| Gateway 567 restarts in 4 days | MEDIUM | Chronic issue | Investigate RPC errors |

---

## 12. FILE STRUCTURE

```
/home/twzrd/milo-token/
├── clean-hackathon/
│   ├── api/                    # Claim API service
│   ├── programs/token-2022/    # Solana CLS program
│   ├── scripts/                # Data collection & utilities
│   ├── exports/                # CSV/JSON data exports
│   └── public/claim-v2.html    # Claim UI
├── apps/
│   ├── twzrd-aggregator/       # Off-chain aggregator
│   ├── worker-v2/              # BullMQ workers
│   ├── gateway/                # RPC gateway
│   └── claim-ui/               # Alternate claim UI
├── programs/                   # Other Solana programs
├── packages/sdk/               # SDK library
└── .env                        # Configuration
```

---

## 13. DEPLOYMENT STATUS

| Component | Environment | Status | Last Update |
|-----------|-------------|--------|-------------|
| CLS Program | Mainnet | ✅ Live | Oct 18, 2025 |
| CHAT Mint | Mainnet | ✅ Live | Oct 18, 2025 |
| Claim API | Mainnet | ✅ Live | Nov 12, 2025 |
| Aggregator | DigitalOcean | ✅ Running | Now (17h uptime) |
| Data Collector | N/A | ❌ Stopped | Last: Nov 12 00:28 |

---

## 14. NEXT IMMEDIATE STEPS

**Today (Nov 12, Launch -1 to -0 days)**:

1. **Start Live Data Collection** (5 minutes)
   ```bash
   cd /home/twzrd/milo-token
   npx tsx scripts/twitch-irc-collector.ts &
   ```

2. **Verify Full Claim Flow** (10 minutes)
   - Test login at /claim-v2
   - Submit claim with test user
   - Verify CHAT received in Phantom

3. **Monitor Data Ingestion** (ongoing)
   ```bash
   psql -U twzrd -h /var/run/postgresql -d twzrd -c "SELECT COUNT(*) FROM twitch_events_raw WHERE ts > NOW() - INTERVAL '5 minutes';"
   ```

4. **Decision Point**: Aggregate to Local DB or Keep Split?
   - Option A: Migrate aggregator to local DB (simpler, single source of truth)
   - Option B: Keep DigitalOcean, replicate to local for claims (redundant but safer)

---

**End of Architecture Document**
