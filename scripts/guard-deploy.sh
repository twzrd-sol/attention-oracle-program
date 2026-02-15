#!/usr/bin/env bash
# guard-deploy.sh — Pre-flight checks for mainnet program deployments.
# Validates cluster, keypair security, and RPC availability before
# delegating to the actual deploy command.
set -euo pipefail

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)

# ---------------------------------------------------------------------------
# 1. Cluster validation
# ---------------------------------------------------------------------------
CLUSTER=${CLUSTER:-}
if [[ -z "${CLUSTER}" ]]; then
  echo "❌ Missing CLUSTER. Set CLUSTER=mainnet-beta" >&2
  exit 2
fi
# Normalize shorthand
if [[ "${CLUSTER}" == "mainnet" ]]; then
  CLUSTER="mainnet-beta"
fi
if [[ "${CLUSTER}" != "mainnet-beta" ]]; then
  echo "❌ guard-deploy is mainnet-only. Got CLUSTER=${CLUSTER}" >&2
  exit 2
fi
if [[ "${I_UNDERSTAND_MAINNET:-}" != "1" ]]; then
  echo "❌ Refusing mainnet without I_UNDERSTAND_MAINNET=1" >&2
  exit 2
fi

# ---------------------------------------------------------------------------
# 2. Keypair validation
# ---------------------------------------------------------------------------
KEYPAIR_PATH=${KEYPAIR:-${ANCHOR_WALLET:-}}
if [[ -z "${KEYPAIR_PATH}" ]]; then
  echo "❌ Missing KEYPAIR. Set KEYPAIR=/path/to/keypair.json" >&2
  exit 2
fi

# Resolve relative paths against repo root
if [[ "$KEYPAIR_PATH" != /* ]]; then
  KEYPAIR_PATH="$REPO_ROOT/$KEYPAIR_PATH"
fi

# Security: reject keypairs stored inside the repo tree
if [[ "$KEYPAIR_PATH" == "$REPO_ROOT"/* ]]; then
  echo "❌ Refusing keypair inside repo: $KEYPAIR_PATH" >&2
  exit 3
fi

if [[ ! -f "$KEYPAIR_PATH" ]]; then
  echo "❌ Keypair not found: $KEYPAIR_PATH" >&2
  exit 4
fi

# Verify restrictive file permissions (owner-only)
PERMS=$(stat -c %a "$KEYPAIR_PATH" 2>/dev/null || stat -f %Lp "$KEYPAIR_PATH")
if [[ "$PERMS" != "600" && "$PERMS" != "400" ]]; then
  echo "❌ Keypair perms must be 600 or 400 (got $PERMS): $KEYPAIR_PATH" >&2
  exit 5
fi

# ---------------------------------------------------------------------------
# 3. RPC endpoint resolution (checked in priority order)
# ---------------------------------------------------------------------------
resolve_rpc() {
  local -a candidates=(
    "${RPC_URL:-}"
    "${ANCHOR_PROVIDER_URL:-}"
    "${AO_RPC_URL:-}"
    "${SOLANA_RPC_URL:-}"
    "${SOLANA_RPC:-}"
    "${SOLANA_URL:-}"
  )
  for url in "${candidates[@]}"; do
    if [[ -n "${url}" ]]; then
      echo "${url}"
      return 0
    fi
  done
  return 1
}

RPC=$(resolve_rpc) || {
  echo "❌ Missing RPC endpoint. Set one of: RPC_URL, ANCHOR_PROVIDER_URL, AO_RPC_URL, SOLANA_RPC_URL, SOLANA_RPC, SOLANA_URL" >&2
  exit 6
}

# ---------------------------------------------------------------------------
# 4. Confirmation gate
# ---------------------------------------------------------------------------
echo "[guard] mainnet deploy with keypair=$KEYPAIR_PATH rpc=${RPC:0:50}..."
read -p "Type DEPLOY to continue: " confirmation
if [[ "$confirmation" != "DEPLOY" ]]; then
  echo "Aborted." >&2
  exit 7
fi

# ---------------------------------------------------------------------------
# 5. Delegate to the wrapped command
# ---------------------------------------------------------------------------
shift 0
"$@"
