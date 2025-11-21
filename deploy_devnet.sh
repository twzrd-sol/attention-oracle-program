#!/bin/bash

# Enforcer Devnet Deployment Script
# Automates the process of deploying and testing the Enforcer patch on devnet

set -e  # Exit on error

echo "════════════════════════════════════════════════════════════"
echo "  ENFORCER PATCH - DEVNET DEPLOYMENT"
echo "════════════════════════════════════════════════════════════"
echo ""

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Step 1: Configure Solana CLI for Devnet
echo -e "${YELLOW}[1/5]${NC} Configuring Solana CLI for devnet..."
solana config set --url https://api.devnet.solana.com
echo ""

# Step 2: Check balance
echo -e "${YELLOW}[2/5]${NC} Checking wallet balance..."
WALLET=$(solana address)
BALANCE=$(solana balance)
echo "Wallet: $WALLET"
echo "Balance: $BALANCE"
echo ""

# Airdrop if needed
if [ "$BALANCE" = "0 SOL" ]; then
  echo "Balance is 0. Requesting airdrop..."
  solana airdrop 2
  echo ""
fi

# Step 3: Build program (if not already built)
echo -e "${YELLOW}[3/5]${NC} Building program..."
anchor build
echo -e "${GREEN}✅ Build complete${NC}"
echo ""

# Step 4: Deploy program
echo -e "${YELLOW}[4/5]${NC} Deploying program to devnet..."
echo "Program binary: target/deploy/token_2022.so"
echo "Program size: $(ls -lh target/deploy/token_2022.so | awk '{print $5}')"
echo ""

read -p "Deploy to devnet? (y/n) " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
  anchor deploy --provider.cluster devnet
  echo -e "${GREEN}✅ Deployment complete${NC}"
  echo ""
else
  echo -e "${YELLOW}Skipping deployment${NC}"
  echo ""
fi

# Step 5: Update enforcer config
echo -e "${YELLOW}[5/5]${NC} Updating enforcer configuration..."
echo "Running: ts-node scripts/update_enforcer_devnet.ts"
echo ""

read -p "Update enforcer config on devnet? (y/n) " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
  cd scripts
  ts-node update_enforcer_devnet.ts
  cd ..
  echo ""
  echo -e "${GREEN}✅ Enforcer config updated${NC}"
else
  echo -e "${YELLOW}Skipping enforcer config update${NC}"
fi

echo ""
echo "════════════════════════════════════════════════════════════"
echo -e "${GREEN}  DEVNET DEPLOYMENT COMPLETE${NC}"
echo "════════════════════════════════════════════════════════════"
echo ""
echo "Next steps:"
echo "1. Test VIP transfers (score ≥3000)"
echo "2. Test tourist transfers (score <3000)"
echo "3. Test zero trust (no passport)"
echo "4. Verify tax calculations in transfer events"
echo ""
echo "When ready, deploy to mainnet with:"
echo "  $ solana config set --url https://api.mainnet-beta.solana.com"
echo "  $ anchor deploy"
echo ""
