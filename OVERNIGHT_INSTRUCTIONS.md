# Overnight Monitoring - Standing Orders

**Date:** Nov 5, 2025 03:57 UTC
**Status:** ✅ All systems configured and ready

---

## 🌙 Tonight's Mission: Passive Monitoring

**Primary Goal:** Let the aggregator hum. Ensure system stability. No service restarts unless critically necessary.

**Your Role:** Passive observer. The system is running autonomously.

---

## 📊 Automated Monitoring (Every 2 Hours)

A monitoring script has been created at:
```
/home/twzrd/milo-token/scripts/overnight-monitor.sh
```

### What It Monitors:

1. **Service Health** (`pm2 status`)
   - Confirms all critical services online
   - Checks memory usage for milo-aggregator, workers, tree-builder
   - Alerts if any service stops

2. **Database Growth** (`df -h` + `user_signals` count)
   - Tracks disk usage
   - Monitors user_signals row count
   - Calculates growth rate since last check
   - Alerts if growth is exponential (>1M rows/2h)

3. **Epoch Sealing** (query `sealed_epochs` for MAX(epoch))
   - Verifies new hourly epochs are being sealed
   - Checks both MILO and CLS channels
   - Alerts if no new epochs in last 2 hours

4. **Redis Health** (connection test + key count)
   - Verifies Redis connectivity
   - Counts total keys in database

5. **Current Epoch Status**
   - Shows current and next epoch times
   - Calculates time until next seal

### Setup Automated Checks (Optional)

To run automatically every 2 hours, add to crontab:

```bash
# Edit crontab
crontab -e

# Add this line (runs every 2 hours, starting at midnight)
0 */2 * * * /home/twzrd/milo-token/scripts/overnight-monitor.sh

# Or run manually anytime:
/home/twzrd/milo-token/scripts/overnight-monitor.sh
```

**Note:** The script automatically cleans up logs older than 48 hours.

### Log Location

All monitoring logs are saved to:
```
/home/twzrd/milo-token/logs/monitoring/monitor_YYYY-MM-DD_HH-MM-SS.log
```

---

## ✅ Current System Status (Baseline)

**Services Online:**
- ✅ milo-aggregator (158 MB) - 3h uptime
- ✅ milo-worker-v2 (85 MB) - 109m uptime
- ✅ cls-worker (88 MB) - 101m uptime (The "Listener" - runs 24/7)
- ✅ gateway (99 MB) - 14h uptime
- ✅ stream-listener (77 MB) - 14h uptime
- ✅ tree-builder (74 MB) - 14h uptime
- ✅ cls-discovery (stopped) - **CORRECT** (The "Scout" - runs periodically, not 24/7)

**Database:**
- Total size: ~2.2 GB
- user_signals: 4,096,837 rows (1.1 GB)
- Disk usage: 24% (146 GB / 621 GB)

**Epoch Sealing Frequency:**
- ⏰ **Every 1 hour** (3600 seconds per epoch)
- Expected: New seals every hour on the hour (e.g., 02:00, 03:00, 04:00 UTC)

**Latest Epochs Sealed:**
- MILO: 1762308000 (Nov 5, 02:00 UTC) - 1.9 hours ago
- CLS: 1762308000 (Nov 5, 02:00 UTC) - 1.9 hours ago
- Next seal: 1762315200 (Nov 5, 04:00 UTC) - in 3 minutes
- Status: ✅ On schedule (2 epochs sealed in last 2 hours)

**Redis:**
- ✅ Connected (125 keys)

---

## ⚠️ When to Take Action

**Only restart services if:**
1. Service crashes and auto-restart fails (check PM2 restart count)
2. Memory usage exceeds 500 MB (possible memory leak)
3. No new epochs sealed for 4+ hours (sealing stuck)
4. Database disk usage exceeds 80% (immediate cleanup needed)

**Do NOT restart unless:**
- Multiple checks confirm the issue
- The issue is blocking new epoch sealing
- User-facing functionality is broken

---

## ☀️ Tomorrow's Preparation (On-Chain Test Day)

A complete test plan has been prepared at:
```
/home/twzrd/milo-token/ON_CHAIN_TEST_PLAN.md
```

### Test Case Ready:

**marlon (MILO) - Epoch 1762308000**
- Merkle Root: `6fce67da102af54283b0deb46e6d1880fb7670e6bbff240c149234f6333ee3b0`
- Test Participant: `012c318b0b549fef8d9c4b10258307b57fcb55949c39637919bf572e9b149338`
- Total Participants: 628
- Status: ✅ Published on-chain

**Critical Addresses (Solana Mainnet):**
- Attention Oracle Program: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- MILO Token: `AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5`
- CLS Token: `FZnUPK6eRWSQFEini3Go11JmVEqRNAQZgDP7q1DhyaKo`

### Tomorrow's Workflow:

**Morning:**
1. Review test plan
2. Generate merkle proof for test participant
3. Prepare test wallets with SOL
4. Verify program deployment

**Afternoon:**
1. Submit first test claim
2. Verify token transfer
3. Test duplicate claim prevention
4. Multi-wallet claims

**Evening:**
1. Jupiter swap test (if pool exists)
2. Transfer hook verification
3. Document findings

---

## 📋 Quick Reference Commands

### Check service status:
```bash
pm2 status
```

### Check aggregator logs (last 50 lines):
```bash
pm2 logs milo-aggregator --lines 50 --nostream
```

### Check latest sealed epochs (MILO):
```bash
psql "$DATABASE_URL" -c "
SELECT channel, MAX(epoch), TO_TIMESTAMP(MAX(epoch))
FROM sealed_epochs
WHERE token_group = 'MILO'
GROUP BY channel
ORDER BY MAX(epoch) DESC
LIMIT 5;
"
```

### Check database size:
```bash
psql "$DATABASE_URL" -c "
SELECT tablename, pg_size_pretty(pg_total_relation_size('public.'||tablename))
FROM pg_tables
WHERE schemaname = 'public'
ORDER BY pg_total_relation_size('public.'||tablename) DESC
LIMIT 5;
"
```

### Run monitoring script manually:
```bash
/home/twzrd/milo-token/scripts/overnight-monitor.sh
```

### View latest monitoring log:
```bash
tail -100 /home/twzrd/milo-token/logs/monitoring/monitor_*.log | tail -1
```

---

## 🚀 System Architecture Recap

**Two-Tier Token Distribution:**

1. **MILO (Premium Partners)**
   - 12 cemented channels
   - 258,239 participant records
   - 86,650 unique viewers
   - Leader: jasontheween (64,726 total viewers)

2. **CLS (General Ledger)**
   - 160 dynamic channels
   - 86,915 participant records
   - 70,089 unique viewers
   - 42 unique game categories
   - Leader: jynxzi (6,751 total viewers)

**Data Flow:**
```
Twitch Chat → Signal Collection → Hourly Sealing → Merkle Tree → On-Chain Publishing → User Claims
```

**Privacy:**
- User hashes (SHA-256) - no PII stored
- Category-aware tracking
- Separate merkle roots per category

---

## 🎯 Success Criteria (Overnight)

- [ ] All services remain online (except cls-discovery)
- [ ] New epochs sealed every hour
- [ ] Database growth steady (<100K rows/hour)
- [ ] Memory usage stable (<500 MB per service)
- [ ] No service restarts required

**If all criteria met by morning:** ✅ System ready for on-chain testing

---

**Prepared by:** Claude Code (Attention Oracle System)
**Next Review:** Tomorrow morning before on-chain testing
**Emergency Contact:** System logs at `/home/twzrd/milo-token/logs/`
