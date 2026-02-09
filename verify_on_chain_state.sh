#!/bin/bash

echo "=== FIRST TRUTHS VERIFICATION SCRIPT ==="
echo "Date: $(date)"
echo ""

# Check if solana CLI is available
if ! command -v solana &> /dev/null; then
    echo "❌ Solana CLI not found. Install: https://docs.solanalabs.com/cli/install"
    exit 1
fi

echo "✅ Solana CLI found: $(solana --version)"
echo ""

# Set cluster
export CLUSTER="https://api.mainnet-beta.solana.com"
echo "🌐 Cluster: mainnet-beta"
echo ""

# Program IDs from Anchor.toml and docs
AO_PROGRAM="GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
VAULT_PROGRAM="5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ"

echo "=== 1. VERIFY ATTENTION ORACLE (token_2022) ==="
echo "Program ID: $AO_PROGRAM"
echo ""
solana program show $AO_PROGRAM --url $CLUSTER | grep -E "(Program Id|Owner|ProgramData Address|Upgrade Authority|Last Deployed In Slot|Data Length)"
echo ""

echo "=== 2. VERIFY CHANNEL VAULT ==="
echo "Program ID: $VAULT_PROGRAM"
echo ""
solana program show $VAULT_PROGRAM --url $CLUSTER | grep -E "(Program Id|Owner|ProgramData Address|Upgrade Authority|Last Deployed In Slot|Data Length)"
echo ""

echo "=== 3. UPGRADE AUTHORITY ANALYSIS ==="
echo ""
echo "Expected per DEPLOYMENTS.md:"
echo "  - AO: 2v9jrkuJM99uf4xDFwfyxuzoNmqfggqbuW34mad2n6kW (Squads V4 vault PDA)"
echo "  - Vault: 2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD (Single signer)"
echo ""
echo "Expected per UPGRADE_AUTHORITY.md (Feb 5, 2026):"
echo "  - Both: 2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD (Operational keypair)"
echo ""
echo "Expected per SECURITY_AUDIT.md (Feb 8, 2026):"
echo "  - Both: Squads V4 multisig 3-of-5"
echo ""

AO_AUTHORITY=$(solana program show $AO_PROGRAM --url $CLUSTER 2>/dev/null | grep "Upgrade Authority" | awk '{print $3}')
VAULT_AUTHORITY=$(solana program show $VAULT_PROGRAM --url $CLUSTER 2>/dev/null | grep "Upgrade Authority" | awk '{print $3}')

echo "🔍 ON-CHAIN REALITY:"
echo "  - AO Upgrade Authority: $AO_AUTHORITY"
echo "  - Vault Upgrade Authority: $VAULT_AUTHORITY"
echo ""

# Check if they match documented values
SINGLE_SIGNER="2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD"
SQUADS_VAULT="2v9jrkuJM99uf4xDFwfyxuzoNmqfggqbuW34mad2n6kW"

if [ "$AO_AUTHORITY" == "$SINGLE_SIGNER" ]; then
    echo "⚠️  AO: SINGLE SIGNER (matches UPGRADE_AUTHORITY.md)"
elif [ "$AO_AUTHORITY" == "$SQUADS_VAULT" ]; then
    echo "✅ AO: SQUADS MULTISIG (matches DEPLOYMENTS.md)"
else
    echo "❓ AO: UNKNOWN AUTHORITY (doesn't match any documented value)"
fi

if [ "$VAULT_AUTHORITY" == "$SINGLE_SIGNER" ]; then
    echo "⚠️  Vault: SINGLE SIGNER (matches all docs)"
elif [ "$VAULT_AUTHORITY" == "$SQUADS_VAULT" ]; then
    echo "✅ Vault: SQUADS MULTISIG (unexpected)"
else
    echo "❓ Vault: UNKNOWN AUTHORITY (doesn't match any documented value)"
fi

echo ""
echo "=== 4. DEPLOYMENT SLOT VERIFICATION ==="
echo ""
AO_SLOT=$(solana program show $AO_PROGRAM --url $CLUSTER 2>/dev/null | grep "Last Deployed In Slot" | awk '{print $5}')
VAULT_SLOT=$(solana program show $VAULT_PROGRAM --url $CLUSTER 2>/dev/null | grep "Last Deployed In Slot" | awk '{print $5}')

echo "AO Last Deployed Slot: $AO_SLOT"
echo "Expected (DEPLOYMENTS.md): 398,836,086 (Feb 8, 2026)"
echo ""
echo "Vault Last Deployed Slot: $VAULT_SLOT"
echo "Expected (DEPLOYMENTS.md): 398,835,029 (Feb 8, 2026)"
echo ""

echo "=== VERIFICATION COMPLETE ==="
echo ""
echo "📋 NEXT STEPS:"
echo "1. Update documentation to reflect on-chain reality"
echo "2. If single-signer, transfer to multisig immediately"
echo "3. Verify deployed bytecode matches source (see VERIFY.md)"
