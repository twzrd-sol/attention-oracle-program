#!/usr/bin/env bash
set -euo pipefail
PROG_ID="GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
URL="${1:-https://api.mainnet-beta.solana.com}"
REMOTE="/tmp/${PROG_ID}.remote.so"
solana program dump "$PROG_ID" "$REMOTE" --url "$URL" >/dev/null
L=$(sha256sum target/deploy/token_2022.so | awk '{print $1}')
R=$(sha256sum "$REMOTE" | awk '{print $1}')
if [[ "$L" == "$R" ]]; then
  echo "OK: Local == Remote ($L)"
else
  echo "MISMATCH:\n  local:  $L\n  remote: $R"; exit 1
fi
