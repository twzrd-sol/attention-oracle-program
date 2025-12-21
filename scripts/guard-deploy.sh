#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)

CLUSTER=${CLUSTER:-}
if [[ -z "${CLUSTER}" ]]; then
  echo "Missing CLUSTER. Set CLUSTER=localnet|devnet|testnet|mainnet-beta" >&2
  exit 2
fi
if [[ "${CLUSTER}" != "mainnet-beta" && "${CLUSTER}" != "mainnet" ]]; then
  echo "guard-deploy is mainnet-only. Set CLUSTER=mainnet-beta" >&2
  exit 2
fi
if [[ "${CLUSTER}" == "mainnet" ]]; then
  CLUSTER="mainnet-beta"
fi
if [[ "${I_UNDERSTAND_MAINNET:-}" != "1" ]]; then
  echo "Refusing mainnet without I_UNDERSTAND_MAINNET=1" >&2
  exit 2
fi

KEYPAIR_PATH=${KEYPAIR:-${ANCHOR_WALLET:-}}
if [[ -z "${KEYPAIR_PATH}" ]]; then
  echo "Missing KEYPAIR. Set KEYPAIR=/path/to/keypair.json" >&2
  exit 2
fi

if [[ "$KEYPAIR_PATH" == /* ]]; then :; else KEYPAIR_PATH="$REPO_ROOT/$KEYPAIR_PATH"; fi
if [[ "$KEYPAIR_PATH" == $REPO_ROOT/* ]]; then
  echo "Refusing keypair inside repo: $KEYPAIR_PATH" >&2; exit 3
fi
[[ -f "$KEYPAIR_PATH" ]] || { echo "Keypair not found: $KEYPAIR_PATH" >&2; exit 4; }

PERMS=$(stat -c %a "$KEYPAIR_PATH" 2>/dev/null || stat -f %Lp "$KEYPAIR_PATH")
[[ "$PERMS" == "600" || "$PERMS" == "400" ]] || { echo "Keypair perms must be 600/400 (got $PERMS)" >&2; exit 5; }

RPC=${RPC_URL:-${ANCHOR_PROVIDER_URL:-${SYNDICA_RPC:-${SOLANA_RPC:-${SOLANA_URL:-}}}}}
if [[ -z "${RPC}" ]]; then
  echo "Missing RPC_URL (or ANCHOR_PROVIDER_URL/SYNDICA_RPC/SOLANA_RPC/SOLANA_URL)" >&2
  exit 6
fi

echo "[guard] mainnet op with $KEYPAIR_PATH on $RPC"; read -p "Type DEPLOY to continue: " c; [[ "$c" == DEPLOY ]] || exit 7

shift 0
"$@"
