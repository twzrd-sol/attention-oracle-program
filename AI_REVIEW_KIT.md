# AI Review Kit: Critical Files & Diagnostics Export

**Purpose**: Provide an AI with comprehensive context to review system architecture, identify scaling bottlenecks, validate expansion strategy, and spot security/data integrity issues.

**Export Date**: 2025-11-12
**System Status**: Launch-ready (9,318 users staged, IRC collector live)
**Goal**: Scale to 100K+ eligible users via backfill + live mapping

---

## 📋 File Manifest (Priority Order)

### TIER 1: ARCHITECTURE & OVERVIEW
**What to provide first** - Gives AI the big picture

```
1. SYSTEM_ARCHITECTURE.md              (This repo root)
2. ARCHITECTURE_DIAGRAM.txt            (This repo root)
3. .env (REDACTED)                     (See export below)
4. git log --oneline -20               (Recent changes)
```

### TIER 2: DATABASE SCHEMA & STATE
**Current data structure & volumes**

```
1. Database schema dump (key tables)
2. Row count inventory (all tables)
3. Index definitions (performance-critical)
4. Suppression list contents
5. user_mapping sample (first 100 rows)
6. Allocation distribution (histogram)
```

### TIER 3: SOURCE CODE (Key Modules)
**What AI needs to understand logic**

```
1. clean-hackathon/api/server.ts       (Claim API entry point)
2. clean-hackathon/api/claim.ts        (Claim transaction logic)
3. clean-hackathon/api/eligibility.ts  (Eligibility check logic)
4. apps/twzrd-aggregator/src/db-pg.ts  (DB abstraction)
5. apps/twzrd-aggregator/src/index.ts  (Aggregator entry point)
6. scripts/twitch-irc-collector.ts      (Live data ingestion)
```

### TIER 4: CONFIGURATION & DEPLOYMENT
**Infrastructure & service config**

```
1. PM2 ecosystem config (if exists)
2. Solana program IDL / ABI
3. RPC endpoint configuration
4. Environment variable reference
```

### TIER 5: LOGS & DIAGNOSTICS
**Real-time system behavior**

```
1. PM2 list output
2. Last 100 lines of cls-aggregator logs
3. Last 100 lines of API server logs
4. IRC collector logs (last 1 min)
5. Database connection health check
6. Recent SQL queries (slow query log if available)
```

### TIER 6: DATA & QUERIES
**The expansion/backfill strategy**

```
1. Current eligible user distribution
2. Suppressed users list
3. Proposed expansion SQL
4. Signal-weighted allocation formula
5. On-chain token supply state
```

---

## 🔧 Export Commands (Run These)

### EXPORT 1: System Architecture (Already Done ✓)
```bash
# Files already created:
cat SYSTEM_ARCHITECTURE.md
cat ARCHITECTURE_DIAGRAM.txt
```

### EXPORT 2: Environment Config (REDACTED)
```bash
# Create redacted .env for sharing
cp .env .env.review
sed -i 's/AVNS_[^@]*/[REDACTED_DO_CREDENTIALS]/g' .env.review
sed -i 's/OAuth_[^&]*/[REDACTED_OAUTH]/g' .env.review
sed -i 's/twzrd_password_[^@]*/[REDACTED_PASSWORD]/g' .env.review
cat .env.review
```

### EXPORT 3: Database Schema & State
```bash
# Save database inventory
psql -U twzrd -h /var/run/postgresql -d twzrd << 'SQL' > db-inventory.txt
-- Table structure & row counts
SELECT
  tablename,
  (SELECT COUNT(*) FROM information_schema.columns WHERE table_name = tablename) as columns,
  pg_size_pretty(pg_total_relation_size(tablename::regclass)) as size
FROM pg_tables
WHERE schemaname = 'public'
ORDER BY pg_total_relation_size(tablename::regclass) DESC;

-- Row count summary
SELECT 'sealed_participants' as table_name, COUNT(*) as rows FROM sealed_participants
UNION ALL SELECT 'user_mapping', COUNT(*) FROM user_mapping
UNION ALL SELECT 'claimable_allocations', COUNT(*) FROM claimable_allocations
UNION ALL SELECT 'user_signals', COUNT(*) FROM user_signals
UNION ALL SELECT 'channel_participation', COUNT(*) FROM channel_participation
UNION ALL SELECT 'suppression_list', COUNT(*) FROM suppression_list;

-- Index definitions
SELECT indexname, indexdef FROM pg_indexes WHERE schemaname = 'public' LIMIT 20;

-- Current allocation distribution
SELECT
  'min', MIN(amount)
UNION ALL SELECT 'max', MAX(amount)
UNION ALL SELECT 'avg', AVG(amount)::INT
UNION ALL SELECT 'p50', percentile_cont(0.5) WITHIN GROUP (ORDER BY amount)::INT
FROM claimable_allocations;
SQL

cat db-inventory.txt
```

### EXPORT 4: Key Source Files
```bash
# Create source code bundle
mkdir -p ai-review/source
cp clean-hackathon/api/server.ts ai-review/source/
cp clean-hackathon/api/claim.ts ai-review/source/
cp clean-hackathon/api/eligibility.ts ai-review/source/
cp apps/twzrd-aggregator/src/db-pg.ts ai-review/source/
cp apps/twzrd-aggregator/src/index.ts ai-review/source/
cp clean-hackathon/scripts/twitch-irc-collector.ts ai-review/source/

# Create README for source code
cat > ai-review/source/README.md << 'EOF'
# Source Code Files for AI Review

## Entry Points
- **server.ts**: Main claim API (Express.js)
  - /api/claim: POST endpoint (mint tokens)
  - /api/eligibility: GET endpoint (check user allocation)

- **claim.ts**: Transaction builder for Solana
- **eligibility.ts**: Eligibility check logic
- **db-pg.ts**: PostgreSQL connection & queries
- **index.ts**: Aggregator main loop
- **twitch-irc-collector.ts**: Live data ingestion from IRC

## Key Things AI Should Review
1. Database query patterns (N+1 issues?)
2. Error handling & retry logic
3. Solana transaction signing (key management)
4. Rate limiting on claim endpoint
5. Suppression list enforcement
6. Allocation calculation correctness
7. IRC parsing for username extraction
EOF

cat ai-review/source/README.md
```

### EXPORT 5: Current System State (PM2 & Logs)
```bash
# Save PM2 status
pm2 list > ai-review/pm2-status.txt
pm2 logs cls-aggregator --nostream | tail -100 > ai-review/aggregator-logs.txt
tail -100 /tmp/api-server.log > ai-review/api-logs.txt
tail -100 /tmp/irc-collector.log > ai-review/irc-logs.txt

# Database health
psql -U twzrd -h /var/run/postgresql -d twzrd << 'SQL' > ai-review/db-health.txt
-- Connection pool status
SELECT datname, count(*) FROM pg_stat_activity GROUP BY datname;

-- Table bloat (dead tuples)
SELECT schemaname, tablename, n_dead_tup, n_live_tup, ROUND(n_dead_tup::numeric / (n_live_tup + n_dead_tup) * 100, 2) as bloat_pct
FROM pg_stat_user_tables
WHERE n_live_tup > 0
ORDER BY bloat_pct DESC
LIMIT 10;

-- Slow queries (if pg_stat_statements enabled)
SELECT query, calls, mean_time FROM pg_stat_statements ORDER BY mean_time DESC LIMIT 10;
SQL

cat ai-review/db-health.txt
```

### EXPORT 6: Expansion Strategy & Data
```bash
# Current allocation distribution
psql -U twzrd -h /var/run/postgresql -d twzrd << 'SQL' > ai-review/expansion-baseline.txt
-- Baseline: Current eligible users
SELECT
  'Total Unique Users' as metric,
  COUNT(DISTINCT username) as count
FROM claimable_allocations
UNION ALL
SELECT 'Total Allocations', COUNT(*) FROM claimable_allocations
UNION ALL
SELECT 'Total CHAT Staged', SUM(amount)::BIGINT FROM claimable_allocations
UNION ALL
SELECT 'Mapped Users (sealed_participants)', COUNT(DISTINCT username) FROM sealed_participants WHERE username IS NOT NULL
UNION ALL
SELECT 'Unmapped Hashes (sealed_participants)', COUNT(DISTINCT user_hash) FROM sealed_participants WHERE username IS NULL
UNION ALL
SELECT 'Suppressd Users', COUNT(*) FROM suppression_list;

-- Sample eligible users
SELECT username, SUM(amount) as total_chat, COUNT(*) as allocations
FROM claimable_allocations
GROUP BY username
ORDER BY total_chat DESC
LIMIT 20;
SQL

cat ai-review/expansion-baseline.txt

# Proposed expansion query
cat > ai-review/expansion-query.sql << 'EOF'
-- Proposed: Expand claimable_allocations to ALL mapped users
-- Strategy: Base 100 CHAT + signal-weighted bonus (capped at 1000)

INSERT INTO claimable_allocations (epoch, channel, username, amount)
SELECT
  MAX(sp.epoch) as epoch,
  'allocation_base' as channel,
  sp.username,
  LEAST(
    100 + (COUNT(*) * 10),  -- Base 100 + 10 per participation
    1000  -- Cap at 1000 to manage supply
  ) as amount
FROM sealed_participants sp
WHERE sp.username IS NOT NULL
  AND sp.username NOT IN (SELECT username FROM suppression_list)
  AND NOT EXISTS (
    SELECT 1 FROM claimable_allocations ca
    WHERE ca.username = sp.username
  )
GROUP BY sp.username
ON CONFLICT (epoch, channel, username) DO NOTHING;

-- Expected: ~150K new rows, ~17M additional CHAT tokens
-- Verify post-run:
-- SELECT COUNT(DISTINCT username), SUM(amount) FROM claimable_allocations;
EOF

cat ai-review/expansion-query.sql
```

### EXPORT 7: On-Chain Data
```bash
# Program and mint info
cat > ai-review/onchain-config.txt << 'EOF'
# Solana Mainnet Configuration

## CLS Token Program
Program ID: GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
Flavor: MILO_OPEN

## CHAT Token Mint
Mint Address: AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5
Decimals: (check on-chain)
Supply: (check current)
Authority: (check current)

## RPC Endpoints
Primary: https://solana-mainnet.api.Helius.io/
Fallback: https://mainnet.helius-rpc.com/

## Recent Claims (check Solscan)
- Example tx: (run recent claim to populate)
- Verify mint authority signature
- Verify token account creation

## Keys & Authorities
- Mint Authority: (stored in /home/twzrd/.keys/)
- Freeze Authority: (check if set)
- Owner: (check current)
EOF

cat ai-review/onchain-config.txt
```

---

## 📦 Bundle Everything

```bash
# Create a tarball for easy sharing
cd /home/twzrd/milo-token
tar -czf ai-review-kit-2025-11-12.tar.gz \
  SYSTEM_ARCHITECTURE.md \
  ARCHITECTURE_DIAGRAM.txt \
  .env.review \
  db-inventory.txt \
  ai-review/

# Size check
du -sh ai-review-kit-*.tar.gz

# Or create a summary index
cat > AI_REVIEW_INDEX.md << 'EOF'
# AI Review Kit Index

## Quick Reference

### What's in the Kit?
1. **Architecture**: System design & data flow
2. **Schema**: Database structure & current volumes
3. **Source Code**: Key modules (API, aggregator, IRC collector)
4. **Logs**: Real-time system behavior
5. **Baseline**: Current user counts & allocation distribution
6. **Expansion**: Proposed backfill strategy & SQL
7. **On-Chain**: Program ID, mint address, RPC config

### Key Questions for AI to Answer

1. **Scaling Bottlenecks**:
   - Can we allocate 150K users without DB performance drop?
   - Any index suggestions?

2. **Data Integrity**:
   - Are we handling suppression list correctly?
   - Any duplicates in user_mapping?
   - Orphaned allocations (user deleted but allocation remains)?

3. **Security**:
   - Is claim endpoint rate-limited?
   - Any SQL injection risks in eligibility query?
   - Mint authority key storage secure?

4. **Expansion Strategy**:
   - Is signal-weighted formula optimal (100 + 10x, cap 1K)?
   - Should we include historical window cutoff?
   - Any supply implications (total CHAT impact)?

5. **Live Mapping**:
   - Will IRC collector keep up with claim surge?
   - Should we cache username→hash mappings?
   - Any collision risks with hash function?

6. **Post-Launch**:
   - Database unification priorities?
   - Leaderboard query optimization?
   - Replication strategy (DO to local)?

### Files to Review First
1. SYSTEM_ARCHITECTURE.md (2 min read)
2. db-inventory.txt (current state)
3. source/server.ts (claim logic)
4. expansion-query.sql (scaling plan)
5. expansion-baseline.txt (before/after metrics)

### How to Run AI Review
```bash
# Unpack kit
tar -xzf ai-review-kit-2025-11-12.tar.gz

# Share with Claude or your AI:
# "Review this kit. Answer the 6 key questions above.
#  Priority: Spot blockers for 100K-user launch.
#  Then suggest optimizations for 1M users."

# Expected output:
# - 3-5 critical issues (if any)
# - 10-15 optimization suggestions
# - Confidence score for 100K launch
```
EOF

cat AI_REVIEW_INDEX.md
```

---

## 🎯 What an AI Should Focus On

### Critical Review Areas

**1. Database Performance**
- [ ] Row counts & query latency on claimable_allocations
- [ ] Index strategy for expansion (174K → 150K new rows)
- [ ] Vacuum/ANALYZE recommendations
- [ ] Dead tuple bloat in high-write tables (user_signals, channel_participation)

**2. Expansion Logic**
- [ ] Signal-weighted formula correctness (100 + 10x, cap 1K)
- [ ] Suppression list enforcement (will excluded users bypass?)
- [ ] Deduplication logic (ON CONFLICT handling)
- [ ] Supply impact (17M new tokens—safe?)

**3. Claim Flow Security**
- [ ] Rate limiting on /api/claim endpoint
- [ ] Solana transaction signing (key rotation, HSM?)
- [ ] Phantom wallet integration (token account creation)
- [ ] Double-claim prevention (redeemed_at tracking)

**4. Live Data Mapping**
- [ ] IRC username extraction correctness (edge cases: special chars?)
- [ ] Hash collision risks (keccak256(lowercase(username)))
- [ ] Throughput: Can 1.6K events/min sustain 100K claims/day?
- [ ] Fallback if IRC collector crashes

**5. Post-Launch Scaling**
- [ ] Database replication strategy (DO → local)
- [ ] Leaderboard query optimization (top 100 claimers)
- [ ] User retention metrics (repeat claims, churn)
- [ ] Governance: Who can modify suppression list?

---

## 📊 Export Summary

**Total files**: ~15
**Total size**: ~50-100 MB (compressed: ~5-10 MB)
**Time to review**: 30-60 min for thorough AI analysis
**Expected output**: 20-30 recommendations + risk score

---

## Next: Share & Review

```bash
# Option 1: Upload to S3 or paste to Claude
aws s3 cp ai-review-kit-2025-11-12.tar.gz s3://your-bucket/

# Option 2: Print to markdown for direct paste
cat ai-review/* | pbcopy  # macOS
cat ai-review/* | xclip   # Linux

# Option 3: GitHub issue (invite reviewer)
gh issue create --title "AI System Review Kit" \
  --body "$(cat AI_REVIEW_INDEX.md)" \
  --label "review,scaling"
```

---

**Generated**: 2025-11-12
**System**: Launch-ready (9,318 users staged)
**Next Step**: Get AI feedback → Fix critical issues → Ship to 100K+ ✓
