#!/bin/bash
# Initialize ExtraAccountMetaList for Token-2022 transfer hook (ccm_hook)
#
# This script calls `ccm_hook::initialize_extra_account_meta_list` for a CCM mint,
# enabling Token-2022 to pass extra accounts to the hook program.
#
# Usage:
#   ./scripts/init-eaml.sh <MINT_ADDRESS> [RPC_URL] [KEYPAIR_PATH]
#
# Examples:
#   ./scripts/init-eaml.sh 7XJ8KF3wYPn4YvD2jZqZ1z2qZ3Z4Z5Z6Z7Z8Z9ZaZ
#   ./scripts/init-eaml.sh 7XJ8KF3wYPn4YvD2jZqZ1z2qZ3Z4Z5Z6Z7Z8Z9ZaZ ${SYNDICA_RPC:-https://api.mainnet-beta.solana.com} ~/.config/solana/id.json

set -e

# Args
MINT=${1:?❌ Usage: $0 <MINT_ADDRESS> [RPC_URL] [KEYPAIR_PATH]}
RPC_URL=${2:-${SYNDICA_RPC:-https://api.mainnet-beta.solana.com}}
KEYPAIR=${3:-~/.config/solana/id.json}

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
echo "  2. export ANCHOR_PROVIDER_URL='$RPC_URL'"
echo "  3. export ANCHOR_WALLET='$KEYPAIR'"
echo "  4. npx ts-node scripts/init-eaml.ts $MINT"
echo ""
