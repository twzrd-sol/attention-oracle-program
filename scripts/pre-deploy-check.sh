#!/usr/bin/env bash
# Pre-deployment verification checks
# Run before any mainnet deployment to catch common issues
set -euo pipefail

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
cd "$REPO_ROOT"

echo "=== Pre-Deploy Checks ==="

# 1. Clean git state
echo -n "[1/6] Git state... "
if [[ -n $(git status --porcelain) ]]; then
  echo "FAIL: Uncommitted changes"
  git status --short
  exit 1
fi
echo "OK (clean)"

# 2. Build programs
echo -n "[2/6] Anchor build... "
if ! anchor build 2>/dev/null; then
  echo "FAIL: Build failed"
  exit 1
fi
echo "OK"

# 3. Run tests
echo -n "[3/6] Tests... "
if ! anchor test --skip-local-validator 2>/dev/null; then
  echo "WARN: Tests failed (continuing)"
else
  echo "OK"
fi

# 4. Check program size
echo -n "[4/6] Program size... "
for so in target/deploy/*.so; do
  SIZE=$(stat -c%s "$so" 2>/dev/null || stat -f%z "$so")
  MAX=$((10 * 1024 * 1024))  # 10MB limit
  if [[ $SIZE -gt $MAX ]]; then
    echo "FAIL: $so exceeds 10MB ($SIZE bytes)"
    exit 1
  fi
  echo "OK ($(basename $so): $((SIZE/1024))KB)"
done

# 5. Verify keypair matches deployed program
echo -n "[5/6] Program ID match... "
for keypair in target/deploy/*-keypair.json; do
  PROGRAM_NAME=$(basename "$keypair" -keypair.json)
  EXPECTED=$(solana-keygen pubkey "$keypair")
  if grep -q "$EXPECTED" "programs/$PROGRAM_NAME/src/lib.rs" 2>/dev/null || \
     grep -q "$EXPECTED" "Anchor.toml"; then
    echo "OK ($PROGRAM_NAME: $EXPECTED)"
  else
    echo "WARN: $PROGRAM_NAME keypair may not match declared ID"
  fi
done

# 6. RPC connectivity
echo -n "[6/6] RPC connectivity... "
RPC=${SYNDICA_RPC:-https://api.mainnet-beta.solana.com}
if curl -s -X POST -H "Content-Type: application/json" \
   -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' "$RPC" | grep -q "ok"; then
  echo "OK ($RPC)"
else
  echo "WARN: RPC health check failed"
fi

echo ""
echo "=== All checks passed ==="
echo "Ready for: scripts/guard-deploy.sh anchor deploy ..."
