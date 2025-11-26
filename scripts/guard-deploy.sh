#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
DEFAULT_RPC="https://api.mainnet-beta.solana.com"

KEYPAIR_PATH=${ANCHOR_WALLET:-${1:-}}
if [[ -z "${KEYPAIR_PATH}" ]]; then
  echo "Usage: ANCHOR_WALLET=~/.config/solana/id.json scripts/guard-deploy.sh <command ...>" >&2
  exit 2
fi

if [[ "$KEYPAIR_PATH" == /* ]]; then :; else KEYPAIR_PATH="$REPO_ROOT/$KEYPAIR_PATH"; fi
if [[ "$KEYPAIR_PATH" == $REPO_ROOT/* ]]; then
  echo "Refusing keypair inside repo: $KEYPAIR_PATH" >&2; exit 3
fi
[[ -f "$KEYPAIR_PATH" ]] || { echo "Keypair not found: $KEYPAIR_PATH" >&2; exit 4; }

PERMS=$(stat -c %a "$KEYPAIR_PATH" 2>/dev/null || stat -f %Lp "$KEYPAIR_PATH")
[[ "$PERMS" == "600" || "$PERMS" == "400" ]] || { echo "Keypair perms must be 600/400 (got $PERMS)" >&2; exit 5; }

RPC=${SOLANA_URL:-${SOLANA_RPC:-$DEFAULT_RPC}}
[[ "$RPC" == *"mainnet-beta"* ]] || { echo "Not mainnet RPC: $RPC" >&2; exit 6; }

echo "[guard] mainnet op with $KEYPAIR_PATH on $RPC"; read -p "Type DEPLOY to continue: " c; [[ "$c" == DEPLOY ]] || exit 7

shift 0
"$@"

