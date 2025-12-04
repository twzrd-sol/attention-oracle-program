#!/bin/bash
# Wrapper script for test-epoch-now.ts

cd "$(dirname "$0")"
ANCHOR_WALLET="${ANCHOR_WALLET:-$HOME/.config/solana/amm-admin.json}" \
  ts-node test-epoch-now.ts "$@"
