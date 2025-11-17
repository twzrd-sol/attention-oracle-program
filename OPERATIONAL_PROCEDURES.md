# Operational Procedures - Off-Chain Infrastructure

**Last Updated**: 2025-11-15
**Status**: Production
**Owner**: Agent B (Off-Chain)

---

## 🚀 Quick Start - Setup Automated Maintenance

**One-command setup** (requires sudo):
```bash
sudo bash /home/twzrd/milo-token/scripts/ops/CRONTAB_SETUP.sh
```

This installs three automated maintenance tasks:
- **Monday 00:00 UTC**: Weekly health check
- **Hourly**: Daily alert checks (swap, load, crashes)
- **Friday 01:00 UTC**: Service restart + health verification

---

## 📅 Weekly Maintenance Tasks

### Monday - Health Check (Automated)
```bash
# Manual run anytime:
./scripts/ops/weekly-health-check.sh

# Output:
# - Error log scan (last 7 days)
# - Memory & swap status
# - System load
# - PM2 process status
# - Alert thresholds (swap>20%, load>8)
# - Report saved to /tmp/weekly-health-report-YYYY-MM-DD.txt
```

### Wednesday - System Updates (Manual, low-traffic window 02:00 UTC)
```bash
# Update and reboot (PM2 auto-restarts on boot)
sudo apt update && sudo apt upgrade -y && sudo reboot

# After reboot, verify services:
pm2 list  # Should show all services as "online"
pm2 logs --lines 50  # Check for startup errors
```

### Friday - Service Restart (Automated)
```bash
# Automatic every Friday 01:00 UTC
# Manual run anytime:
pm2 restart all

# Verify health:
curl -s http://localhost:8080/health | jq .
pm2 list
```

---

## ⚠️ Alerting Thresholds

### Hourly Alert Checks (Automated)
Triggered alerts for:
- **Swap > 20%**: Emergency action needed
- **Load > 8** (on 8-core system): Investigate high CPU
- **Process crashes > 0**: Service issue detected

**View alert logs**:
```bash
tail -f /var/log/twzrd-daily-alerts.log
```

---

## 🚨 Emergency Response Runbook

### Swap Thrashing (>50%)
```bash
# Immediate action
sudo swapoff -a && sudo swapon -a
free -h  # Verify cleared

# If continues, identify memory hogs
ps aux --sort=-%mem | head -10

# Restart service if needed
pm2 restart milo-aggregator
```

### Load Spike (>12)
```bash
# Check what's consuming CPU
top -b -n 1 | head -20

# Check PM2 logs
pm2 logs --lines 100 | grep ERROR

# Restart problematic service
pm2 restart <service>
```

### Service Crash Loop (restarts > 5 in 5 min)
```bash
# Stop crashing service
pm2 stop <service>

# Check logs
pm2 logs <service> --lines 100

# Fix configuration if needed
# Then restart
pm2 restart <service>
```

---

## 📊 Monitoring Dashboard

**Real-time system view**:
```bash
# One-liner dashboard
watch -n 1 'echo "=== MEMORY ===" && free -h && echo -e "\n=== LOAD ===" && uptime && echo -e "\n=== PM2 ===" && pm2 list | head -15'
```

**Real-time logs**:
```bash
# All services
pm2 logs

# Specific service
pm2 logs milo-aggregator -f  # -f for follow
```

---

## ✅ Verification Checklist

**Daily**:
- [ ] Check alert logs: `tail /var/log/twzrd-daily-alerts.log`
- [ ] Verify `pm2 list` shows all services online

**Weekly**:
- [ ] Review health report: `/tmp/weekly-health-report-*`
- [ ] Check swap usage: `free -h | grep Swap`
- [ ] Verify no error spikes: `pm2 logs --lines 100 | grep ERROR`

**Monthly**:
- [ ] Review all weekly reports for patterns
- [ ] Check PM2 restart counts for anomalies
- [ ] Update this runbook if procedures changed

**Quarterly**:
- [ ] Review `ecosystem.config.js` environment variables
- [ ] Verify swappiness setting: `cat /proc/sys/vm/swappiness` (should be 10)
- [ ] Audit memory limits: `grep max_memory ecosystem.config.js`
- [ ] Test emergency procedures

---

## 🔧 Maintenance Scripts Location

```
scripts/ops/
├── weekly-health-check.sh      # Monday health scan
├── daily-alerts.sh              # Hourly threshold check
├── CRONTAB_SETUP.sh            # Install cron jobs
├── MAINTENANCE_SCHEDULE.md     # Full schedule
├── emergency-swap-clear.sh     # Manual swap clearing
└── ... (other operational scripts)
```

---

## 📝 Log Files

**Cron-generated logs**:
```
/var/log/twzrd-health.log        # Weekly health checks
/var/log/twzrd-daily-alerts.log  # Hourly alerts
/var/log/twzrd-restart.log       # Friday service restarts
```

**PM2 logs**:
```
~/.pm2/logs/milo-aggregator-out.log
~/.pm2/logs/milo-aggregator-error.log
~/.pm2/logs/cls-worker-s0-out.log
... (one pair per service)
```

---

## 🔗 Related Documentation

- **SOURCE_OF_TRUTH.md** - Canonical file locations
- **MAINTENANCE_SCHEDULE.md** - Detailed schedule + checklists
- **VPS_HEALTH_REPORT.md** - System baseline & long-term tuning
- **VPS_EMERGENCY_RESPONSE_COMPLETE.md** - Emergency procedures

---

**Status**: ✅ Production Ready
**Last Test**: 2025-11-15
**Next Review**: 2025-11-22 (weekly)
