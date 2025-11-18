#!/usr/bin/env bash
set -euo pipefail

BIN="programs/target/deploy/token_2022.so"

solana --version
echo "==> Switching to devnet"
solana config set --url devnet >/dev/null

if [ ! -f "$BIN" ]; then
  echo "==> Building SBF binary"
  (cd programs && cargo build-sbf)
fi

echo "==> Deploying to devnet"
OUT=$(solana program deploy "$BIN" --url devnet)
echo "$OUT"
PID=$(echo "$OUT" | rg -o "Program Id: ([A-Za-z0-9]+)" | awk '{print $3}')

echo "==> Show program"
solana program show "$PID" --url devnet

echo "==> Dump hash"
TMP=$(mktemp /tmp/ao.devnet.XXXXXX.so)
solana program dump "$PID" "$TMP" --url devnet >/dev/null
sha256sum "$TMP"

echo "==> Success. Program Id: $PID"

