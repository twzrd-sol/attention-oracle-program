#!/usr/bin/env bash
# MIT License
# Archive (make read-only) GitHub repos in an org, preserving history & contributions.
# Requires: GitHub CLI `gh` authenticated with repo admin rights.
# Usage:
#   scripts/ops/github-archive-repos.sh --org twzrd-sol \
#     --exclude "attention-oracle-program claim-ui" --dry-run
#   scripts/ops/github-archive-repos.sh --org twzrd-sol --exclude "attention-oracle-program claim-ui"

set -euo pipefail

ORG="twzrd-sol"
EXCLUDE=("attention-oracle-program" "claim-ui")
DRY_RUN=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --org)
      ORG="$2"; shift 2;;
    --exclude)
      # space-separated string → array
      IFS=' ' read -r -a EXCLUDE <<< "$2"; shift 2;;
    --dry-run)
      DRY_RUN=1; shift;;
    *)
      echo "Unknown arg: $1"; exit 1;;
  esac
done

if ! command -v gh >/dev/null 2>&1; then
  echo "Error: GitHub CLI 'gh' not found. Install from https://cli.github.com/" >&2
  exit 1
fi

echo "Org: $ORG"
echo "Exclude: ${EXCLUDE[*]}"
echo "Mode: $([[ $DRY_RUN -eq 1 ]] && echo DRY-RUN || echo EXECUTE)"

mapfile -t CANDIDATES < <(gh repo list "$ORG" --json name,isArchived,visibility -L 500 \
  | jq -r '.[] | select(.isArchived==false) | .name')

should_exclude() {
  local name="$1"
  for x in "${EXCLUDE[@]}"; do
    [[ "$x" == "$name" ]] && return 0
  done
  return 1
}

COUNT=0
for name in "${CANDIDATES[@]}"; do
  if should_exclude "$name"; then
    printf "[skip] %s/%s\n" "$ORG" "$name"
    continue
  fi
  printf "[archive] %s/%s\n" "$ORG" "$name"
  if [[ $DRY_RUN -eq 0 ]]; then
    gh repo archive "$ORG/$name" -y
  fi
  COUNT=$((COUNT+1))
done

echo "Done. Repos processed: $COUNT"
echo "Tip: Unarchive via 'gh repo edit $ORG/<name> --archived=false' if needed."

