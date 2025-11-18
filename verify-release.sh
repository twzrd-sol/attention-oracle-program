#!/bin/bash
set -e

echo "=== Milo Token v0.2.1-clean Release Verification ==="
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Change to project root
cd "$(dirname "$0")"

echo -e "${YELLOW}[1/4] Running cargo clippy...${NC}"
cd programs/token_2022
cargo clippy --all-features -- -D warnings
echo -e "${GREEN}✓ Clippy passed${NC}\n"

echo -e "${YELLOW}[2/4] Running anchor test...${NC}"
cd ../..
anchor test
echo -e "${GREEN}✓ Tests passed${NC}\n"

echo -e "${YELLOW}[3/4] Building Solana BPF program...${NC}"
cargo build-sbf
echo -e "${GREEN}✓ BPF build completed${NC}\n"

echo -e "${YELLOW}[4/4] Verifying build...${NC}"
solana-verify build --library-name token_2022
echo -e "${GREEN}✓ Build verification passed${NC}\n"

echo -e "${GREEN}=== All verification checks passed! ===${NC}"
echo ""
echo "Ready to push:"
echo "  git push origin main"
echo "  git push origin v0.2.1-clean --force-with-lease"
