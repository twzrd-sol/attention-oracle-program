#!/usr/bin/env bash
set -euo pipefail

# DigitalOcean Managed DB secrets rotation helper (PG + Valkey)
# - Dry-run by default. Set CONFIRM=1 to actually rotate and write files.
# - Requires: doctl (authed), jq, sed, curl, pm2
# - For Valkey, this script expects NEW_VALKEY_PASSWORD to be provided (UI rotation step),
#   because doctl does not expose a reset-password command for Valkey in this version.

echo "== TWZRD secrets rotation helper (PG + Valkey) =="

CONFIRM="${CONFIRM:-0}"
PG_USER="${PG_USER:-doadmin}"
PG_CLUSTER_NAME="${PG_CLUSTER_NAME:-twzrd-prod-postgres}"
VALKEY_CLUSTER_NAME="${VALKEY_CLUSTER_NAME:-twzrd-bullmq-redis}"
PG_CA_CERT_PATH="${PG_CA_CERT_PATH:-/home/twzrd/certs/do-managed-db-ca.crt}"

timestamp() { date +%Y%m%d_%H%M%S; }
mask() { sed -E 's#(://[^:]+:)[^@]+#\1****#g'; }

require() {
  command -v "$1" >/dev/null 2>&1 || { echo "Missing dependency: $1"; exit 1; }
}

for bin in doctl jq sed curl; do require "$bin"; done

echo "-- Discovering cluster IDs via doctl --"
PG_ID=$(doctl databases list --format ID,Engine,Name --no-header | awk -v n="$PG_CLUSTER_NAME" '$2=="pg" && $3==n{print $1}')
VALKEY_ID=$(doctl databases list --format ID,Engine,Name --no-header | awk -v n="$VALKEY_CLUSTER_NAME" '$2=="valkey" && $3==n{print $1}')
echo "   PG_ID=$PG_ID"
echo "   VALKEY_ID=$VALKEY_ID"

if [ -z "$PG_ID" ]; then echo "Could not find PG cluster '$PG_CLUSTER_NAME'"; exit 1; fi
if [ -z "$VALKEY_ID" ]; then echo "Could not find Valkey cluster '$VALKEY_CLUSTER_NAME'"; exit 1; fi

BACKUP_DIR="/home/twzrd/backups/env_$(timestamp)"
mkdir -p "$BACKUP_DIR"

ROOT_ENV="/home/twzrd/milo-token/.env"
AGG_ENV="/home/twzrd/milo-token/apps/twzrd-aggregator/.env"
ECOSYS="/home/twzrd/milo-token/ecosystem.config.js"

echo "-- Backing up env files to $BACKUP_DIR --"
[ -f "$ROOT_ENV" ] && cp "$ROOT_ENV" "$BACKUP_DIR/.env"
[ -f "$AGG_ENV" ] && cp "$AGG_ENV" "$BACKUP_DIR/aggregator.env"
[ -f "$ECOSYS" ] && cp "$ECOSYS" "$BACKUP_DIR/ecosystem.config.js"

echo "-- Current DATABASE_URL (masked) --"
grep -h "^DATABASE_URL=" "$ROOT_ENV" "$AGG_ENV" 2>/dev/null | mask || true

echo "\n== PLAN =="
echo "1) Reset Postgres user password via doctl (cluster $PG_ID, user $PG_USER)."
echo "2) Update DATABASE_URL in .env files and ecosystem.config.js."
echo "3) Rotate Valkey password via DO UI, then set NEW_VALKEY_PASSWORD env and update REDIS_URL."
echo "4) pm2 restart milo-aggregator; verify health + DB connection (CA TLS)."

if [ "${CONFIRM}" != "1" ]; then
  echo "\nDry-run mode. Set CONFIRM=1 to execute."
  exit 0
fi

echo "-- Rotating Postgres user password --"
RESET_OUT_JSON="/home/twzrd/backups/pg_reset_$(timestamp).json"
doctl databases user reset "$PG_ID" "$PG_USER" -o json > "$RESET_OUT_JSON"
NEW_PG_PASSWORD=$(jq -r '..|.password? // empty' "$RESET_OUT_JSON" | head -n1)
if [ -z "$NEW_PG_PASSWORD" ]; then
  echo "Failed to extract new PG password from $RESET_OUT_JSON"; exit 1;
fi
echo "   New PG password captured (hidden)."

echo "-- Updating .env files with new PG password --"
update_pg_url() {
  local file="$1"
  [ -f "$file" ] || return 0
  tmp="${file}.tmp.$(timestamp)"
  # Replace password between : and @ in postgresql://user:pass@host
  sed -E "s#(postgresql://[^:]+:)[^@]+#\1${NEW_PG_PASSWORD//\//\\/}#" "$file" > "$tmp"
  mv "$tmp" "$file"
}
update_pg_url "$ROOT_ENV"
update_pg_url "$AGG_ENV"

echo "-- Updating ecosystem.config.js PG URL + removing NODE_TLS_REJECT_UNAUTHORIZED --"
if [ -f "$ECOSYS" ]; then
  sed -i -E "s#(postgresql://[^:]+:)[^@]+#\1${NEW_PG_PASSWORD//\//\\/}#" "$ECOSYS"
  sed -i -E "/NODE_TLS_REJECT_UNAUTHORIZED/s/'0'/'1'/" "$ECOSYS"
fi

if [ -n "${NEW_VALKEY_PASSWORD:-}" ]; then
  echo "-- Updating REDIS_URL in ecosystem.config.js with NEW_VALKEY_PASSWORD --"
  if [ -f "$ECOSYS" ]; then
    sed -i -E "s#(rediss?://[^:]+:)[^@]+#\1${NEW_VALKEY_PASSWORD//\//\\/}#" "$ECOSYS"
  fi
  for file in "$ROOT_ENV" "$AGG_ENV"; do
    [ -f "$file" ] || continue
    sed -i -E "s#(REDIS_URL=rediss?://[^:]+:)[^@]+#\1${NEW_VALKEY_PASSWORD//\//\\/}#" "$file" || true
  done
else
  echo "NOTE: NEW_VALKEY_PASSWORD not set; skipping REDIS_URL updates."
fi

echo "-- Restarting aggregator via pm2 --"
pm2 restart milo-aggregator --update-env || pm2 restart 58 || true
sleep 2
echo "-- Health check --"
curl -fsS --max-time 5 http://localhost:8080/health || true
echo
echo "-- Quick CA TLS query --"
node -e "import {Pool} from 'pg'; import fs from 'fs'; const conn=process.env.DATABASE_URL || require('fs').readFileSync(process.argv[1],'utf8'); const ssl={ca:fs.readFileSync(process.env.PG_CA_CERT_PATH||'${PG_CA_CERT_PATH}','utf8'),rejectUnauthorized:true}; const p=new Pool({connectionString:conn,ssl}); p.query('select 1').then(()=>console.log('select1 ok')).catch(e=>console.error('select1 failed',e.message)).finally(()=>p.end());" "$AGG_ENV" 2>/dev/null || true

echo "== Rotation complete =="

