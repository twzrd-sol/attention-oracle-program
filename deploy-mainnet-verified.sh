#!/bin/bash

# Deployment script for verified build
# Program: GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
# Date: November 15, 2025

set -e

echo "=========================================="
echo "ATTENTION ORACLE MAINNET DEPLOYMENT"
echo "=========================================="
echo ""
echo "Program ID: GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
echo "Binary: /home/twzrd/milo-token/programs/attention-oracle/target/deploy/token_2022.so"
echo "Size: $(ls -lh /home/twzrd/milo-token/programs/attention-oracle/target/deploy/token_2022.so | awk '{print $5}')"
echo "Authority: $(solana address)"
echo ""

# Check balance
BALANCE=$(solana balance --url mainnet-beta | awk '{print $1}')
echo "Current balance: $BALANCE SOL"
echo ""

# Estimate deployment cost
echo "Estimating deployment cost..."
COST=$(solana program deploy /home/twzrd/milo-token/programs/attention-oracle/target/deploy/token_2022.so \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --url mainnet-beta \
  --dry-run 2>&1 | grep -i "rent" || echo "~0.05 SOL")

echo "Estimated cost: $COST"
echo ""

read -p "Do you want to proceed with deployment? (yes/no): " CONFIRM

if [ "$CONFIRM" != "yes" ]; then
    echo "Deployment cancelled"
    exit 1
fi

echo ""
echo "Deploying..."
echo ""

# Deploy with verbose output
solana program deploy /home/twzrd/milo-token/programs/attention-oracle/target/deploy/token_2022.so \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --url mainnet-beta \
  --verbose

echo ""
echo "=========================================="
echo "DEPLOYMENT COMPLETE"
echo "=========================================="
echo ""
echo "Next steps:"
echo "1. Verify on Solscan: https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
echo "2. Run solana-verify to get verification badge"
echo "3. Upload verification data to OSEC"
echo ""