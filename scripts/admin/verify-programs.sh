#!/bin/bash
#
# Submit TWZRD programs for verification on OtterSec
#
# Prerequisites:
#   - Programs deployed to mainnet
#   - Source code pushed to public GitHub repo
#   - Verifiable build completed
#
# Usage:
#   ./scripts/admin/verify-programs.sh

set -e

REPO_URL="https://github.com/twzrd-sol/attention-oracle-program"
AO_PROGRAM="GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
VAULT_PROGRAM="5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ"

echo ""
echo "╔════════════════════════════════════════════════════════╗"
echo "║  TWZRD Program Verification                            ║"
echo "╚════════════════════════════════════════════════════════╝"
echo ""

# Check if git is clean
if [[ -n $(git status -s) ]]; then
  echo "⚠️  WARNING: Working directory has uncommitted changes"
  echo "   Verification must be against a committed revision"
  echo ""
  read -p "Continue anyway? (y/N) " -n 1 -r
  echo
  if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    exit 1
  fi
fi

COMMIT_HASH=$(git rev-parse HEAD)
echo "📍 Current commit: $COMMIT_HASH"
echo "📦 Repository: $REPO_URL"
echo ""

# Build verifiable binaries
echo "🔨 Building verifiable programs..."
echo ""

if ! command -v anchor &> /dev/null; then
  echo "❌ ERROR: Anchor CLI not found"
  echo "   Install: cargo install --git https://github.com/coral-xyz/anchor anchor-cli --locked"
  exit 1
fi

# Anchor verifiable build
anchor build --verifiable

echo ""
echo "✅ Verifiable build complete"
echo ""

# Get on-chain hashes
echo "🔍 Fetching on-chain program hashes..."
echo ""

if ! command -v solana-verify &> /dev/null; then
  echo "⚠️  solana-verify not found, installing..."
  cargo install solana-verify
fi

AO_ONCHAIN_HASH=$(solana-verify get-program-hash -u https://api.mainnet-beta.solana.com $AO_PROGRAM 2>/dev/null || echo "ERROR")
VAULT_ONCHAIN_HASH=$(solana-verify get-program-hash -u https://api.mainnet-beta.solana.com $VAULT_PROGRAM 2>/dev/null || echo "ERROR")

echo "Attention Oracle on-chain hash: $AO_ONCHAIN_HASH"
echo "ChannelVault on-chain hash:     $VAULT_ONCHAIN_HASH"
echo ""

# Get executable hashes
echo "🔍 Computing verifiable build hashes..."
echo ""

AO_EXECUTABLE_HASH=$(solana-verify get-executable-hash target/verifiable/token_2022.so 2>/dev/null || echo "ERROR")
VAULT_EXECUTABLE_HASH=$(solana-verify get-executable-hash target/verifiable/channel_vault.so 2>/dev/null || echo "ERROR")

echo "Attention Oracle build hash:    $AO_EXECUTABLE_HASH"
echo "ChannelVault build hash:        $VAULT_EXECUTABLE_HASH"
echo ""

# Compare hashes
echo "─────────────────────────────────────────────────────────"
echo ""

AO_MATCH="❌"
VAULT_MATCH="❌"

if [[ "$AO_ONCHAIN_HASH" == "$AO_EXECUTABLE_HASH" ]]; then
  AO_MATCH="✅"
fi

if [[ "$VAULT_ONCHAIN_HASH" == "$VAULT_EXECUTABLE_HASH" ]]; then
  VAULT_MATCH="✅"
fi

echo "Attention Oracle:  $AO_MATCH"
echo "ChannelVault:      $VAULT_MATCH"
echo ""

if [[ "$AO_MATCH" == "❌" ]] || [[ "$VAULT_MATCH" == "❌" ]]; then
  echo "⚠️  WARNING: Hash mismatch detected!"
  echo ""
  echo "This means the on-chain program does NOT match the current source."
  echo "Possible causes:"
  echo "  - Program was modified after deployment"
  echo "  - Building from wrong commit"
  echo "  - Build environment differences (Rust version, deps)"
  echo ""
  echo "To fix:"
  echo "  1. Check out the exact commit that was deployed"
  echo "  2. Use identical build environment (Docker recommended)"
  echo "  3. Or redeploy with current build"
  echo ""
  read -p "Submit for verification anyway? (y/N) " -n 1 -r
  echo
  if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    exit 1
  fi
fi

echo "─────────────────────────────────────────────────────────"
echo ""
echo "📤 Submitting to OtterSec Verified Builds..."
echo ""

# OtterSec submission
# Visit: https://verify.osec.io/
# Manual steps (no public API for submission yet):

echo "Manual submission required:"
echo ""
echo "1. Go to: https://verify.osec.io/"
echo "2. Click 'Submit Program'"
echo "3. Enter details:"
echo ""
echo "   Program ID:  $AO_PROGRAM"
echo "   Repository:  $REPO_URL"
echo "   Commit:      $COMMIT_HASH"
echo "   Build cmd:   anchor build --verifiable"
echo "   Binary path: target/verifiable/token_2022.so"
echo ""
echo "4. Repeat for ChannelVault:"
echo ""
echo "   Program ID:  $VAULT_PROGRAM"
echo "   Repository:  $REPO_URL"
echo "   Commit:      $COMMIT_HASH"
echo "   Build cmd:   anchor build --verifiable"
echo "   Binary path: target/verifiable/channel_vault.so"
echo ""
echo "5. OtterSec will rebuild and verify (takes ~15 mins)"
echo ""
echo "After verification, check status:"
echo "  curl https://verify.osec.io/status/$AO_PROGRAM"
echo "  curl https://verify.osec.io/status/$VAULT_PROGRAM"
echo ""
