#!/bin/bash
# Daily Alert Checker - Run hourly (0 * * * *)
# Thresholds: swap>20%, load>8, process crashes

SWAP_TOTAL=$(free | awk '/^Swap:/ {print $2}')
SWAP_USED=$(free | awk '/^Swap:/ {print $3}')
SWAP_PCT=$([ "$SWAP_TOTAL" -eq 0 ] && echo 0 || echo $((SWAP_USED * 100 / SWAP_TOTAL)))

LOAD=$(uptime | awk -F'load average:' '{print $2}' | awk '{print $1}')
LOAD_INT=${LOAD%.*}

CRASH_COUNT=$(pm2 list | awk '{print $8}' | grep -E '^[0-9]+$' | awk '$1 > 0' | wc -l)

ALERT=""
[ "$SWAP_PCT" -gt 20 ] && ALERT="$ALERT\n🚨 SWAP: ${SWAP_PCT}%"
[ "$LOAD_INT" -gt 8 ] && ALERT="$ALERT\n🚨 LOAD: $LOAD"
[ "$CRASH_COUNT" -gt 0 ] && ALERT="$ALERT\n⚠️ CRASHES: $CRASH_COUNT"

if [ -n "$ALERT" ]; then
  echo -e "$(date): $ALERT"
  echo -e "Swap: ${SWAP_PCT}% | Load: $LOAD | Crashes: $CRASH_COUNT"
else
  echo "$(date): ✅ All clear (Swap: ${SWAP_PCT}%, Load: $LOAD)"
fi
