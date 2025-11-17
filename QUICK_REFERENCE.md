# Quick Reference - Off-Chain Operations

**For daily use**: Copy these commands to your terminal as needed.

---

## 📊 System Health Check (60 seconds)

```bash
# All-in-one dashboard
watch -n 1 'echo "=== SWAP ===" && free -h | grep Swap && echo "=== LOAD ===" && uptime && echo "=== PM2 ===" && pm2 list | head -12'
```

**What to watch for**:
- Swap: Should be < 20% (alert if > 20%)
- Load: Should be < 8 on 8-core system (alert if > 8)
- PM2: All services should show `online`

---

## 🔧 Daily Operations

### Check Alert Logs
```bash
# View latest hourly alerts
tail -20 /var/log/twzrd-daily-alerts.log

# Follow alerts in real-time
tail -f /var/log/twzrd-daily-alerts.log
```

### Check Service Logs
```bash
# View all PM2 logs
pm2 logs

# Follow specific service
pm2 logs milo-aggregator -f

# Last 50 lines of errors
pm2 logs --lines 100 | grep -i error
```

### Verify Services
```bash
# Quick status
pm2 list

# Detailed status
pm2 info <service-name>

# Recent restarts
pm2 list | grep -E "↺|restart"
```

---

## 🚨 Emergency Actions

### Swap Thrashing (> 50%)
```bash
# Clear swap immediately
sudo swapoff -a && sudo swapon -a
free -h

# If continues, identify memory hogs
ps aux --sort=-%mem | head -10
```

### High Load (> 10)
```bash
# Check what's consuming CPU
top -b -n 1 | head -20

# Check for error spikes
pm2 logs --lines 100 | grep -i error

# Restart specific service if stuck
pm2 restart <service-name>
```

### Service Crash Loop
```bash
# Stop the crashing service
pm2 stop <service-name>

# Check error logs
pm2 logs <service-name> --lines 100

# View config
grep -A 10 <service-name> ecosystem.config.js

# Restart when ready
pm2 restart <service-name>
```

---

## 📅 Scheduled Maintenance

**Monday 00:00 UTC** - Weekly health check (automated)
```bash
# Or run manually anytime:
bash /home/twzrd/milo-token/scripts/ops/weekly-health-check.sh
```

**Wednesday 02:00 UTC** - System updates (manual, low-traffic window)
```bash
sudo apt update && sudo apt upgrade -y && sudo apt autoclean
# Then reboot during window
```

**Friday 01:00 UTC** - Service restart (automated)
```bash
# Or run manually anytime:
pm2 restart all
sleep 2
curl -s http://localhost:8080/health | jq .
```

---

## 📋 Installation (One-Time Setup)

### Install Automated Maintenance Cron Jobs

```bash
# Interactive setup (recommended)
sudo bash /home/twzrd/milo-token/scripts/ops/CRONTAB_SETUP.sh

# Verify installation
crontab -l | grep twzrd
```

This installs three automation tasks:
- **Mon 00:00 UTC**: Weekly health check → `/var/log/twzrd-health.log`
- **Every hour**: Daily alerts → `/var/log/twzrd-daily-alerts.log`
- **Fri 01:00 UTC**: Service restart + verification → `/var/log/twzrd-restart.log`

---

## 📂 Important Files & Locations

### Automation Scripts
```
/home/twzrd/milo-token/scripts/ops/
├── weekly-health-check.sh       (Mon health check)
├── daily-alerts.sh              (Hourly alerts)
└── CRONTAB_SETUP.sh            (Cron installer)
```

### Configuration
```
/home/twzrd/milo-token/
├── ecosystem.config.js          (PM2 config + env vars)
├── .env                         (Local environment vars)
└── apps/twzrd-aggregator/      (Off-chain aggregator code)
```

### Documentation
```
/home/twzrd/milo-token/
├── OPERATIONAL_PROCEDURES.md    (Weekly tasks, emergencies)
├── MAINTENANCE_SCHEDULE.md      (Full schedule + checklists)
├── AUTOMATION_STATUS.md         (This automation's status)
├── SOURCE_OF_TRUTH.md          (Canonical file locations)
└── VPS_HEALTH_REPORT.md        (System baseline)
```

---

## 🎯 Success Indicators

**System is healthy when**:
- ✅ Swap < 20%
- ✅ Load < 8 (on 8-core system)
- ✅ All PM2 services `online`
- ✅ No ERROR spikes in logs (< 5 per hour)
- ✅ Restart counts stable (< 1 per day per service)

**Alert conditions**:
- 🚨 Swap > 20% → Review memory usage
- 🚨 Load > 8 → Check CPU, restart if stuck
- 🚨 Crashes > 0 → Review service logs, check config

---

## 📞 Getting Help

**Check the full guides**:
```bash
# Emergency procedures
cat /home/twzrd/milo-token/OPERATIONAL_PROCEDURES.md | grep "🚨 Emergency"

# Weekly schedule details
cat /home/twzrd/milo-token/MAINTENANCE_SCHEDULE.md

# Current automation status
cat /home/twzrd/milo-token/AUTOMATION_STATUS.md
```

---

**Last Updated**: 2025-11-15 08:52 UTC
**Temperature**: Deterministic (0.0) - Follow procedures exactly
**Owner**: Agent B (Off-Chain Infrastructure)
