#!/bin/bash
# TWZRD Publisher Quick Start
# Run this to deploy the publisher after verifying infrastructure

set -e

echo "🚀 TWZRD Publisher Quick Start"
echo "================================"
echo ""

# Step 1: Verify infrastructure
echo "Step 1: Running pre-flight checks..."
/home/twzrd/milo-token/scripts/verify-publisher-ready.sh

echo ""
read -p "Continue with deployment? (y/n) " -n 1 -r
echo ""

if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Deployment cancelled."
    exit 0
fi

# Step 2: Check publisher script exists
echo "Step 2: Checking publisher script..."
if [[ ! -f "scripts/publisher/publish-cls-category.ts" ]]; then
    echo "❌ Publisher script not found!"
    exit 1
fi
echo "✅ Publisher script found"

# Step 3: Verify environment variables
echo "Step 3: Verifying environment variables..."
MISSING=0

if [[ -z "$DATABASE_URL" ]]; then
    echo "⚠️  DATABASE_URL not set (will use default)"
    export DATABASE_URL="postgresql://twzrd:twzrd_password_2025@localhost:5432/twzrd"
fi

if [[ -z "$SOLANA_RPC_URL" ]]; then
    echo "⚠️  SOLANA_RPC_URL not set (will use default)"
    export SOLANA_RPC_URL="https://api.mainnet-beta.solana.com"
fi

# Step 4: Test publisher (dry run)
echo ""
read -p "Run publisher test (dry run)? (y/n) " -n 1 -r
echo ""

if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Running publisher test..."
    npx tsx scripts/publisher/publish-cls-category.ts || {
        echo "❌ Publisher test failed. Check logs above."
        exit 1
    }
    echo "✅ Publisher test successful"
fi

# Step 5: Deploy with PM2
echo ""
read -p "Deploy publisher to PM2? (y/n) " -n 1 -r
echo ""

if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Deploying publisher with PM2..."

    # Stop existing publisher if running
    pm2 delete publisher 2>/dev/null || true

    # Start publisher with cron schedule (every 4 hours)
    pm2 start scripts/publisher/publish-cls-category.ts \
      --name publisher \
      --interpreter npx \
      --interpreter-args "tsx" \
      --cron-restart="0 */4 * * *" \
      --no-autorestart

    pm2 save

    echo ""
    echo "✅ Publisher deployed!"
    echo ""
    echo "Monitor with: pm2 logs publisher"
    echo "Check status: pm2 status publisher"
    echo "Trigger manually: pm2 restart publisher"
    echo ""

    # Show current backlog
    echo "Current backlog:"
    curl -s http://localhost:8080/metrics | jq '{backlog_count, last_sealed_epoch}'
fi

echo ""
echo "🎉 Deployment complete!"
echo ""
echo "Next steps:"
echo "  1. Watch logs: pm2 logs publisher --lines 100"
echo "  2. Monitor backlog: curl -s http://localhost:8080/metrics | jq .backlog_count"
echo "  3. Verify on-chain: Check Solana Explorer for published transactions"
echo ""
