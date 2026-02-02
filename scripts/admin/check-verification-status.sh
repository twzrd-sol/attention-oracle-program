#!/bin/bash
#
# Quick check of program verification status
#
# Usage: ./scripts/admin/check-verification-status.sh

AO_PROGRAM="GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
VAULT_PROGRAM="5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ"

echo ""
echo "╔════════════════════════════════════════════════════════╗"
echo "║  TWZRD Program Verification Status                     ║"
echo "╚════════════════════════════════════════════════════════╝"
echo ""

echo "Checking Attention Oracle..."
AO_STATUS=$(curl -s "https://verify.osec.io/status/$AO_PROGRAM")
AO_VERIFIED=$(echo "$AO_STATUS" | jq -r '.is_verified')

if [[ "$AO_VERIFIED" == "true" ]]; then
  echo "  ✅ Verified"
  echo "$AO_STATUS" | jq '{commit, last_verified_at, repo_url}'
else
  echo "  ❌ Not verified"
  echo "$AO_STATUS" | jq '{message}'
fi

echo ""
echo "Checking ChannelVault..."
VAULT_STATUS=$(curl -s "https://verify.osec.io/status/$VAULT_PROGRAM")
VAULT_VERIFIED=$(echo "$VAULT_STATUS" | jq -r '.is_verified')

if [[ "$VAULT_VERIFIED" == "true" ]]; then
  echo "  ✅ Verified"
  echo "$VAULT_STATUS" | jq '{commit, last_verified_at, repo_url}'
else
  echo "  ❌ Not verified"
  echo "$VAULT_STATUS" | jq '{message}'
fi

echo ""
echo "Checking security.txt..."
if [[ -f ".well-known/security.txt" ]]; then
  echo "  ✅ Exists at .well-known/security.txt"
else
  echo "  ❌ Not found"
fi

echo ""
echo "Repository: https://github.com/twzrd-sol/attention-oracle-program"
echo ""
echo "To verify programs:"
echo "  1. Run: ./scripts/admin/verify-programs.sh"
echo "  2. Submit to: https://verify.osec.io/"
echo "  3. See: docs/PROGRAM_VERIFICATION.md"
echo ""
