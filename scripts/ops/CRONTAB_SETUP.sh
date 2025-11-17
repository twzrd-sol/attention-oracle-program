#!/bin/bash
# Automated Crontab Setup for Off-Chain Maintenance
# Run this to install all maintenance cron jobs
#
# Usage: sudo bash scripts/ops/CRONTAB_SETUP.sh

CRON_ENTRIES="
# ============= TWZRD OFF-CHAIN MAINTENANCE (Nov 15, 2025) =============

# Weekly health check (Monday 00:00 UTC)
0 0 * * 1 /home/twzrd/milo-token/scripts/ops/weekly-health-check.sh >> /var/log/twzrd-health.log 2>&1

# Daily alerts (every hour - checks swap>20%, load>8, crashes)
0 * * * * /home/twzrd/milo-token/scripts/ops/daily-alerts.sh >> /var/log/twzrd-daily-alerts.log 2>&1

# Database health check (daily 01:30 UTC - connections, bloat, indexes)
30 1 * * * /home/twzrd/milo-token/scripts/ops/db-health-check.sh >> /var/log/twzrd-db-health.log 2>&1

# Database vacuum & analyze (Wed 03:00 UTC - clean dead rows, update stats)
0 3 * * 3 /home/twzrd/milo-token/scripts/ops/db-vacuum-analyze.sh >> /var/log/twzrd-db-maint.log 2>&1

# Friday service restart (01:00 UTC - low traffic window)
0 1 * * 5 pm2 restart all && sleep 10 && pm2 list >> /var/log/twzrd-restart.log 2>&1

# =====================================================================
"

echo "Installing maintenance cron jobs..."
echo "This will add the following entries to crontab:"
echo "$CRON_ENTRIES"
echo ""
read -p "Continue? (y/n): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
  (crontab -l 2>/dev/null; echo "$CRON_ENTRIES") | crontab -
  echo "✅ Cron jobs installed"
  echo ""
  echo "Verify with: crontab -l"
else
  echo "Aborted"
  exit 1
fi
