#!/usr/bin/env bash
set -euo pipefail

# Create a clean distribution archive of the submission
# Excludes VCS history, build outputs, caches, and local artifacts.

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT_DIR"

OUT=${1:-submission_clean_$(date +%Y%m%d%H%M%S).zip}

echo "Creating $OUT ..."
zip -rq "$OUT" . \
  -x "*/node_modules/*" \
  -x "*/.next/*" \
  -x "*/dist/*" \
  -x "*/build/*" \
  -x "*/target/*" \
  -x "*/test-ledger/*" \
  -x "*/.anchor/*" \
  -x "*/.git/*" \
  -x "*.log" \
  -x "*.db" \
  -x "*.sqlite" \
  -x "*.DS_Store" \
  -x "*/.DS_Store" \
  -x "*.pem" \
  -x "*.key"

echo "Done. Archive at: $OUT"
