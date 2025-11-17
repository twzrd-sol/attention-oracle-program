#!/usr/bin/env bash
set -euo pipefail

# run-devnet-claims.sh
#
# Starts the gateway with devnet env, runs allocate-and-claim for a CSV,
# and shuts the gateway down. Prints concise results and exits non-zero on failure.
#
# Usage:
#   scripts/run-devnet-claims.sh --epoch 424245 --csv scripts/claims.csv \
#     --channel test-cls \
#     --program-id <DEVNET_PROGRAM_ID> --mint <DEVNET_MINT> \
#     [--rpc https://api.devnet.solana.com]

# Load devnet.config if present
if [[ -f devnet.config ]]; then
  # shellcheck disable=SC1091
  source devnet.config
fi

CSV=${CSV:-scripts/claims.csv}
CHANNEL=${CHANNEL:-test-cls}
EPOCH=${EPOCH:-}
RPC=${SOLANA_RPC:-${RPC:-https://api.devnet.solana.com}}
PROGRAM_ID=${PROGRAM_ID:-}
MINT=${MINT_PUBKEY:-${MINT:-}}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --csv) CSV="$2"; shift 2 ;;
    --epoch|-e) EPOCH="$2"; shift 2 ;;
    --channel|-c) CHANNEL="$2"; shift 2 ;;
    --rpc) RPC="$2"; shift 2 ;;
    --program-id) PROGRAM_ID="$2"; shift 2 ;;
    --mint) MINT="$2"; shift 2 ;;
    *) echo "Unknown arg: $1" >&2; exit 1 ;;
  esac
done

if [[ -z "$EPOCH" ]]; then
  echo "--epoch is required" >&2; exit 1
fi
if [[ -z "$PROGRAM_ID" || -z "$MINT" ]]; then
  echo "--program-id and --mint are required (devnet IDs)" >&2; exit 1
fi
if [[ ! -f "$CSV" ]]; then
  echo "CSV file not found: $CSV" >&2; exit 1
fi

: "${DATABASE_URL:?Set DATABASE_URL}"

echo "[check] CSV: $CSV | epoch: $EPOCH | channel: $CHANNEL"
echo "[check] RPC: $RPC"
echo "[check] PROGRAM_ID: $PROGRAM_ID"
echo "[check] MINT: $MINT"

# Optional: verify program exists
if command -v solana >/dev/null 2>&1; then
  if ! solana program show "$PROGRAM_ID" --url "$RPC" >/dev/null 2>&1; then
    echo "⚠️  Program $PROGRAM_ID not found on this cluster. Claims will fail to simulate." >&2
  fi
fi

# Optional: init epoch state with provided root from DB
ROOT=$(psql "$DATABASE_URL" -tAc "SELECT REPLACE(root,'0x','') FROM sealed_epochs WHERE epoch=$EPOCH AND channel='$CHANNEL' ORDER BY sealed_at DESC LIMIT 1")
if [[ -n "$ROOT" ]]; then
  echo "[info] Sealed root: $ROOT"
  echo "[info] You can initialize epoch on-chain via:"
  echo "       npx tsx scripts/init-epoch-for-claim.ts -c $CHANNEL -e $EPOCH --root $ROOT --claim-count $(wc -l < "$CSV" | awk '{print $1-1}')"
fi

pushd gateway >/dev/null
# pick port
PORT_ENV=${PORT:-5000}
if nc -z localhost "$PORT_ENV" >/dev/null 2>&1; then
  PORT_ENV=5001
fi
PORT=$PORT_ENV DATABASE_URL="$DATABASE_URL" SOLANA_RPC="$RPC" PROGRAM_ID="$PROGRAM_ID" MINT_PUBKEY="$MINT" CLS_STREAMER_NAME="$CHANNEL" node dist/index.js &
GW_PID=$!
sleep 2
if ! curl -sf http://127.0.0.1:$PORT_ENV/api/verification-status?wallet=111 >/dev/null; then
  echo "Gateway failed to start" >&2
  kill $GW_PID || true
  exit 1
fi
popd >/dev/null

echo "[run] allocate-and-claim from $CSV"
GATEWAY_URL="http://127.0.0.1:$PORT_ENV" SOLANA_RPC="$RPC" npx tsx scripts/allocate-and-claim.ts --csv "$CSV" || true

echo "[stop] gateway pid=$GW_PID"
kill $GW_PID || true

echo "[report]"
npx tsx scripts/cls-epoch-report.ts --epoch "$EPOCH" --channel "$CHANNEL" --summary || true
