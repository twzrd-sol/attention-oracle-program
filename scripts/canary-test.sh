#!/usr/bin/env bash
# Canary test for mainnet deployments
# Verifies basic program functionality post-deploy
set -euo pipefail

RPC=${SYNDICA_RPC:-https://api.mainnet-beta.solana.com}
PROGRAM_ID=${1:-GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop}
CCM_MINT="CCMxTq9GiRkznHxvmxAU18yoHV3AXLQ9D7gt4NMCLWBL"
HOOK_PROGRAM="8VE2Wt5JNusGUCwkrpVmdwgdgyD6vYPEHop2g2CAArzS"

echo "=== Canary Tests (Mainnet) ==="
echo "RPC: $RPC"
echo "Program: $PROGRAM_ID"
echo ""

# 1. Program deployed and executable
echo -n "[1/4] Program executable... "
RESULT=$(solana program show "$PROGRAM_ID" -u "$RPC" 2>&1 || echo "ERROR")
if echo "$RESULT" | grep -q "Program Id"; then
  echo "OK"
else
  echo "FAIL: Program not found"
  exit 1
fi

# 2. Protocol state PDA exists
echo -n "[2/4] Protocol state PDA... "
PROTOCOL_PDA=$(solana-keygen find-program-address --program-id "$PROGRAM_ID" protocol "$CCM_MINT" 2>/dev/null | head -1 || echo "")
if [[ -n "$PROTOCOL_PDA" ]]; then
  ACCT=$(solana account "$PROTOCOL_PDA" -u "$RPC" 2>&1 || echo "")
  if echo "$ACCT" | grep -q "lamports"; then
    echo "OK ($PROTOCOL_PDA)"
  else
    echo "WARN: PDA not initialized"
  fi
else
  echo "SKIP: Could not derive PDA"
fi

# 3. Hook program active
echo -n "[3/4] Transfer hook... "
HOOK_RESULT=$(solana program show "$HOOK_PROGRAM" -u "$RPC" 2>&1 || echo "ERROR")
if echo "$HOOK_RESULT" | grep -q "Program Id"; then
  echo "OK ($HOOK_PROGRAM)"
else
  echo "WARN: Hook program not found"
fi

# 4. Recent transactions
echo -n "[4/4] Recent activity... "
SIGS=$(solana transaction-history "$PROGRAM_ID" -u "$RPC" --limit 5 2>&1 || echo "none")
if echo "$SIGS" | grep -qE "^[A-Za-z0-9]{88}"; then
  COUNT=$(echo "$SIGS" | grep -cE "^[A-Za-z0-9]{88}" || echo "0")
  echo "OK ($COUNT recent txs)"
else
  echo "OK (no recent txs)"
fi

echo ""
echo "=== Canary tests complete ==="
