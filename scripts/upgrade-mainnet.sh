#!/usr/bin/env bash
set -euo pipefail

PROGRAM_ID="GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
BIN="programs/target/deploy/token_2022.so"

echo "==> Network: mainnet-beta"
solana --version
solana config set --url mainnet-beta >/dev/null

if [ ! -f "$BIN" ]; then
  echo "==> Building SBF binary"
  (cd programs && cargo build-sbf)
fi

echo "==> Binary details"
ls -lh "$BIN"
sha256sum "$BIN"

AUTH=$(solana-keygen pubkey)
echo "==> Wallet pubkey: $AUTH"

echo "==> On-chain program info (before)"
solana program show "$PROGRAM_ID"

echo "==> Upgrading program"
solana program deploy "$BIN" \
  --program-id "$PROGRAM_ID" \
  --url mainnet-beta

echo "==> On-chain program info (after)"
solana program show "$PROGRAM_ID"

TMP=$(mktemp /tmp/ao.upgrade.XXXXXX.so)
solana program dump "$PROGRAM_ID" "$TMP" --url mainnet-beta >/dev/null
echo "==> On-chain binary hash (post-upgrade)"
sha256sum "$TMP"

echo "==> Done."

