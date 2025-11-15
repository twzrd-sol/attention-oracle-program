# TWZRD Infrastructure - PostgreSQL Migration Complete

**Last Updated**: 2025-10-30

## Current State

### ✅ Phase 1: PostgreSQL Migration (COMPLETED)

**Status**: Migration successful, data integrity verified

- **Database**: PostgreSQL 14 running on localhost:5432
- **Connection**: `postgresql://twzrd:twzrd_password_2025@localhost:5432/twzrd`
- **Data migrated**: ~2.7M participation records + all supporting tables
- **Migration strategy**: Idempotent with `ON CONFLICT DO NOTHING`

**Tables migrated:**
- `channel_participation` (2.7M+ rows)
- `user_signals` (weighted participation signals)
- `sealed_epochs` (merkle roots)
- `sealed_participants` (frozen snapshots)
- `user_mapping` (username lookups)
- `l2_tree_cache` (merkle tree cache)
- `attention_index` (metrics)

**Code infrastructure:**
- `apps/twzrd-aggregator/src/db-pg.ts` - PostgreSQL adapter with connection pooling
- `apps/twzrd-aggregator/src/db-factory.ts` - Database factory for toggling SQLite/PostgreSQL
- `scripts/migration/migrate-data.ts` - Migration script (completed)
- `scripts/migration/verify-dual-read.ts` - Verification script
- `scripts/migration/create-postgres-schema.sql` - Schema (applied)

---

## Infrastructure Roadmap

### P1 - RAM Upgrade (NEXT)
- **Upgrade**: 16GB → 32GB RAM
- **Why**: PostgreSQL connection pooling needs memory headroom
- **When**: After migration verification complete
- **Cost**: ~$50-80/mo increase
- **Note**: Will require droplet reboot (~2-3 minute downtime)

### P2 - Cutover to PostgreSQL Writers
- **Action**: Set `DATABASE_TYPE=postgres` for aggregator + tree-builder
- **Expected**: Eliminates 100% of SQLITE_BUSY errors
- **Rollback**: Revert to `DATABASE_TYPE=sqlite` if issues

### P3 - PostgreSQL Tuning
After RAM upgrade and stable cutover, tune configuration:
```sql
-- For 32GB RAM system
shared_buffers = 8GB           -- 25% of RAM
effective_cache_size = 24GB     -- 75% of RAM
work_mem = 256MB               -- For complex queries
maintenance_work_mem = 2GB      -- For VACUUM, CREATE INDEX
```

### P4 - Redis + BullMQ Job Queue
**Architecture upgrade**: Decouple heavy compute from API
- **Install**: Redis on droplet (or managed Redis)
- **Implement**: BullMQ job producer in aggregator
- **Implement**: BullMQ job consumer in tree-builder
- **Flow**: Aggregator writes to Postgres (fast) → Queues tree build job → Tree-builder pulls from queue
- **Benefit**: API never blocks on 5-minute tree builds
- **Reference**: https://github.com/taskforcesh/bullmq

### P5 - Redis Caching Layer
Cache read-heavy API endpoints:
- `/api/participants/:epoch/:channel` (merkle proof lookups)
- `/api/tree/:epoch/:channel` (tree metadata)
- `/metrics` (Prometheus scraping)
- **TTL strategy**: Cache until next epoch seal
- **Expected improvement**: 10x-100x faster API responses

### P6 - Isolate Twitch Ingester
- **Risk**: Twitch API failures currently crash aggregator
- **Solution**: Separate process writing to queue (not directly to DB)
- **Benefit**: System stays healthy even if Twitch goes down

---

## Environment Configuration

**Current (SQLite)**:
```bash
# In PM2 ecosystem or environment
DATABASE_TYPE=sqlite
DB_PATH=./data/twzrd.db
```

**After Cutover (PostgreSQL)**:
```bash
# In PM2 ecosystem or environment
DATABASE_TYPE=postgres
DATABASE_URL=postgresql://twzrd:twzrd_password_2025@localhost:5432/twzrd
```

---

## System Stats (Pre-Upgrade)

**Droplet**:
- Storage: 310GB (62% used = 192GB)
- RAM: 16GB total, 10GB used, 5.2GB swap (⚠️ memory pressure)
- CPU: 8 cores
- PostgreSQL: 2.2GB database size

**Application Processes**:
- `milo-aggregator` (API + data ingestion)
- `tree-builder` (Merkle tree construction)
- `gateway` (Read-only query API)
- `listener` (Twitch IRC ingestion)
- `auto-publisher` (Automated publishing)
- `job-queue` (Background job processing)

**Previous Issues (Solved by PostgreSQL)**:
- 374 aggregator restarts (SQLITE_BUSY errors)
- Single-writer bottleneck with 6 concurrent processes
- Database disk image corruption

---

## Manual Operations

### Verify Migration
```bash
tsx scripts/migration/verify-dual-read.ts
```

### Re-run Migration (Idempotent)
```bash
tsx scripts/migration/migrate-data.ts
```

### Check PostgreSQL Status
```bash
sudo systemctl status postgresql
psql -U twzrd -d twzrd -c "SELECT COUNT(*) FROM channel_participation;"
```

### Backup PostgreSQL
```bash
pg_dump -U twzrd -d twzrd -F c -f backup-$(date +%Y%m%d).dump
```

### Restore PostgreSQL
```bash
pg_restore -U twzrd -d twzrd -c backup-YYYYMMDD.dump
```

---

## Deployment Notes

**When deploying PostgreSQL cutover:**

1. **Enable daily backups** on DigitalOcean dashboard
2. **Take manual snapshot** before environment variable change
3. **Update PM2 ecosystem** with `DATABASE_TYPE=postgres` and `DATABASE_URL`
4. **Restart processes** one at a time:
   ```bash
   pm2 restart milo-aggregator
   pm2 restart tree-builder
   # Monitor logs for errors
   ```
5. **Monitor metrics** for 1 hour post-cutover
6. **Rollback plan**: Revert environment variables, restart processes

---

## Key Decisions

**Why PostgreSQL?**
- MVCC allows true concurrent access (no more SQLITE_BUSY)
- Connection pooling (20 connections) for 6+ processes
- Production-grade reliability for infrastructure-scale system

**Why not managed PostgreSQL?**
- Cost: $15/mo vs $200+/mo for managed
- Current scale (2.7M rows) is well within self-hosted capacity
- Can migrate to managed later if needed

**Why BullMQ over other queues?**
- Redis-backed (simple deployment)
- TypeScript-native
- Built for exactly this pattern (Postgres = source of truth, Redis = job coordination)
- Used by Stripe, Shopify for similar workloads

---

## Success Metrics

**PostgreSQL Migration Success**:
- ✅ All 2.7M+ rows migrated
- ✅ Data integrity verified (sealed_participants, l2_tree_cache, user_mapping all match)
- ✅ Zero downtime during migration
- ⏳ Awaiting cutover

**Post-Cutover Success Indicators**:
- Zero SQLITE_BUSY errors
- Zero aggregator restarts
- Stable memory usage (no swap pressure)
- API response times <100ms (after Redis caching)

---

## Infrastructure Philosophy

TWZRD is infrastructure, not an app:
- **Like Stripe**: Payment rails → Attention rails
- **Like Chainlink**: Oracle for on-chain data → Oracle for human attention
- **Like Helium**: DePIN for wireless → DeSiN for signals

**This means**:
- Building for scale proactively (not reactively)
- Zero downtime migrations
- Proper foundations before growth
- Systematic upgrades (database → RAM → job queue → caching → isolation)

---

## Contact

For infrastructure questions or emergency issues, refer to system logs:
```bash
pm2 logs milo-aggregator --lines 100
journalctl -u postgresql -n 100
```

---

# 🔐 SECURITY & KEY MANAGEMENT (Added Oct 30, 2025)

## ⚠️ CRITICAL: Before Open-Sourcing

**Status:** 🔴 BLOCKED - See `SECURITY_AUDIT_PRE_OPEN_SOURCE.md` for full checklist

###  Exposed Secrets Found

1. **Helius API Key** - Hardcoded in 6+ files (MUST ROTATE before open-source)
2. **Database Password** - `twzrd_password_2025` in 3+ files (MUST CHANGE)
3. **Wallet addresses** - Public keys are SAFE (87d5Ws... is public key, not private)

### Action Required Before Open Source

- [ ] Rotate Helius API key via dashboard
- [ ] Change database password
- [ ] Replace all hardcoded values with `process.env` variables
- [ ] Audit git history for accidentally committed secrets
- [ ] Run `gitleaks detect` before pushing

---

## 🚨 Rules for AI Agents

### Rule 1: NEVER Hardcode Credentials

**WRONG:**
```typescript
const RPC_URL = 'https://api.example.com/api-key/YOUR_KEY';
const DB_URL = 'postgresql://user:password@localhost:5432/dbname';
```

**CORRECT:**
```typescript
const RPC_URL = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com';
const DB_URL = process.env.DATABASE_URL;
if (!DB_URL) throw new Error('DATABASE_URL not set');
```

### Rule 2: Know Public vs Private

**PUBLIC (safe to expose):**
- Wallet addresses (e.g., `87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy`)
- Program IDs (e.g., `4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5`)
- Transaction signatures
- PDAs (derived addresses)

**PRIVATE (NEVER commit):**
- Keypair JSON files (ends with `.json` containing private keys)
- API keys
- Database passwords
- Files ending in `.key`, `.pem`

### Rule 3: Borsh Account Data Layout

**Protocol State Account has Borsh prefix:**
- Bytes 0-7: Discriminator
- Bytes 8-9: **Borsh `0101` prefix** ← Don't forget this!
- Bytes 10-41: Admin pubkey (32 bytes)
- Bytes 42-73: Publisher pubkey (32 bytes)

**WRONG:**
```typescript
const admin = new PublicKey(data.slice(8, 40)); // Missing Borsh prefix
```

**CORRECT:**
```typescript
const admin = new PublicKey(data.slice(10, 42)); // Accounts for 0101
```

### Rule 4: Use Anchor's Instruction Encoder

**WRONG:**
```typescript
const data = Buffer.concat([discriminator, pubkey.toBuffer()]);
```

**CORRECT:**
```typescript
const data = program.coder.instruction.encode('instructionName', { pubkey });
```

### Rule 5: Always Test with --dry-run

```typescript
// Add to all admin scripts
program.option('--dry-run', 'Simulate without executing', false);

if (opts.dryRun) {
  console.log('🔍 DRY RUN - Simulating...');
  const simulation = await connection.simulateTransaction(tx);
  console.log('✅ Simulation result:', simulation);
  return;
}
```

---

## 📝 Protocol Ownership (Oct 30, 2025)

**Current Owner:** `87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy`
- Protocol Admin (signs admin ops)
- Publisher (signs merkle root publications)
- Program Upgrade Authority (signs code upgrades)

**Program:** `4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5` (636KB)
**Protocol State:** `3RhGhHjdzYCCeT9QY1mdBoe8t7XkAaHH225nfQUmH4RX`
**Mint:** `AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5`

**Status:** ✅ Full ownership recovered (emergency recovery Oct 2025)

**Capabilities:**
- `update_admin_open` - Transfer admin to Ledger (ready for post-hackathon)
- `update_publisher_open` - Change publisher key
- `set_paused_open` - Emergency pause
- `set_policy_open` - Update receipt requirements

**Removed:** `emergency_transfer_admin` (backdoor removed after recovery)

---

## 📚 Key Documents

1. `SECURITY_AUDIT_PRE_OPEN_SOURCE.md` - Pre-release security checklist
2. `POST_HACKATHON_LEDGER_MIGRATION.md` - Hardware wallet migration guide
3. `DEPLOYMENT_SUMMARY.md` - Emergency recovery story & lessons
4. `.env.example` - Required environment variables template

---

## 🔑 Key Management

**Current Keypair:** `~/.config/solana/oracle-authority.json`
- Controls: Admin, Publisher, Upgrade Authority
- **NEVER commit to git**
- **NEVER share unencrypted**

**Backup Strategy:**
1. Encrypted in 1Password/Bitwarden (AES-256)
2. USB drive in physical safe
3. Geographic redundancy (second location)

**Post-Hackathon Plan:**
- Migrate admin to Ledger (cold storage)
- Keep publisher as hot wallet (automation)
- Consider multi-sig for upgrade authority

---

## 🐛 Common Mistakes Fixed

1. **Borsh Encoding Bug** - Used manual buffers instead of Anchor coder (Oct 2025)
   - Fix: Always use `program.coder.instruction.encode()`

2. **Wrong Byte Offsets** - Forgot Borsh `0101` prefix in account parsing
   - Fix: Read admin from bytes 10-42, not 8-40

3. **Hardcoded Credentials** - API keys and passwords in code
   - Fix: Use environment variables everywhere

4. **Trusted Simulations** - Assumed simulation = confirmed
   - Fix: Always verify with `solana confirm <signature>`

---

## ⏭️ Next Steps

**Before Open-Sourcing:**
1. Complete `SECURITY_AUDIT_PRE_OPEN_SOURCE.md` checklist
2. Rotate all exposed credentials
3. Test clean clone with env vars
4. Run `gitleaks detect`

**Post-Hackathon:**
1. Execute Ledger migration (guide ready)
2. Separate publisher hot wallet
3. Secure old keypair backups
4. Update team documentation

---

*Last Security Update: October 30, 2025*
*"Don't trust, verify" - Always check on-chain state*
