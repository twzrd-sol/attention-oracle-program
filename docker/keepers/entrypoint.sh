#!/usr/bin/env bash
set -euo pipefail

json_log() {
  local level="$1" msg="$2"
  echo "{\"ts\":\"$(date -Iseconds)\",\"level\":\"$level\",\"keeper\":\"entrypoint\",\"msg\":\"$msg\"}" >&2
}

# ── Validation gates ──────────────────────────────────────────
if [ -z "${CLUSTER:-}" ]; then
  json_log "error" "CLUSTER is required (devnet | mainnet-beta)"
  exit 1
fi

if [ -z "${RPC_URL:-}" ]; then
  json_log "error" "RPC_URL is required"
  exit 1
fi

if [ -z "${KEYPAIR:-}" ]; then
  json_log "error" "KEYPAIR is required (path to keypair JSON)"
  exit 1
fi

if [ ! -f "$KEYPAIR" ]; then
  json_log "error" "Keypair file not found: $KEYPAIR"
  exit 1
fi

if [ -z "${CCM_V3_MINT:-}" ]; then
  json_log "error" "CCM_V3_MINT is required"
  exit 1
fi

if [ "$CLUSTER" = "mainnet-beta" ] && [ "${I_UNDERSTAND_MAINNET:-}" != "1" ]; then
  json_log "error" "Refusing mainnet without I_UNDERSTAND_MAINNET=1"
  exit 1
fi

# ── Doppler injection (optional) ─────────────────────────────
if [ -n "${DOPPLER_TOKEN:-}" ]; then
  json_log "info" "Doppler token detected, wrapping command"
  exec doppler run -- "$@"
fi

json_log "info" "Starting keeper (cluster=$CLUSTER)"
exec "$@"
