#!/bin/bash
# Publisher Infrastructure Verification Script
# Run this after main AI completes database refactor

set -e

echo "=================================================="
echo "TWZRD Publisher Infrastructure Verification"
echo "=================================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
DB_URL="postgresql://twzrd:twzrd_password_2025@localhost:5432/twzrd"
API_BASE="http://localhost:8080"

echo "1. Redis Status"
echo "-----------------"
REDIS_VERSION=$(redis-cli --version | awk '{print $2}')
echo -e "${GREEN}✓${NC} Redis version: $REDIS_VERSION (required: 6.2+)"
redis-cli ping > /dev/null 2>&1 && echo -e "${GREEN}✓${NC} Redis connectivity OK" || echo -e "${RED}✗${NC} Redis not responding"
echo ""

echo "2. PostgreSQL Database Stats"
echo "-----------------------------"
psql $DB_URL -c "SELECT COUNT(*) as unpublished_count FROM sealed_epochs WHERE published IS NULL OR published = 0;" -t | xargs echo "Unpublished sealed epochs:"
psql $DB_URL -c "SELECT COUNT(DISTINCT epoch) as sealed_epochs_total FROM sealed_epochs;" -t | xargs echo "Total sealed epochs:"
psql $DB_URL -c "SELECT COUNT(DISTINCT channel) as active_channels FROM sealed_epochs;" -t | xargs echo "Active channels:"
echo ""

echo "3. Latest Sealed Epoch"
echo "----------------------"
psql $DB_URL -c "SELECT epoch, channel, LEFT(root, 16) || '...' as root_preview, sealed_at FROM sealed_epochs ORDER BY epoch DESC, channel LIMIT 5;"
echo ""

echo "4. API Endpoint Tests"
echo "---------------------"

# Test /stats endpoint
echo -n "Testing /stats endpoint... "
if curl -s -f "$API_BASE/stats" > /dev/null 2>&1; then
    echo -e "${GREEN}✓${NC}"
else
    echo -e "${RED}✗${NC}"
fi

# Test /metrics endpoint
echo -n "Testing /metrics endpoint... "
if curl -s -f "$API_BASE/metrics" > /dev/null 2>&1; then
    echo -e "${GREEN}✓${NC}"
else
    echo -e "${RED}✗${NC}"
fi

# Test /claim-root endpoint (critical for publisher)
echo -n "Testing /claim-root endpoint... "
CLAIM_ROOT_RESPONSE=$(curl -s "$API_BASE/claim-root?channel=adapt&epoch=1761825600")
ROOT_VALUE=$(echo $CLAIM_ROOT_RESPONSE | jq -r '.root' 2>/dev/null)

if [[ "$ROOT_VALUE" != "null" ]] && [[ "$ROOT_VALUE" != "0xundefined" ]] && [[ ${#ROOT_VALUE} -eq 66 ]]; then
    echo -e "${GREEN}✓${NC} (root: ${ROOT_VALUE:0:18}...)"
else
    echo -e "${RED}✗${NC} (got: $ROOT_VALUE)"
    echo -e "${YELLOW}⚠${NC}  This endpoint must return valid 32-byte hex root for publisher to work"
fi

echo ""

echo "5. PM2 Process Status"
echo "---------------------"
pm2 list | grep -E "twzrd-aggregator|tree-builder|publisher" || echo "No TWZRD processes found"
echo ""

echo "6. Publisher Readiness Checklist"
echo "---------------------------------"

READY=true

# Check 1: Redis
if redis-cli ping > /dev/null 2>&1; then
    echo -e "${GREEN}✓${NC} Redis running and accessible"
else
    echo -e "${RED}✗${NC} Redis not running"
    READY=false
fi

# Check 2: PostgreSQL
if psql $DB_URL -c "SELECT 1;" > /dev/null 2>&1; then
    echo -e "${GREEN}✓${NC} PostgreSQL accessible"
else
    echo -e "${RED}✗${NC} PostgreSQL not accessible"
    READY=false
fi

# Check 3: Unpublished epochs exist
UNPUBLISHED=$(psql $DB_URL -t -c "SELECT COUNT(*) FROM sealed_epochs WHERE published IS NULL OR published = 0;" | xargs)
if [[ $UNPUBLISHED -gt 0 ]]; then
    echo -e "${GREEN}✓${NC} $UNPUBLISHED sealed epochs ready to publish"
else
    echo -e "${YELLOW}⚠${NC}  No unpublished epochs found"
fi

# Check 4: API server running
if curl -s -f "$API_BASE/stats" > /dev/null 2>&1; then
    echo -e "${GREEN}✓${NC} API server responding"
else
    echo -e "${RED}✗${NC} API server not responding"
    READY=false
fi

# Check 5: /claim-root endpoint fixed
if [[ "$ROOT_VALUE" != "null" ]] && [[ "$ROOT_VALUE" != "0xundefined" ]] && [[ ${#ROOT_VALUE} -eq 66 ]]; then
    echo -e "${GREEN}✓${NC} /claim-root endpoint returning valid roots"
else
    echo -e "${RED}✗${NC} /claim-root endpoint still broken (main AI refactor incomplete)"
    READY=false
fi

echo ""

if $READY; then
    echo -e "${GREEN}=================================================="
    echo "✓ PUBLISHER READY FOR DEPLOYMENT"
    echo "==================================================${NC}"
    echo ""
    echo "Next steps:"
    echo "  1. Deploy tree-builder: pm2 start apps/twzrd-aggregator/src/workers/tree-builder.ts --name tree-builder"
    echo "  2. Deploy publisher: pm2 start scripts/publisher/publish-cls-category.ts --name publisher"
    echo "  3. Monitor logs: pm2 logs publisher"
else
    echo -e "${RED}=================================================="
    echo "✗ PUBLISHER NOT READY"
    echo "==================================================${NC}"
    echo ""
    echo "Blockers:"
    echo "  - Wait for main AI to complete database refactor"
    echo "  - Test /claim-root endpoint returns valid roots"
    echo "  - Re-run this script to verify"
fi

echo ""
