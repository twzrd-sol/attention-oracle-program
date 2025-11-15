#!/bin/bash

# Verification script for Attention Oracle on mainnet
# This gets us the green checkmark on Solana Explorer

set -e

echo "=========================================="
echo "SOLANA PROGRAM VERIFICATION"
echo "=========================================="
echo ""
echo "Program: GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
echo "Repository: https://github.com/twzrd-sol/attention-oracle-program"
echo ""

# Step 1: Get the on-chain executable hash
echo "Step 1: Getting on-chain program hash..."
ON_CHAIN_HASH=$(solana program dump GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop /tmp/onchain.so --url mainnet-beta 2>/dev/null && sha256sum /tmp/onchain.so | cut -d' ' -f1)
echo "On-chain hash: $ON_CHAIN_HASH"
echo ""

# Step 2: Try to build with cargo build-sbf directly (since solana-verify has issues)
echo "Step 2: Building program locally..."
cd /home/twzrd/milo-token/programs/attention-oracle
cargo build-sbf --sbf-out-dir ./target/verify 2>&1 | tail -5
LOCAL_HASH=$(sha256sum ./target/verify/token_2022.so | cut -d' ' -f1)
echo "Local build hash: $LOCAL_HASH"
echo ""

# Step 3: Check if hashes match
if [ "$ON_CHAIN_HASH" = "$LOCAL_HASH" ]; then
    echo "✅ Hashes match! Program is verifiable."
else
    echo "⚠️  Hashes don't match. This is expected if the on-chain version was built differently."
fi
echo ""

# Step 4: Use solana-verify to verify against the deployed program
echo "Step 4: Running solana-verify..."
echo ""
echo "Option A: Verify with remote build (recommended)"
echo "This will build in the Ellipsis Labs container and verify against mainnet"
echo ""
echo "Command to run:"
echo "solana-verify verify-from-repo \\"
echo "  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \\"
echo "  --remote https://github.com/twzrd-sol/attention-oracle-program \\"
echo "  --mount-path programs/attention-oracle \\"
echo "  --library-name token_2022 \\"
echo "  --commit-hash \$(git rev-parse HEAD)"
echo ""
echo "Option B: Get OSEC verification"
echo "This submits to the on-chain registry for the green checkmark"
echo ""
echo "1. First, verify the build:"
echo "   solana-verify get-program-hash GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
echo ""
echo "2. Then submit for verification:"
echo "   solana-verify verify-from-repo \\"
echo "     --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \\"
echo "     --remote https://github.com/twzrd-sol/attention-oracle-program \\"
echo "     --mount-path programs/attention-oracle \\"
echo "     --library-name token_2022"
echo ""
echo "3. If successful, upload proof:"
echo "   Look for 'Verification successful' and follow the prompts to upload"
echo ""

# Step 5: Check current verification status
echo "Step 5: Checking current verification status..."
solana-verify get-program-authority GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop 2>/dev/null || echo "Not yet verified with OSEC"
echo ""

echo "=========================================="
echo "NEXT STEPS FOR GREEN CHECKMARK:"
echo "=========================================="
echo ""
echo "1. Push latest code to GitHub:"
echo "   git add . && git commit -m 'Prepare for verification' && git push"
echo ""
echo "2. Run the verification command from Option B above"
echo ""
echo "3. When prompted, sign the transaction to upload verification data"
echo ""
echo "4. Check Solana Explorer in ~5 minutes for the green checkmark"
echo "   https://explorer.solana.com/address/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
echo ""