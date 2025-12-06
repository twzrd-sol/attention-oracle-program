#!/bin/bash
set -e

echo "=== Deploying Helius Webhook Receiver ==="
echo ""

# Check if wrangler is installed
if ! command -v wrangler &> /dev/null; then
    echo "❌ Wrangler not found. Installing..."
    npm install -g wrangler
fi

# Check if logged in
echo "Checking Cloudflare authentication..."
if ! wrangler whoami &> /dev/null; then
    echo "❌ Not logged in. Running 'wrangler login'..."
    wrangler login
fi

echo "✅ Authenticated"
echo ""

# Install dependencies
echo "Installing dependencies..."
npm install
echo ""

# Check if KV namespace exists in wrangler.toml
if grep -q "REPLACE_WITH_YOUR_KV_ID" wrangler.toml; then
    echo "Creating KV namespace..."
    KV_OUTPUT=$(wrangler kv namespace create PUMP_DATA)

    # Extract ID from output
    KV_ID=$(echo "$KV_OUTPUT" | grep -oP 'id = "\K[^"]+')

    if [ -z "$KV_ID" ]; then
        echo "❌ Failed to extract KV namespace ID. Please create manually:"
        echo "   wrangler kv namespace create PUMP_DATA"
        exit 1
    fi

    echo "✅ KV namespace created: $KV_ID"

    # Update wrangler.toml
    sed -i "s/REPLACE_WITH_YOUR_KV_ID/$KV_ID/" wrangler.toml
    echo "✅ Updated wrangler.toml with KV namespace ID"
    echo ""
fi

# Check if WEBHOOK_SECRET is set
echo "Checking for WEBHOOK_SECRET..."
if ! wrangler secret list 2>/dev/null | grep -q "WEBHOOK_SECRET"; then
    echo "⚠️  WEBHOOK_SECRET not set."
    echo ""
    echo "Generating random secret..."
    SECRET=$(openssl rand -hex 32)
    echo "Your webhook secret: $SECRET"
    echo ""
    echo "Setting secret in Cloudflare..."
    echo "$SECRET" | wrangler secret put WEBHOOK_SECRET
    echo "✅ WEBHOOK_SECRET set"
    echo ""
    echo "⚠️  SAVE THIS SECRET - You'll need it for Helius webhook:"
    echo "   $SECRET"
    echo ""
else
    echo "✅ WEBHOOK_SECRET already set"
    echo ""
fi

# Deploy
echo "Deploying worker..."
wrangler deploy

echo ""
echo "=== Deployment Complete ==="
echo ""
echo "Your worker is live at:"
WORKER_URL=$(wrangler deployments list --json 2>/dev/null | jq -r '.[0].url' 2>/dev/null || echo "https://twzrd-helius-receiver.YOUR-SUBDOMAIN.workers.dev")
echo "  $WORKER_URL"
echo ""
echo "Next steps:"
echo "1. Test health endpoint:"
echo "   curl $WORKER_URL/health"
echo ""
echo "2. Get current epoch:"
echo "   curl $WORKER_URL/current-epoch"
echo ""
echo "3. Create Helius webhook:"
echo "   export HELIUS_API_KEY='your-api-key'"
echo "   export WEBHOOK_SECRET='your-secret-from-above'"
echo "   curl -X POST \"https://api-mainnet.helius-rpc.com/v0/webhooks?api-key=\$HELIUS_API_KEY\" \\"
echo "     -H 'Content-Type: application/json' \\"
echo "     -d '{"
echo "       \"webhookURL\": \"$WORKER_URL/webhook\","
echo "       \"transactionTypes\": [\"ANY\"],"
echo "       \"accountAddresses\": [\"6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P\"],"
echo "       \"webhookType\": \"enhanced\","
echo "       \"authHeader\": \"x-webhook-secret: '\$WEBHOOK_SECRET'\""
echo "     }'"
echo ""
echo "4. Monitor logs:"
echo "   wrangler tail"
