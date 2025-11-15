#!/usr/bin/env bash
set -euo pipefail

echo "Verifying on-chain embedded security.txt markers in local .so..."
BIN="clean-hackathon/target/deploy/token_2022.so"
if [[ -f "$BIN" ]]; then
  strings "$BIN" | sed -n '/BEGIN SECURITY.TXT/,/END SECURITY.TXT/p'
else
  echo "Binary not found at $BIN; build with: (cd clean-hackathon/programs/token-2022 && cargo build-sbf)" >&2
fi

echo
echo "Verifying web /.well-known/security.txt content..."
WEBFILE="clean-hackathon/public/.well-known/security.txt"
if [[ -f "$WEBFILE" ]]; then
  echo "--- $WEBFILE ---"
  cat "$WEBFILE"
else
  echo "Web security.txt not found at $WEBFILE" >&2
fi

echo
echo "To verify on mainnet after upgrade:"
echo "  solana program dump GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop /tmp/prog.so"
echo "  strings /tmp/prog.so | sed -n '/BEGIN SECURITY.TXT/,/END SECURITY.TXT/p'"

