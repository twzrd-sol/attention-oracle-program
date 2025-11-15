#!/usr/bin/env bash
set -euo pipefail

# Nightly purge of blocklisted channels to keep near-zero footprint by morning.
# Channels: thraedguy, thread_guy, threadguys
# Tables: l2_tree_cache, sealed_participants, sealed_epochs, user_signals, channel_participation

PGHOST="${PGHOST:-localhost}"
PGPORT="${PGPORT:-5432}"
PGUSER="${PGUSER:-twzrd}"
PGDATABASE="${PGDATABASE:-twzrd_oracle}"
PGPASSWORD="${PGPASSWORD:-twzrd_password_2025}"

export PGPASSWORD

SQL=$(cat <<'EOSQL'
BEGIN;
  DELETE FROM l2_tree_cache         WHERE LOWER(channel) IN ('thraedguy','thread_guy','threadguys');
  DELETE FROM sealed_participants   WHERE LOWER(channel) IN ('thraedguy','thread_guy','threadguys');
  DELETE FROM sealed_epochs         WHERE LOWER(channel) IN ('thraedguy','thread_guy','threadguys');
  DELETE FROM user_signals          WHERE LOWER(channel) IN ('thraedguy','thread_guy','threadguys');
  DELETE FROM channel_participation WHERE LOWER(channel) IN ('thraedguy','thread_guy','threadguys');
COMMIT;
EOSQL
)

psql -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -d "$PGDATABASE" -X -q -P pager=off -c "$SQL"

echo "[$(date -u +'%Y-%m-%dT%H:%M:%SZ')] nightly purge complete" >> "$HOME/logs/nightly-purge.log"

