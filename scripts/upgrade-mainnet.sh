#!/usr/bin/env bash
set -euo pipefail

PROGRAM_ID="GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
# Canonical build output (root target)
BIN_ROOT="target/deploy/token_2022.so"
# Legacy path (should not be used)
BIN_PROGRAMS="programs/target/deploy/token_2022.so"

echo "==> Network: mainnet-beta"
solana --version || true
solana config set --url mainnet-beta >/dev/null

# Ensure correct binary exists, build if missing
if [ ! -f "$BIN_ROOT" ]; then
  echo "==> Building SBF binary at $BIN_ROOT"
  (cd programs && cargo build-sbf)
fi

echo "==> Local binary hashes"
SHA_ROOT=$(sha256sum "$BIN_ROOT" | cut -d' ' -f1)
SIZE_ROOT=$(stat -c%s "$BIN_ROOT")
echo "  root:     $SHA_ROOT  ($SIZE_ROOT bytes)  $BIN_ROOT"

if [ -f "$BIN_PROGRAMS" ]; then
  SHA_PROG=$(sha256sum "$BIN_PROGRAMS" | cut -d' ' -f1)
  SIZE_PROG=$(stat -c%s "$BIN_PROGRAMS")
  echo "  programs: $SHA_PROG  ($SIZE_PROG bytes)  $BIN_PROGRAMS"
  if [ "$SHA_ROOT" != "$SHA_PROG" ]; then
    echo "WARNING: programs/ binary hash differs from root target. Proceeding with ROOT binary only." >&2
  fi
fi

AUTH=$(solana-keygen pubkey)
echo "==> Wallet pubkey: $AUTH"

echo "==> On-chain program info (before)"
solana program show "$PROGRAM_ID" || true

echo "==> Upgrading program with ROOT binary"
solana program deploy "$BIN_ROOT" \
  --program-id "$PROGRAM_ID" \
  --url mainnet-beta

echo "==> Post-upgrade verification"
TMP=$(mktemp /tmp/ao.upgrade.XXXXXX.so)
solana program dump "$PROGRAM_ID" "$TMP" --url mainnet-beta >/dev/null
SHA_ON=$(sha256sum "$TMP" | cut -d' ' -f1)
echo "  on-chain (full ProgramData): $SHA_ON  ($(stat -c%s "$TMP") bytes)"

echo "  trim-compare to local root size ($SIZE_ROOT bytes)"
SHA_TRIM=$(head -c "$SIZE_ROOT" "$TMP" | sha256sum | cut -d' ' -f1)
echo "  on-chain (trimmed):          $SHA_TRIM"
echo "  local:                       $SHA_ROOT"

if [ "$SHA_TRIM" != "$SHA_ROOT" ]; then
  echo "ERROR: Trimmed on-chain hash does not match local root binary!" >&2
  exit 1
fi

echo "==> Verified. Upgrade successful."
