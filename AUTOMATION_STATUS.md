# Operational Automation Status

**Last Updated**: 2025-11-15 08:52 UTC
**Status**: Scripts Ready, Cron Installation Pending

---

## ✅ COMPLETED: Automation Scripts

All maintenance scripts have been created and are executable:

```
/home/twzrd/milo-token/scripts/ops/
├── weekly-health-check.sh        (1.1K) ✅ Executable
├── daily-alerts.sh               (873B) ✅ Executable
├── CRONTAB_SETUP.sh             (1.2K) ✅ Executable (installer)
├── MAINTENANCE_SCHEDULE.md       (6.2K) ✅ Documentation
└── check-milo.sh                 (1.2K) ✅ Existing health script
```

### Script Details

**1. weekly-health-check.sh** (Mon 00:00 UTC)
- Scans PM2 logs for ERROR entries (last 7 days)
- Reports: Memory, swap usage, system load, process status
- Alerts: swap > 20%, load > 8
- Output: `/tmp/weekly-health-report-YYYY-MM-DD.txt`

**2. daily-alerts.sh** (Every hour)
- Monitors: Swap usage, load average, process restart count
- Thresholds: swap > 20%, load > 8, crashes > 0
- Output: `/var/log/twzrd-daily-alerts.log`

**3. CRONTAB_SETUP.sh** (Installer)
- One-command setup: `sudo bash scripts/ops/CRONTAB_SETUP.sh`
- Adds three cron entries (see below)
- Interactive confirmation before installing

**4. MAINTENANCE_SCHEDULE.md** (Full Guide)
- Weekly, quarterly, and emergency procedures
- Alert thresholds and response playbooks
- Monitoring dashboard commands

---

## ⏳ PENDING: Cron Job Installation

Three maintenance cron jobs are ready to install but NOT yet active:

### What Will Be Installed

```bash
# Monday 00:00 UTC - Weekly health check
0 0 * * 1 /home/twzrd/milo-token/scripts/ops/weekly-health-check.sh >> /var/log/twzrd-health.log 2>&1

# Every hour - Daily alert checks
0 * * * * /home/twzrd/milo-token/scripts/ops/daily-alerts.sh >> /var/log/twzrd-daily-alerts.log 2>&1

# Friday 01:00 UTC - Service restart + verification
0 1 * * 5 pm2 restart all && sleep 10 && pm2 list >> /var/log/twzrd-restart.log 2>&1
```

### Installation Steps

**Option A: Interactive Installation**
```bash
sudo bash /home/twzrd/milo-token/scripts/ops/CRONTAB_SETUP.sh
# Will prompt for confirmation before adding to crontab
```

**Option B: Manual Installation**
```bash
# Open crontab editor
crontab -e

# Paste these lines at the bottom:
# ============= TWZRD OFF-CHAIN MAINTENANCE (Nov 15, 2025) =============

# Weekly health check (Monday 00:00 UTC)
0 0 * * 1 /home/twzrd/milo-token/scripts/ops/weekly-health-check.sh >> /var/log/twzrd-health.log 2>&1

# Daily alerts (every hour - checks swap>20%, load>8, crashes)
0 * * * * /home/twzrd/milo-token/scripts/ops/daily-alerts.sh >> /var/log/twzrd-daily-alerts.log 2>&1

# Friday service restart (01:00 UTC - low traffic window)
0 1 * * 5 pm2 restart all && sleep 10 && pm2 list >> /var/log/twzrd-restart.log 2>&1

# =====================================================================
```

**Verify Installation**
```bash
crontab -l  # Should show all three new entries
```

---

## ✅ CURRENT SYSTEM HEALTH (Baseline)

**As of 2025-11-15 08:52 UTC**

### Memory & Swap
- **RAM**: 22GB / 31GB used (71%)
- **Swap**: 558MB / 8GB used (7%) ← Healthy (was 100% during emergency)
- **Available**: 4.7GB

### System Load
- **1-min**: 4.14
- **5-min**: 4.28
- **15-min**: 6.01
- **Status**: Stable (8-core system, threshold > 8 = alert)

### PM2 Services
**All Online** ✅
```
cls-worker-s0         online  85.2mb  0 restarts (3 days)
cls-worker-s1         online  83.5mb  0 restarts (3 days)
cls-worker-s2         online  77.5mb  0 restarts (2 days)
stream-listener       online  74.6mb  5 restarts (3 days)
tree-builder          online  62.9mb  7 restarts (3 days)
epoch-watcher         online  35.7mb  4 restarts (3 days)
gateway               online  92.4mb  0 restarts (2 days)
milo-aggregator       online  91.8mb  0 restarts (4 min) ✅ REBUILT
off-chain-monitor     online  57.3mb  2 restarts (6 days)
offchain-health-loop  online  51.1mb  5 restarts (114 min)
cls-discovery         stopped  -       -
```

---

## 📊 Log Destinations

After cron installation, logs will appear at:

```
/var/log/twzrd-health.log        # Weekly health checks (Mon 00:00)
/var/log/twzrd-daily-alerts.log  # Hourly alerts (all hours)
/var/log/twzrd-restart.log       # Friday restart verification (Fri 01:00)
```

**Monitor in Real-Time**
```bash
# View daily alerts as they arrive
tail -f /var/log/twzrd-daily-alerts.log

# View latest health report
cat /tmp/weekly-health-report-*.txt | tail -20
```

---

## 🔍 Existing Cron Job

**Already Active** (from earlier setup):
```bash
*/5 * * * * cd /home/twzrd/milo-token && bash scripts/monitor/health-check.sh >> logs/monitor/health.log 2>&1
```
- Runs: Every 5 minutes
- Purpose: Legacy health monitor
- Logs: `/home/twzrd/milo-token/logs/monitor/health.log`

---

## 📋 Next Actions

### Immediate (Required)
1. **Install cron jobs** (choose Option A or B above)
   ```bash
   sudo bash /home/twzrd/milo-token/scripts/ops/CRONTAB_SETUP.sh
   ```

2. **Verify installation**
   ```bash
   crontab -l | grep -E "health-check|daily-alerts|pm2 restart"
   ```

### Optional (Recommended)
3. **Create log rotation** for cron logs (prevent unbounded growth):
   ```bash
   sudo bash -c 'cat > /etc/logrotate.d/twzrd << EOF
   /var/log/twzrd-*.log {
       daily
       rotate 7
       compress
       delaycompress
       missingok
       notifempty
   }
   EOF'
   ```

4. **Set up email/Slack alerts** (edit daily-alerts.sh to send notifications)
   ```bash
   # Add at end of daily-alerts.sh:
   if [ -n "$ALERT" ]; then
       curl -X POST -H 'Content-type: application/json' \
           --data "{\"text\":\"$ALERT\"}" \
           $SLACK_WEBHOOK_URL
   fi
   ```

---

## 🔗 Related Documentation

- **OPERATIONAL_PROCEDURES.md** - Quick reference guide
- **MAINTENANCE_SCHEDULE.md** - Detailed weekly/quarterly procedures
- **VPS_HEALTH_REPORT.md** - System baseline and tuning notes
- **SOURCE_OF_TRUTH.md** - Canonical file locations

---

**Status**: Ready for cron installation
**Owner**: Agent B (Off-Chain Infrastructure)
**Next Review**: After cron jobs are installed (2025-11-16)
