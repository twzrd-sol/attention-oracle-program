#!/usr/bin/env bash
set -euo pipefail

# Prune/rotate noisy logs to keep audits clean.
# Default: DRY_RUN=1 (only list). Set DRY_RUN=0 to delete.
# Usage: DRY_RUN=1 DAYS=7 bash scripts/prune-transfer-hook-logs.sh [log_dir]

LOG_DIR="${1:-logs}"
DAYS="${DAYS:-7}"
DRY_RUN="${DRY_RUN:-1}"

if [[ ! -d "$LOG_DIR" ]]; then
  echo "No log dir: $LOG_DIR"
  exit 0
fi

echo "Scanning $LOG_DIR for logs older than $DAYS days..."

# Patterns to prune; extend as needed.
PATTERNS=(
  "*transfer-hook*.log*"
  "live-*.log"
)

FOUND=0
for pat in "${PATTERNS[@]}"; do
  while IFS= read -r -d '' file; do
    ((FOUND++)) || true
    if [[ "$DRY_RUN" == "1" ]]; then
      echo "DRY_RUN: would remove $file"
    else
      rm -f -- "$file"
      echo "Removed $file"
    fi
  done < <(find "$LOG_DIR" -type f -name "$pat" -mtime +"$DAYS" -print0 2>/dev/null || true)
done

if [[ $FOUND -eq 0 ]]; then
  echo "Nothing to prune. Logs are already clean."
fi

