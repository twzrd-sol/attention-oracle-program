# Off-Chain Monitoring Guide

**Status:** ✅ Production Ready
**Last Updated:** November 15, 2025
**Monitoring Interval:** Every 30 minutes
**Alert Escalation:** Immediate for CRITICAL, 1 hour for WARNING

---

## Quick Start

### 1. Start the Monitor

```bash
cd /home/twzrd/milo-token
./start-monitor.sh
```

This will:
- ✅ Load your `.env` file (DATABASE_URL)
- ✅ Start the monitoring script in a `screen` or `tmux` session
- ✅ Run health checks every 30 minutes
- ✅ Log all reports to `~/.pm2/monitor.log`

### 2. Attach to Monitor

If using **screen**:
```bash
screen -r milo-monitor
```

If using **tmux**:
```bash
tmux attach -t milo-monitor
```

### 3. Detach Without Stopping

- **Screen:** Press `Ctrl+A`, then `D`
- **Tmux:** Press `Ctrl+B`, then `D`

### 4. View Logs

```bash
# Last 50 lines
tail -50 ~/.pm2/monitor.log

# Follow logs in real-time
tail -f ~/.pm2/monitor.log

# Search for alerts
grep "CRITICAL\|WARNING" ~/.pm2/monitor.log
```

---

## What Gets Monitored

### 1. Process Health (PM2)

Checks every process status:
- **twzrd-aggregator** — Collects Twitch events
- **tree-builder-worker** — Seals epochs hourly
- **cls-worker-1** — Processes livestream data

**Monitored Metrics:**
- Status: `online` | `offline` | `errored` | `stopped`
- Uptime
- Restart count
- Memory usage
- CPU usage

**Alerts:**
- ❌ **CRITICAL:** All workers offline
- ⚠️ **WARNING:** Any single worker offline
- ⚠️ **WARNING:** >20 restarts/hour

---

### 2. Database Connectivity

Verifies PostgreSQL connection and data flow.

**Monitored Metrics:**
- Connection status
- Events in last hour
- Latest sealed epoch
- Time since last epoch seal

**Alerts:**
- ❌ **CRITICAL:** Database unreachable
- ⚠️ **WARNING:** <100 events/hour (no data flowing)
- ⚠️ **WARNING:** Latest epoch >2 hours old (sealing stalled)
- ❌ **CRITICAL:** No sealed epochs in 4+ hours

---

### 3. Epoch Sealing Cadence

Checks that epochs are being sealed regularly.

**Expected Behavior:**
- New sealed epoch every ~1 hour
- All epochs have participants >0
- Recent epochs marked as `published: true`

**Alerts:**
- ⚠️ **WARNING:** Zero participants in an epoch (sealing broken)
- ⚠️ **WARNING:** Epochs not marked as published

---

### 4. Data Anomalies

Detects sudden drops in participant counts.

**Alerts:**
- ⚠️ **WARNING:** Participant count drops >50% from baseline
- ⚠️ **WARNING:** Zero participants in recent epochs

---

### 5. Resource Usage

Tracks system and process resource consumption.

**Thresholds:**
- ⚠️ **WARNING:** Process memory >3GB
- ❌ **CRITICAL:** Process memory >4GB
- ❌ **CRITICAL:** CPU >90% sustained
- ❌ **CRITICAL:** System memory <200MB free

---

## Alert Severity Levels

### 🔴 CRITICAL (Escalate Immediately)

Take action within **5 minutes**:

```
❌ All workers offline
❌ Database unreachable
❌ No sealed epochs in 4+ hours
❌ Zero participants in all recent epochs
❌ System memory <200MB
❌ Process memory >4GB
❌ CPU >90% sustained
```

**Action:**
1. Review logs immediately
2. Attempt to restart affected services (see Common Issues)
3. If issue persists, escalate (see Escalation section below)

---

### 🟠 WARNING (Investigate Within 1 Hour)

```
⚠️ One worker repeatedly crashing (>20 restarts/hour)
⚠️ Participant count drop >50%
⚠️ No new data in last hour
⚠️ Process memory >3GB
⚠️ Epoch sealing stalled (latest >2 hours old)
```

**Action:**
1. Note the timestamp and affected component
2. Check logs for errors
3. Plan a fix (may not require immediate action)
4. Report in next status update

---

### 🔵 INFO (Log Only)

```
ℹ️ Occasional restarts (<5/hour)
ℹ️ Participant variance <20%
ℹ️ Warnings in logs (not errors)
```

**Action:** None needed — normal operation

---

## Common Issues & Fixes

### Issue 1: Process Crashed (Status: offline/errored)

**Diagnosis:**
```bash
# Check recent logs
pm2 logs twzrd-aggregator --lines 100 --nostream | tail -50

# Look for error messages before crash
```

**Common Causes:**
1. Database connection pool exhausted
2. Uncaught exception in event handler
3. Out of memory (OOM)
4. Network timeout from Twitch API

**Fix:**
```bash
# Restart the process
pm2 restart twzrd-aggregator

# Monitor for 5 minutes
watch -n 10 "pm2 list | grep aggregator"

# If it crashes again in <5 minutes, escalate
```

---

### Issue 2: No New Sealed Epochs (Latest >2 hours old)

**Diagnosis:**
```bash
# Check tree builder logs
pm2 logs tree-builder-worker --lines 100 --nostream

# Look for:
# - "Sealing epoch..." (should happen hourly)
# - Error messages
# - "No participants to seal" (unusual)
```

**Common Causes:**
1. Tree builder worker crashed
2. Database write permission issue
3. No data flowing from CLS workers

**Fix:**
```bash
# Restart tree builder
pm2 restart tree-builder-worker

# Monitor logs
pm2 logs tree-builder-worker --nostream

# Check if sealing resumes within 5 minutes
```

---

### Issue 3: Database Connection Errors

**Symptoms:**
```
Error: ECONNREFUSED
Error: Connection terminated unexpectedly
Error: SASL authentication failed
```

**Diagnosis:**
```bash
# Verify DATABASE_URL is set
echo $DATABASE_URL

# Test connection directly (if psql installed)
psql $DATABASE_URL -c "SELECT 1"
```

**Fix:**
```bash
# Reload environment
source .env

# Verify the URL is correct
# Check if the database server is reachable

# Restart all workers
pm2 restart all
```

---

### Issue 4: High Restart Count (>20/hour)

**Diagnosis:**
```bash
# Check error pattern
pm2 logs <process-name> --lines 500 --nostream | grep "Error"

# Look for repeating error messages
```

**Common Causes:**
1. Wrong environment variable
2. Rate limiting from Twitch API (backoff required)
3. Bug in recent code deployment

**Fix:**
```bash
# Check recent code changes
git log --oneline -5

# If recent change, consider rollback
git revert HEAD

# Or, increase rate limit backoff in config
# Then restart:
pm2 restart <process-name>
```

---

### Issue 5: Memory Leak (Continuously Growing Memory)

**Symptoms:**
- Memory usage increases every hour
- Eventually hits 4GB and crashes

**Diagnosis:**
```bash
# Monitor memory over time
pm2 describe twzrd-aggregator | grep memory

# Check logs for unbounded data structures
pm2 logs twzrd-aggregator --lines 500 --nostream | grep -i "accumulate\|cache\|buffer"
```

**Fix:**
1. Look for event listeners not being cleaned up
2. Check for circular references in data
3. Consider implementing periodic cleanup
4. Restart process as temporary fix

---

## Escalation Procedure

### When to Escalate

Escalate **immediately** if:
- ❌ CRITICAL alert triggered AND fix attempt failed
- 🔄 Issue persists for >30 minutes after fix attempt
- 📊 Data loss detected (epochs deleted/corrupted)
- 🔐 Security incident (unauthorized access, API key leaked)
- ❓ Unknown error you cannot diagnose

### How to Escalate

**Format:**
```
ESCALATION: Off-Chain Monitoring

Severity: [CRITICAL/WARNING]
Component: [aggregator/worker/database/sealing]
Issue: [Brief description]
Duration: [When did the issue start?]
Attempted Fixes: [What you tried and result]

Recent Logs:
[Paste relevant log excerpts, 10-20 lines]

Impact:
- Is data collection stopped?
- Are new epochs being sealed?
- Are claims affected?
- How many users impacted?

Next Steps:
[What would you try next?]
```

**Where to Send:**
- Post in internal team channel
- Email: ops-team@twzrd.com
- Page on-call engineer if CRITICAL

---

## Monitoring Reports

The monitor generates automatic reports every 30 minutes. Each report includes:

1. **Process Status** — All worker statuses, uptime, restarts
2. **Database Health** — Connection status, event flow rate, latest epoch
3. **Recent Epochs** — Last 10 sealed epochs with participant counts
4. **Active Alerts** — Any conditions requiring attention
5. **System Resources** — Memory, CPU, disk space

### Sample Report

```
════════════════════════════════════════════════════════════════════════════════
OFF-CHAIN HEALTH REPORT
Timestamp: 2025-11-15T04:50:30.123Z
════════════════════════════════════════════════════════════════════════════════

📊 PROCESS STATUS
────────────────────────────────────────────────────────────────────────────────
✅ twzrd-aggregator
   Status: online | Uptime: 2d 5h | Restarts: 3
   Memory: 1024MB | CPU: 5%

✅ tree-builder-worker
   Status: online | Uptime: 1d 12h | Restarts: 1
   Memory: 512MB | CPU: 2%

✅ cls-worker-1
   Status: online | Uptime: 12h 30m | Restarts: 2
   Memory: 768MB | CPU: 8%

🗄️  DATABASE HEALTH
────────────────────────────────────────────────────────────────────────────────
✅ Connected
Events (last hour): 4523
Latest epoch: 1762370000 (1250 seconds old)

📈 RECENT EPOCHS
────────────────────────────────────────────────────────────────────────────────
Epoch    Channel         Participants  Sealed              Published
────────────────────────────────────────────────────────────────────────────────
1762370000 jasontheween   3210          2025-11-15 04:45:30 ✅
1762366400 train          2156          2025-11-15 03:45:28 ✅
1762362800 sykkuno        1845          2025-11-15 02:45:27 ✅
...

✅ No alerts

💾 SYSTEM RESOURCES
────────────────────────────────────────────────────────────────────────────────
Memory: 8.2G used | 7.8G free
CPU Load (1/5/15 min): 0.45, 0.38, 0.32
════════════════════════════════════════════════════════════════════════════════
```

---

## Performance Baselines

Use these as targets for normal operation:

| Metric | Baseline | Warning | Critical |
|--------|----------|---------|----------|
| Process Uptime | >1 day | <6 hours | <1 hour |
| Restarts/hour | <2 | >5 | >20 |
| Process Memory | <1GB | >3GB | >4GB |
| Process CPU | <10% | >50% | >90% |
| Events/hour | >1000 | <100 | 0 |
| Epoch Staleness | <10 min | >2 hours | >4 hours |
| Participants/epoch | 1000-5000 | ±50% drop | Zero |
| DB Response Time | <100ms | >500ms | Timeout |

---

## Manual Health Check

If the automated monitor isn't running, run a quick manual check:

```bash
#!/bin/bash
# Quick health check

echo "=== PM2 Status ==="
pm2 list

echo -e "\n=== Database Events (Last Hour) ==="
source .env
npx tsx -e "
const { Pool } = require('pg');
const pool = new Pool({ connectionString: process.env.DATABASE_URL, ssl: { rejectUnauthorized: false } });
(async () => {
  const res = await pool.query('SELECT COUNT(*) as count FROM channel_participation WHERE created_at > NOW() - INTERVAL \\'1 hour\\'');
  console.log('Events:', res.rows[0].count);
  await pool.end();
})();
"

echo -e "\n=== Latest Epochs ==="
npx tsx -e "
const { Pool } = require('pg');
const pool = new Pool({ connectionString: process.env.DATABASE_URL, ssl: { rejectUnauthorized: false } });
(async () => {
  const res = await pool.query('SELECT epoch, channel, sealed_at FROM sealed_epochs ORDER BY epoch DESC LIMIT 5');
  console.table(res.rows);
  await pool.end();
})();
"

echo -e "\n=== Memory Usage ==="
pm2 describe twzrd-aggregator | grep memory
```

---

## Useful Commands

```bash
# View all logs
tail -f ~/.pm2/monitor.log

# Search for errors
grep "ERROR\|CRITICAL" ~/.pm2/monitor.log

# Count alerts by severity
grep -c "CRITICAL" ~/.pm2/monitor.log
grep -c "WARNING" ~/.pm2/monitor.log

# Restart all workers
pm2 restart all

# Stop monitoring
pkill -f "monitor.ts"

# Restart monitoring
./start-monitor.sh
```

---

## Next Steps

1. ✅ Run `./start-monitor.sh` to start the monitor
2. ✅ Check logs: `tail -f ~/.pm2/monitor.log`
3. ✅ Bookmark this guide for quick reference
4. ✅ Set up alerting integration (Slack, PagerDuty, etc.) - optional

---

**Questions?** Check the logs first: `tail -100 ~/.pm2/monitor.log`
