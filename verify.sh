#!/bin/bash
set -euo pipefail

PROGRAM_ID="GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
SOLVER_BIN="$HOME/.avm/bin/solana-verify"

echo "🔍 Verifying Attention Oracle Program ($PROGRAM_ID)..."

# 1) Deterministic build (host; Anchor will spawn its own container)
echo "🛠️  Building program (verifiable)..."
anchor build --verifiable

# 2) Dump on-chain program (host)
echo "📥  Fetching on-chain bytecode..."
solana program dump -u mainnet-beta "$PROGRAM_ID" dump.so >/dev/null || true

# 3) Canonical hash compare via solana-verify in a glibc-2.39 container
if [ -x "$SOLVER_BIN" ]; then
  echo "🔐  Computing canonical hashes (solana-verify in ubuntu:24.04)..."
  LOCAL_CANON=$(docker run --rm -v "$(pwd)":/work -v "$HOME/.avm/bin":/avm -w /work ubuntu:24.04 \
    bash -lc '/avm/solana-verify get-executable-hash target/verifiable/token_2022.so')
  REMOTE_CANON=$(docker run --rm -v "$(pwd)":/work -v "$HOME/.avm/bin":/avm -w /work ubuntu:24.04 \
    bash -lc "/avm/solana-verify get-program-hash $PROGRAM_ID")
  echo "Local (canonical):  $LOCAL_CANON"
  echo "Remote (canonical): $REMOTE_CANON"
  if [ "$LOCAL_CANON" = "$REMOTE_CANON" ]; then
    echo "✅ MATCH: Attestation passed (canonical hash)."
    rm -f dump.so
    exit 0
  else
    echo "❌ MISMATCH: Canonical hash differs."
    rm -f dump.so
    exit 1
  fi
fi

# Fallback to raw SHA compare
echo "⚖️  Comparing raw hashes (fallback)..."
LOCAL_SHA=$(sha256sum target/verifiable/token_2022.so | awk '{print $1}')
REMOTE_SHA=$(sha256sum dump.so | awk '{print $1}')

echo "Local (raw):  $LOCAL_SHA"
echo "Remote (raw): $REMOTE_SHA"
if [ "$LOCAL_SHA" = "$REMOTE_SHA" ]; then
  echo "✅ MATCH: Raw hashes equal."
  rm -f dump.so
  exit 0
else
  echo "❌ MISMATCH: Raw hashes differ."
  rm -f dump.so
  exit 1
fi
