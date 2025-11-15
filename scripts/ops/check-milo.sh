#!/usr/bin/env bash
set -euo pipefail

# Checks whether all MILO channels have activity in the last N minutes
# Usage: scripts/ops/check-milo.sh [minutes]

MINUTES=${1:-5}
DB=twzrd_oracle

# Load channel list from repo .env
ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
MILO_CHANNELS=$(grep -E '^MILO_CHANNELS=' "$ROOT_DIR/.env" | cut -d= -f2 | tr -d '"' | tr ',' '\n' | tr -d ' ' | sort -u)

if [[ -z "$MILO_CHANNELS" ]]; then
  echo "No MILO_CHANNELS found in .env" >&2
  exit 1
fi

echo "Checking MILO channels for activity in last $MINUTES minutes..."

NOW=$(date -u +%s)
CUTOFF=$(( NOW - MINUTES*60 ))

missing=()
while read -r ch; do
  [[ -z "$ch" ]] && continue
  count=$(sudo -u postgres psql -d "$DB" -t -A -c "SELECT COUNT(*) FROM channel_participation WHERE channel='${ch}' AND first_seen >= ${CUTOFF};")
  if [[ "$count" == "0" ]]; then
    missing+=("$ch")
  fi
done <<< "$MILO_CHANNELS"

if (( ${#missing[@]} == 0 )); then
  echo "✅ All MILO channels active within $MINUTES minutes."
else
  echo "⚠️  No activity in last $MINUTES minutes for: ${missing[*]}"
fi

