#!/usr/bin/env bash
set -euo pipefail

# Wrapper for local on-chain dump + optional Verified Builds check
# Usage:
#   PROGRAM_ID=... REPO_URL=... GIT_REF=... RPC_URL=https://api.mainnet-beta.solana.com \
#   scripts/verify-onchain.sh [--ellipsis]

PROGRAM_ID="${PROGRAM_ID:-}"
REPO_URL="${REPO_URL:-}"
GIT_REF="${GIT_REF:-}"
RPC_URL="${RPC_URL:-https://api.mainnet-beta.solana.com}"

if [[ -z "${PROGRAM_ID}" ]]; then
  echo "ERROR: PROGRAM_ID is required" >&2
  exit 2
fi

BIN_LOCAL="clean-hackathon/target/deploy/token_2022.so"
BIN_ONCHAIN="/tmp/${PROGRAM_ID}_onchain.so"

echo "[1/3] Dumping on-chain program…"
solana program dump "${PROGRAM_ID}" "${BIN_ONCHAIN}" -u "${RPC_URL}"

echo "[2/3] Comparing SHA256 hashes…"
sha256sum "${BIN_ONCHAIN}" || true
if [[ -f "${BIN_LOCAL}" ]]; then
  sha256sum "${BIN_LOCAL}" || true
else
  echo "Local binary not found at ${BIN_LOCAL}; build with: (cd clean-hackathon/programs/token-2022 && cargo build-sbf)" >&2
fi

if [[ "${1:-}" == "--ellipsis" ]]; then
  if ! command -v solana-verify >/dev/null 2>&1; then
    echo "Installing solana-verify…"
    cargo install --git https://github.com/Ellipsis-Labs/solana-verifiable-build solana-verify --locked
  fi
  if [[ -z "${REPO_URL}" || -z "${GIT_REF}" ]]; then
    echo "ERROR: REPO_URL and GIT_REF are required for --ellipsis mode" >&2
    exit 3
  fi
  echo "[3/3] Running verified build from repo…"
  solana-verify verify-from-repo \
    --program-id "${PROGRAM_ID}" \
    --url "${RPC_URL}" \
    "${REPO_URL}" \
    --tag "${GIT_REF}"
fi

echo "Done."

