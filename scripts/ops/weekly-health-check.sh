#!/bin/bash
# Weekly Health Check - Run every Monday 00:00 UTC
# Checks: logs for errors, memory/swap, load, process status

LOG_DIR="/home/twzrd/.pm2/logs"
REPORT_FILE="/tmp/weekly-health-report-$(date +%Y-%m-%d).txt"

{
  echo "=== Weekly Health Check Report ==="
  echo "Date: $(date)"
  echo ""
  
  # Error log scan
  echo "📋 ERROR LOG SCAN (Last 7 days)"
  ERROR_COUNT=$(find "$LOG_DIR" -name "*.log" -mtime -7 -exec grep -l "ERROR\|Error\|error" {} \; 2>/dev/null | wc -l)
  echo "Error files: $ERROR_COUNT"
  echo ""
  
  # Memory and swap
  echo "💾 MEMORY & SWAP STATUS"
  free -h
  echo ""
  
  # Load
  echo "📈 SYSTEM LOAD"
  uptime
  echo ""
  
  # PM2 status
  echo "🔄 PM2 PROCESS STATUS"
  pm2 list
  echo ""
  
  # Swap alert
  SWAP_USAGE=$(free | awk '/^Swap:/ {if ($2 == 0) print 0; else printf "%.0f", ($3/$2)*100}')
  if [ "$SWAP_USAGE" -gt 20 ]; then
    echo "🚨 SWAP ALERT: ${SWAP_USAGE}% (>20%)"
  else
    echo "✅ Swap: ${SWAP_USAGE}% (OK)"
  fi
  
  # Load alert
  LOAD=$(uptime | awk -F'load average:' '{print $2}' | awk '{print $1}' | cut -d'.' -f1)
  CORES=$(nproc)
  if [ "$LOAD" -gt 8 ]; then
    echo "🚨 LOAD ALERT: $LOAD (>8)"
  else
    echo "✅ Load: $LOAD (OK)"
  fi
  
  echo ""
  echo "Report: $REPORT_FILE"
} | tee "$REPORT_FILE"

echo "✅ Weekly health check complete"
