#!/usr/bin/env bash
set -euo pipefail

PGHOST="${PGHOST:-localhost}"
PGPORT="${PGPORT:-5432}"
PGUSER="${PGUSER:-twzrd}"
PGDATABASE="${PGDATABASE:-twzrd_oracle}"
PGPASSWORD="${PGPASSWORD:-twzrd_password_2025}"
export PGPASSWORD

ROWS=$(psql -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -d "$PGDATABASE" -t -X -q -P pager=off \
  -c "SELECT COUNT(*) FROM sealed_epochs WHERE LOWER(channel) IN ('thraedguy','thread_guy','threadguys');")

ROWS=$(echo "$ROWS" | xargs)
ts=$(date -u +'%Y-%m-%dT%H:%M:%SZ')
if [[ "$ROWS" != "0" ]]; then
  msg="[$ts] GUARDRAIL: sealed_epochs contains $ROWS rows for blocklisted channels"
  echo "$msg" | tee -a "$HOME/logs/guardrails.log"
  if [[ -n "${SLACK_WEBHOOK_URL:-}" ]]; then
    curl -sS -X POST -H 'Content-type: application/json' --data "{\"text\":\"$msg\"}" "$SLACK_WEBHOOK_URL" >/dev/null || true
  fi
else
  echo "[$ts] guardrail OK (0 sealed rows)" >> "$HOME/logs/guardrails.log"
fi

