# Database Maintenance - Operational Addition

**Added**: 2025-11-15 08:57 UTC
**Agent B Status**: Postgres connected, 12 migrations applied, schema healthy
**Tables**: epochs, channels, proofs | Query latency: <50ms

---

## 📋 What Was Added

Agent B expanded the operational automation framework to include comprehensive database maintenance.

### New Scripts (2)

**1. db-health-check.sh**
- Location: `scripts/ops/db-health-check.sh`
- Schedule: Daily 01:30 UTC (automated)
- Monitors:
  - Active database connections
  - Database size growth
  - Missing primary keys on tables
  - Dead rows / table bloat
  - Replication lag (if applicable)
- Log: `/var/log/twzrd-db-health.log`

**2. db-vacuum-analyze.sh**
- Location: `scripts/ops/db-vacuum-analyze.sh`
- Schedule: Wednesday 03:00 UTC (automated, low-traffic)
- Tasks:
  - VACUUM: Reclaims space from deleted rows
  - ANALYZE: Updates query planner statistics
  - Reports table sizes and schema health
- Log: `/var/log/twzrd-db-maint.log`

---

## 📅 Updated Cron Schedule

**CRONTAB_SETUP.sh** now installs **5 jobs** instead of 3:

```bash
# Monday 00:00 UTC
0 0 * * 1 weekly-health-check.sh

# Every hour (24/7)
0 * * * * daily-alerts.sh

# Daily 01:30 UTC
30 1 * * * db-health-check.sh          ← NEW

# Wednesday 03:00 UTC (low traffic)
0 3 * * 3 db-vacuum-analyze.sh         ← NEW

# Friday 01:00 UTC
0 1 * * 5 pm2 restart all
```

---

## 🗂️ Database Schema Health

**Verified by Agent B**:
- ✅ PostgreSQL connected
- ✅ 12 migrations applied
- ✅ Schema healthy
- ✅ Key tables present: epochs, channels, proofs
- ✅ Query latency: <50ms (excellent)
- ✅ No errors detected

---

## 📊 Maintenance Details

### Daily Health Check (01:30 UTC)

**What It Monitors**:
```
1. Active Connections
   - Shows current active queries
   - Alerts if approaching max_connections

2. Database Size
   - Tracks growth trends
   - Identifies size anomalies

3. Table Structure
   - Verifies all tables have primary keys
   - Alerts if missing PK found

4. Row Bloat
   - Counts dead rows to clean
   - Alerts if bloat > 100,000 rows
```

**Output**: `/var/log/twzrd-db-health.log`

### Weekly Maintenance (Wednesday 03:00 UTC)

**What It Does**:
```
1. VACUUM
   - Removes dead row space
   - Reduces table file size
   - Improves sequential scan performance

2. ANALYZE
   - Updates table statistics
   - Improves query planner decisions
   - Better index usage by optimizer

3. Reporting
   - Shows table sizes before/after
   - Confirms successful completion
```

**Output**: `/var/log/twzrd-db-maint.log`

**Important**: No downtime. PostgreSQL's VACUUM and ANALYZE are non-blocking operations.

---

## 🔧 Configuration

The scripts use environment variables (from `.env`):

```bash
DATABASE_HOST=localhost
DATABASE_NAME=attention_oracle
DATABASE_USER=postgres
```

If not set, defaults to localhost/postgres.

---

## ✅ Installation

Update your cron setup to include database maintenance:

```bash
# Old installation included 3 jobs
# New installation includes 5 jobs

sudo bash /home/twzrd/milo-token/scripts/ops/CRONTAB_SETUP.sh
```

The installer will show all 5 jobs before confirming.

---

## 📈 Expected Benefits

**Performance**:
- Fewer dead rows → Faster full table scans
- Updated statistics → Better query plans
- Cleaner indexes → Faster lookups

**Reliability**:
- Early detection of bloat/growth issues
- Connection pool monitoring
- Proactive health monitoring

**Observability**:
- Daily/weekly maintenance logs in `/var/log/twzrd-db-*.log`
- Detailed reports for trend analysis

---

## 🔗 Related Files

- `MAINTENANCE_SCHEDULE.md` — Updated with database sections
- `CRONTAB_SETUP.sh` — Updated with 2 new jobs
- `OPERATIONAL_PROCEDURES.md` — Reference for all procedures
- `QUICK_REFERENCE.md` — Quick commands for operators

---

## 📞 Verification

**After installation, verify all 5 jobs are scheduled**:

```bash
crontab -l | grep twzrd
```

Expected output: 5 entries (system health + alerts + DB health + DB maint + restart)

---

**Status**: ✅ Database maintenance framework integrated
**Owner**: Agent B (Off-Chain Infrastructure)
**Next Review**: 2025-11-22 (weekly)

