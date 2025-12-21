#!/bin/bash
# Initialize ExtraAccountMetaList for Token-2022 transfer hook (ccm_hook)
#
# This script calls `ccm_hook::initialize_extra_account_meta_list` for a CCM mint,
# enabling Token-2022 to pass extra accounts to the hook program.
#
# Usage:
#   CLUSTER=mainnet-beta KEYPAIR=~/.config/solana/id.json RPC_URL=... ./scripts/init-eaml.sh <MINT_ADDRESS>

set -e

# Args
MINT=${1:?❌ Usage: $0 <MINT_ADDRESS>}

CLUSTER=${CLUSTER:-}
if [[ -z "${CLUSTER}" ]]; then
  echo "❌ Missing CLUSTER. Set CLUSTER=localnet|devnet|testnet|mainnet-beta" >&2
  exit 2
fi
if [[ "${CLUSTER}" == "mainnet" ]]; then
  CLUSTER="mainnet-beta"
fi
if [[ "${CLUSTER}" == "mainnet-beta" && "${I_UNDERSTAND_MAINNET:-}" != "1" ]]; then
  echo "❌ Refusing mainnet without I_UNDERSTAND_MAINNET=1" >&2
  exit 2
fi

RPC_URL=${RPC_URL:-${ANCHOR_PROVIDER_URL:-${SYNDICA_RPC:-${SOLANA_RPC:-${SOLANA_URL:-}}}}}
if [[ -z "${RPC_URL}" ]]; then
  echo "❌ Missing RPC_URL (or ANCHOR_PROVIDER_URL/SYNDICA_RPC/SOLANA_RPC/SOLANA_URL)" >&2
  exit 2
fi

KEYPAIR=${KEYPAIR:-${ANCHOR_WALLET:-}}
if [[ -z "${KEYPAIR}" ]]; then
  echo "❌ Missing KEYPAIR. Set KEYPAIR=/path/to/keypair.json" >&2
  exit 2
fi

# Expand tilde in keypair path
KEYPAIR="${KEYPAIR/#\~/$HOME}"

echo "🚀 Initialize ExtraAccountMetaList for Transfer Hook"
echo ""
echo "📋 Configuration:"
echo "   Mint:     $MINT"
echo "   RPC:      $RPC_URL"
echo "   Keypair:  $KEYPAIR"
echo ""

# Derive EAML PDA using Solana CLI.
# EAML seed: ["extra-account-metas", mint] under the *hook program id*.
PROGRAM_ID="8VE2Wt5JNusGUCwkrpVmdwgdgyD6vYPEHop2g2CAArzS"
EAML_PDA=$(solana-program-address --program $PROGRAM_ID --seed "extra-account-metas" --seed-type bytes --seed "$MINT" --seed-type pubkey 2>/dev/null || echo "")

if [ -z "$EAML_PDA" ]; then
  echo "⚠️  Unable to derive EAML PDA via CLI. Using ts-node instead:"
  echo ""
  echo "   cd $(dirname $0)/.."
  echo "   npx ts-node scripts/init-eaml.ts $MINT"
  exit 1
fi

echo "🔐 EAML PDA: $EAML_PDA"
echo ""

# Check if EAML already exists
if solana account -u "$RPC_URL" "$EAML_PDA" >/dev/null 2>&1; then
  echo "✅ EAML already initialized for this mint."
  echo "   No action needed."
  exit 0
fi

echo "📡 Initializing EAML..."
echo ""
echo "Command: (requires ts-node + Anchor)"
echo "  npx ts-node scripts/init-eaml.ts $MINT"
echo ""
echo "Step-by-step:"
echo "  1. cd $(dirname $0)/.."
echo "  2. export CLUSTER='$CLUSTER'"
echo "  3. export RPC_URL='$RPC_URL'"
echo "  4. export KEYPAIR='$KEYPAIR'"
echo "  5. npx ts-node scripts/init-eaml.ts $MINT"
echo ""
