# Off-Chain Infrastructure Maintenance Schedule

**Last Updated**: 2025-11-15
**Owner**: Agent B (Off-Chain)
**Status**: Production

---

## 📅 Weekly Maintenance Schedule

### Monday - Weekly Health Check (00:00 UTC)
**Script**: `scripts/ops/weekly-health-check.sh`

**Tasks**:
```bash
# Manual run:
./scripts/ops/weekly-health-check.sh

# Or automated via cron:
0 0 * * 1 /home/twzrd/milo-token/scripts/ops/weekly-health-check.sh >> /var/log/twzrd-health.log 2>&1
```

**Checks**:
- [ ] Scan PM2 logs for ERROR entries (last 7 days)
- [ ] Memory & swap status (`free -h`)
- [ ] System load (`uptime`)
- [ ] PM2 process status (`pm2 list`)
- [ ] Swap usage alert (>20%)
- [ ] Load average alert (>8)
- [ ] Process restart count

**Output**:
- Report file: `/tmp/weekly-health-report-YYYY-MM-DD.txt`
- Slack notification (if webhook configured)

---

### Wednesday - System Updates (02:00 UTC, low traffic)
**Script**: Manual (requires sudo)

**Tasks**:
```bash
# 1. Update package manager
sudo apt update

# 2. Upgrade packages (non-interactive)
sudo apt upgrade -y

# 3. Clean up
sudo apt autoclean && sudo apt autoremove -y

# 4. Verify changes
sudo reboot  # During low-traffic window (2-4 UTC)

# PM2 will auto-restart services on boot ✅
pm2 resurrect  # Verify all services restarted
pm2 list
```

**Expected Outcome**:
- System updates applied
- Boot confirms all PM2 services auto-restart
- No manual intervention needed

---

### Wednesday - Database Maintenance (03:00 UTC)
**Script**: `scripts/ops/db-vacuum-analyze.sh` (automated)

**Tasks**:
```bash
# Automated weekly vacuum & analyze
# Cleans dead rows, updates query planner statistics
0 3 * * 3 /home/twzrd/milo-token/scripts/ops/db-vacuum-analyze.sh >> /var/log/twzrd-db-maint.log 2>&1

# Or run manually:
./scripts/ops/db-vacuum-analyze.sh
```

**What It Does**:
- [ ] VACUUM: Reclaims space from deleted rows
- [ ] ANALYZE: Updates table statistics for query planner
- [ ] Reports table sizes and schema health
- [ ] Logs to `/var/log/twzrd-db-maint.log`

**Expected Results**:
- Cleaner tables, better query performance
- Index statistics refreshed
- Zero downtime (PostgreSQL-native operation)

---

### Daily - Database Health Check (01:30 UTC)
**Script**: `scripts/ops/db-health-check.sh` (automated)

**Monitoring**:
```bash
# Automated daily health check
30 1 * * * /home/twzrd/milo-token/scripts/ops/db-health-check.sh >> /var/log/twzrd-db-health.log 2>&1

# Or run manually:
./scripts/ops/db-health-check.sh
```

**Checks**:
- Active connections (alert if > 80% of max_connections)
- Database size growth
- Missing primary keys on tables
- Dead rows / table bloat
- Replication lag (if applicable)

**Output**: `/var/log/twzrd-db-health.log`

---

### Friday - Service Restart & Health Check (01:00 UTC)
**Script**: Manual

**Tasks**:
```bash
# 1. Restart all PM2 services (graceful, zero downtime)
pm2 restart all

# 2. Wait for services to stabilize
sleep 10

# 3. Verify health endpoints
curl -s http://localhost:8080/health | jq .
curl -s http://127.0.0.1:8089/health 2>&1 || echo "Health port TBD"

# 4. Check PM2 status
pm2 list

# 5. Verify no new errors in logs
pm2 logs --lines 50 | grep -i error || echo "No errors detected"
```

**Expected Results**:
- All services: `online` status
- Health check: `{"ok":true}`
- No new error logs

---

## 📊 Daily Automated Alerts (Every Hour)
**Script**: `scripts/ops/daily-alerts.sh`

**Setup**:
```bash
# Add to crontab:
0 * * * * /home/twzrd/milo-token/scripts/ops/daily-alerts.sh >> /var/log/twzrd-daily-alerts.log 2>&1
```

**Thresholds**:
- ⚠️ **Swap > 20%** → Alert (emergency if >80%)
- ⚠️ **Load > 8** (on 8-core system) → Alert
- ⚠️ **Process crashes > 0** → Alert

**Alert Methods** (set via environment):
```bash
export ALERT_EMAIL="admin@twzrd.xyz"
export SLACK_WEBHOOK="https://hooks.slack.com/services/..."
```

**Alert Content**:
- Current timestamp
- Swap usage % and absolute KB
- Load average
- Number of process restarts
- PM2 process table

---

## 🔧 Quarterly System Tuning

### Q1, Q2, Q3, Q4 - System Optimization Review
**Frequency**: Every 3 months

**Checklist**:
- [ ] Review `ecosystem.config.js` environment variables
- [ ] Verify all RPC URLs are responding
- [ ] Check database connection pool settings
- [ ] Review swappiness setting:
  ```bash
  # Should be 10 (prefer RAM over swap)
  cat /proc/sys/vm/swappiness

  # If not 10, set it:
  sudo sysctl vm.swappiness=10
  echo "vm.swappiness=10" | sudo tee -a /etc/sysctl.conf
  ```
- [ ] Audit memory limits in PM2:
  ```bash
  # Each service should have max_memory_restart
  cat ecosystem.config.js | grep max_memory
  ```
- [ ] Review Redis connection pool
- [ ] Check PostgreSQL connection limits

---

## 🚨 Emergency Procedures

### Swap Thrashing (>80%)
**Immediate Action**:
```bash
# 1. Clear swap
sudo swapoff -a && sudo swapon -a
free -h

# 2. If continues, identify heavy process
ps aux --sort=-%mem | head -10

# 3. Kill or restart if needed
pm2 restart <process>
```

### Load Spike (>10)
**Immediate Action**:
```bash
# 1. Check what's consuming CPU
top -b -n 1 | head -20

# 2. Check PM2 logs for errors
pm2 logs --lines 100 | grep ERROR

# 3. Restart problematic service
pm2 restart <process>

# 4. Monitor recovery
watch -n 1 uptime
```

### Service Crash Loop (restarts > 5 in 5 min)
**Immediate Action**:
```bash
# 1. Stop crashing service
pm2 stop <process>

# 2. Check error logs
pm2 logs <process> --lines 100

# 3. Review configuration
cat ecosystem.config.js | grep -A10 <process>

# 4. Fix issue and restart
pm2 restart <process>
```

---

## 📝 Crontab Setup

**Install automated maintenance**:
```bash
# Open crontab editor
crontab -e

# Add these lines (paste at bottom):
# ============= TWZRD OFF-CHAIN MAINTENANCE =============

# Weekly health check (Monday 00:00 UTC)
0 0 * * 1 /home/twzrd/milo-token/scripts/ops/weekly-health-check.sh >> /var/log/twzrd-health.log 2>&1

# Daily alerts (every hour)
0 * * * * /home/twzrd/milo-token/scripts/ops/daily-alerts.sh >> /var/log/twzrd-daily-alerts.log 2>&1

# Friday service restart (01:00 UTC - low traffic)
0 1 * * 5 pm2 restart all && sleep 10 && pm2 list >> /var/log/twzrd-restart.log 2>&1

# ============================================================
```

**Verify crontab**:
```bash
crontab -l  # List all scheduled jobs
```

**Monitor cron logs**:
```bash
# Check recent cron executions
sudo grep CRON /var/log/syslog | tail -20
```

---

## 📊 Monitoring Dashboard

**Real-time system health**:
```bash
# Watch memory in real-time
watch -n 1 'free -h && echo "---" && uptime && echo "---" && pm2 list'

# Watch PM2 logs in real-time
pm2 logs

# Watch specific service
pm2 logs milo-aggregator
```

---

## ✅ Maintenance Checklist Template

### Weekly (Monday)
- [ ] Run health check: `./scripts/ops/weekly-health-check.sh`
- [ ] Review alert log: `/var/log/twzrd-daily-alerts.log`
- [ ] Document any anomalies in `INCIDENTS.md`

### Monthly (First Friday)
- [ ] Review all weekly reports
- [ ] Check for patterns in restarts
- [ ] Update this schedule if procedures changed

### Quarterly (First Monday of quarter)
- [ ] Review ecosystem.config.js
- [ ] Tune system parameters
- [ ] Capacity planning (disk, memory, bandwidth)

---

## 🔗 Related Documentation

- **SOURCE_OF_TRUTH.md** - Canonical codebase locations
- **VPS_HEALTH_REPORT.md** - System baseline metrics
- **VPS_EMERGENCY_RESPONSE_COMPLETE.md** - Emergency procedures
- **TECHNICAL_ARCHITECTURE.md** - System design

---

**Last Reviewed**: 2025-11-15
**Next Review**: 2025-12-15 (monthly)
**Owner**: Agent B (Off-Chain Infrastructure)
