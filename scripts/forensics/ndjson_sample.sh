#!/usr/bin/env bash
set -euo pipefail

CANDIDATES=(
  "/home/twzrd/milo-token/logs/stream-events.ndjson"
  "/home/twzrd/milo-token/apps/listener/logs/events.ndjson"
  "/var/log/twzrd/stream-listener.ndjson"
)

FOUND=""
for f in "${CANDIDATES[@]}"; do
  if [ -f "$f" ]; then
    FOUND="$f"
    echo "✅ Found: $FOUND"
    break
  fi
done

if [ -z "$FOUND" ]; then
  echo "--- Searching recent NDJSON files (last 7 days) ---"
  FOUND=$(find /home/twzrd -type f -name '*.ndjson' -mtime -7 -print 2>/dev/null | head -n 1 || true)
fi

if [ -n "$FOUND" ] && [ -f "$FOUND" ]; then
  echo "✅ NDJSON source: $FOUND"
  echo "--- Last 10 lines ---"
  tail -n 10 "$FOUND"
else
  echo "⚠️  No NDJSON file found in standard locations."
  echo "    If using PM2, verify listener is running: pm2 status"
  echo "    Check PM2 logs: pm2 logs stream-listener --lines 10"
  exit 1
fi

