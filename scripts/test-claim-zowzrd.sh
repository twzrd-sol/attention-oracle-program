#!/usr/bin/env bash
# Test Claim Flow for ZoWzrd
# Usage: ./test-claim-zowzrd.sh [twitch_username] [channel]

set -e

TWITCH_USERNAME="${1:-zowzrd}"
CHANNEL="${2:-lacy}"
WALLET="${3:-YOUR_SOLANA_WALLET_HERE}"

echo "🔍 MILO Claim Test - ZoWzrd"
echo "======================================"
echo ""
echo "Twitch Username: $TWITCH_USERNAME"
echo "Channel: $CHANNEL"
echo "Wallet: $WALLET"
echo ""

# Get current and recent epochs
CURRENT_EPOCH=$(date -u +%s | awk '{print int($1/3600)*3600}')
PREV_EPOCH_1=$((CURRENT_EPOCH - 3600))
PREV_EPOCH_2=$((CURRENT_EPOCH - 7200))

echo "📅 Checking Recent Epochs:"
echo "  Current: $CURRENT_EPOCH ($(date -u -d @$CURRENT_EPOCH '+%Y-%m-%d %H:%M UTC'))"
echo "  -1 hour: $PREV_EPOCH_1 ($(date -u -d @$PREV_EPOCH_1 '+%Y-%m-%d %H:%M UTC'))"
echo "  -2 hours: $PREV_EPOCH_2 ($(date -u -d @$PREV_EPOCH_2 '+%Y-%m-%d %H:%M UTC'))"
echo ""

# Function to check proof
check_proof() {
  local epoch=$1
  local epoch_label=$2

  echo "🔎 Checking $epoch_label epoch ($epoch)..."

  RESPONSE=$(curl -s "http://127.0.0.1:8080/proof?channel=$CHANNEL&epoch=$epoch&user=$TWITCH_USERNAME")

  if echo "$RESPONSE" | jq -e '.error' > /dev/null 2>&1; then
    ERROR=$(echo "$RESPONSE" | jq -r '.error')
    echo "  ❌ No proof found: $ERROR"
    return 1
  else
    echo "  ✅ PROOF FOUND!"
    echo "$RESPONSE" | jq '{
      channel,
      epoch,
      username,
      index,
      weight,
      total_participants,
      root
    }'
    echo ""
    echo "📝 Full response saved to /tmp/proof-$epoch.json"
    echo "$RESPONSE" | jq '.' > "/tmp/proof-$epoch.json"
    return 0
  fi
}

# Check last 3 epochs
FOUND=false

if check_proof "$CURRENT_EPOCH" "Current"; then
  FOUND_EPOCH=$CURRENT_EPOCH
  FOUND=true
elif check_proof "$PREV_EPOCH_1" "Previous (-1h)"; then
  FOUND_EPOCH=$PREV_EPOCH_1
  FOUND=true
elif check_proof "$PREV_EPOCH_2" "Previous (-2h)"; then
  FOUND_EPOCH=$PREV_EPOCH_2
  FOUND=true
fi

if [ "$FOUND" = false ]; then
  echo ""
  echo "❌ No proofs found in last 3 epochs"
  echo ""
  echo "💡 Next Steps:"
  echo "  1. Verify your Twitch username is correct"
  echo "  2. Watch a MILO channel stream for 10+ minutes"
  echo "  3. Wait for next epoch seal (top of hour)"
  echo "  4. Run this script again"
  echo ""
  echo "📺 Current MILO Channels:"
  echo "  lacy, jasontheween, adapt, kaysan, silky, yourragegaming,"
  echo "  stableronaldo, threadguy, marlon, n3on, thesketchreal, orangieyt"
  exit 1
fi

echo ""
echo "================================"
echo "✅ PROOF FOUND - Ready to Claim!"
echo "================================"
echo ""

# Test claim transaction
echo "🔐 Testing claim transaction..."
CLAIM_RESPONSE=$(curl -s -X POST http://127.0.0.1:8082/api/milo/claim-open \
  -H "Content-Type: application/json" \
  -d "{
    \"wallet\": \"$WALLET\",
    \"channel\": \"$CHANNEL\",
    \"epoch\": $FOUND_EPOCH,
    \"mint\": \"AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5\"
  }")

if echo "$CLAIM_RESPONSE" | jq -e '.transaction' > /dev/null 2>&1; then
  echo "  ✅ Claim transaction generated!"
  echo ""
  echo "$CLAIM_RESPONSE" | jq '{
    proof: {
      index: .proof.index,
      amount: .proof.amount,
      id: .proof.id
    },
    blockhash: .blockhash,
    lastValidBlockHeight: .lastValidBlockHeight
  }'
  echo ""
  echo "📝 Full claim transaction saved to /tmp/claim-tx.json"
  echo "$CLAIM_RESPONSE" | jq '.' > /tmp/claim-tx.json
  echo ""
  echo "🎉 SUCCESS! You can now:"
  echo "  1. Use the Claims UI to sign this transaction"
  echo "  2. Or manually submit via Solana CLI/SDK"
  echo ""
else
  echo "  ❌ Claim transaction failed:"
  echo "$CLAIM_RESPONSE" | jq '.'
fi
