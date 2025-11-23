#!/usr/bin/env bash
set -euo pipefail

# Publish on-chain verification data (no remote build required).
# - Uses local Anchor verifiable build
# - Writes repo URL + commit + library name to the on-chain verification PDA
# - Signs with your default Solana keypair (~/.config/solana/id.json)

PROGRAM_ID="GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
REPO_URL="https://github.com/twzrd-sol/attention-oracle-program"
LIB_NAME="token_2022"
MOUNT_PATH="."

cd "$(dirname "$0")/.."

echo "🛠️  Building verifiable artifact..."
anchor build --verifiable

COMMIT=$(git rev-parse HEAD)
echo "🔗 Repo: $REPO_URL@${COMMIT} (lib=$LIB_NAME, mount=$MOUNT_PATH)"

# Run solana-verify in an Ubuntu 24.04 container with the local AVM binary mounted
# This answers 'y' automatically to upload the verification params on-chain

docker run --rm \
  -e HOME=/root \
  -e PROGRAM_ID="$PROGRAM_ID" \
  -e REPO_URL="$REPO_URL" \
  -e COMMIT="$COMMIT" \
  -e LIB_NAME="$LIB_NAME" \
  -e MOUNT_PATH="$MOUNT_PATH" \
  -v "${PWD}":/work \
  -v "$HOME/.avm/bin":/avm \
  -v "$HOME/.config/solana":/root/.config/solana \
  -w /work ubuntu:24.04 bash -lc '
    apt-get update >/dev/null && apt-get install -y ca-certificates git expect >/dev/null && \
    expect -c " \
      log_user 1; \
      spawn /avm/solana-verify verify-from-repo --url https://api.mainnet-beta.solana.com --program-id $env(PROGRAM_ID) $env(REPO_URL) --commit-hash $env(COMMIT) --library-name $env(LIB_NAME) --mount-path $env(MOUNT_PATH); \
      expect -re {Do you want to upload.*\(y/n\)} { send \"y\\r\" }; \
      expect eof; \
    "
  '

echo "✅ On-chain verification published (local build, no remote)."
