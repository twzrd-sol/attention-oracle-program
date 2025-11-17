#!/usr/bin/env bash
# scripts/check-schema.sh
# Pre-deployment schema validation gate
# Fails fast if production DB doesn't match application expectations

set -euo pipefail

# Ensure DATABASE_URL is set
if [ -z "${DATABASE_URL:-}" ]; then
  echo "ERROR: DATABASE_URL not set"
  exit 1
fi

echo "🔍 Validating production schema..."

# Assert required columns exist
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 << 'EOF'
DO $$
BEGIN
  -- Check sealed_epochs.token_group
  IF NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'sealed_epochs'
      AND column_name = 'token_group'
  ) THEN
    RAISE EXCEPTION 'SCHEMA VALIDATION FAILED: sealed_epochs.token_group missing';
  END IF;

  -- Check sealed_participants.token_group
  IF NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'sealed_participants'
      AND column_name = 'token_group'
  ) THEN
    RAISE EXCEPTION 'SCHEMA VALIDATION FAILED: sealed_participants.token_group missing';
  END IF;

  -- Add more validations here as schema evolves
  -- Example:
  -- IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'foo' AND column_name = 'bar') THEN
  --   RAISE EXCEPTION 'SCHEMA VALIDATION FAILED: foo.bar missing';
  -- END IF;

  RAISE NOTICE 'Schema validation passed ✅';
END$$;
EOF

echo "✅ Schema validation OK"
