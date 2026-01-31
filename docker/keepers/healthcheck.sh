#!/usr/bin/env bash
set -euo pipefail

HEARTBEAT_FILE="${HEALTHCHECK_FILE:-/tmp/keeper-heartbeat}"

# Gate 1: File must exist (keeper has ticked at least once)
if [ ! -f "$HEARTBEAT_FILE" ]; then
  exit 1
fi

# Gate 2: File must be recent (within MAX_AGE seconds)
# Default 600s (10 min) — safe for compound (5 min interval)
# Override to 7800 for harvest-fees (1 hour interval)
MAX_AGE="${HEALTHCHECK_MAX_AGE:-600}"
FILE_EPOCH=$(date -r "$HEARTBEAT_FILE" +%s)
NOW_EPOCH=$(date +%s)
AGE=$(( NOW_EPOCH - FILE_EPOCH ))

if [ "$AGE" -gt "$MAX_AGE" ]; then
  exit 1
fi

exit 0
