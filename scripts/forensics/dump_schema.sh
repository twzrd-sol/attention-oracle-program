#!/usr/bin/env bash
set -euo pipefail

# Fail immediately if credentials are missing
: "${DATABASE_URL:?ERROR: Set DATABASE_URL or use PG* vars + PGPASSWORD/.pgpass}"

TABLES=(merkle_trees roots claims wallet_bindings)
OUT="schema_dump.sql"

echo "--- Dumping schema (DDL only) to ${OUT} ---"
# pg_dump respects DATABASE_URL; silent mode to avoid noise
pg_dump "${DATABASE_URL}" -s $(printf ' -t %s' "${TABLES[@]}") \
  --no-owner --no-privileges --lock-wait-timeout=5000 \
  --quiet > "${OUT}" 2>&1 \
  || { echo "❌ pg_dump failed. Check credentials and table names."; exit 1; }

[ -s "${OUT}" ] || { echo "❌ Empty dump. Check permissions on tables."; exit 1; }

echo "✅ Wrote ${OUT} $( [ -t 1 ] && echo "($(wc -l < \"${OUT}\") lines)" )"
echo "--- First 120 lines ---"
sed -n '1,120p' "${OUT}"

