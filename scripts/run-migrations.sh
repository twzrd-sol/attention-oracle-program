#!/usr/bin/env bash
# scripts/run-migrations.sh
# Minimal migration runner for TWZRD schema changes

set -euo pipefail

# Ensure DATABASE_URL is set
if [ -z "${DATABASE_URL:-}" ]; then
  echo "ERROR: DATABASE_URL not set"
  exit 1
fi

# Create schema_migrations tracking table
psql "$DATABASE_URL" -c "
  CREATE TABLE IF NOT EXISTS schema_migrations (
    id TEXT PRIMARY KEY,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
  );
" > /dev/null

echo "🔍 Checking for pending migrations..."

# Apply each migration in order
for migration_file in db/migrations/*.sql; do
  migration_id=$(basename "$migration_file")

  # Check if migration already applied
  applied=$(psql -tA "$DATABASE_URL" \
    -c "SELECT 1 FROM schema_migrations WHERE id = '$migration_id'" || echo "")

  if [ -z "$applied" ]; then
    echo "📦 Applying migration: $migration_id"
    psql "$DATABASE_URL" -f "$migration_file"
    psql "$DATABASE_URL" \
      -c "INSERT INTO schema_migrations (id) VALUES ('$migration_id');" > /dev/null
    echo "✅ Applied: $migration_id"
  else
    echo "⏭️  Skipping already-applied: $migration_id"
  fi
done

echo "🎉 All migrations up to date!"
