#!/usr/bin/env bash
set -euo pipefail

# Publish on-chain verification data, with optional remote submission.
# - Uploads repo URL + commit + library name to the on-chain verification PDA
# - Uses your default Solana keypair (~/.config/solana/id.json) unless overridden
# - Optionally submits a remote verification job after upload

usage() {
  cat <<USAGE
Usage: $0 [options]

Options:
  --program-id <PUBKEY>        Program ID to verify (default: from env below)
  --repo-url <URL>             Git repo URL (default: repo origin URL or project URL)
  --commit <HASH>              Commit hash to verify (default: git rev-parse HEAD)
  --library-name <NAME>        Cargo lib name (e.g. token_2022)
  --mount-path <PATH>          Path to mount for build context (default: .)
  --rpc-url <URL>              RPC endpoint (default: ${SYNDICA_RPC:-https://api.mainnet-beta.solana.com})
  --wallet <PATH>              Path to keypair JSON (default: ~/.config/solana/id.json)
  --skip-build                 Skip local 'anchor build --verifiable'
  --remote                     Submit a remote verification job after upload
  -h, --help                   Show this help

Examples:
  $0 \
    --program-id G2v5XVA4SZnZ5NVLSC7pHJp9JRWSN13jHoXQ9ebpujvB \
    --library-name token_2022 \
    --mount-path . \
    --remote
USAGE
}

cd "$(dirname "$0")/.."

# Defaults
PROGRAM_ID="GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
REPO_URL="https://github.com/twzrd-sol/attention-oracle-program"
LIB_NAME="token_2022"
MOUNT_PATH="."
RPC_URL="${SYNDICA_RPC:-https://api.mainnet-beta.solana.com}"
WALLET_PATH="~/.config/solana/id.json"
SKIP_BUILD=0
DO_REMOTE=0

# Parse args
while [[ $# -gt 0 ]]; do
  case "$1" in
    --program-id) PROGRAM_ID="$2"; shift 2 ;;
    --repo-url) REPO_URL="$2"; shift 2 ;;
    --commit) COMMIT="$2"; shift 2 ;;
    --library-name) LIB_NAME="$2"; shift 2 ;;
    --mount-path) MOUNT_PATH="$2"; shift 2 ;;
    --rpc-url) RPC_URL="$2"; shift 2 ;;
    --wallet) WALLET_PATH="$2"; shift 2 ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    --remote) DO_REMOTE=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage; exit 1 ;;
  esac
done

# Expand ~ in wallet path
WALLET_PATH="${WALLET_PATH/#\~/$HOME}"

# Commit
COMMIT=${COMMIT:-$(git rev-parse HEAD)}

echo "🔗 Repo: $REPO_URL@${COMMIT}"
echo "🏷️  Program: $PROGRAM_ID | Lib: $LIB_NAME | Mount: $MOUNT_PATH"
echo "🌐 RPC: $RPC_URL"
echo "🔑 Wallet: $WALLET_PATH"

# Optional local verifiable build
if [[ $SKIP_BUILD -eq 0 ]]; then
  echo "🛠️  Building verifiable artifact..."
  anchor build --verifiable
else
  echo "⏭️  Skipping local build (--skip-build)"
fi

# Derive uploader pubkey from the keypair (base58, no external tools required)
UPLOADER=$(python3 - "$WALLET_PATH" <<'PY'
import json, sys

ALPHABET = b'123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'

def b58encode(b: bytes) -> str:
    n = int.from_bytes(b, 'big')
    res = bytearray()
    while n > 0:
        n, r = divmod(n, 58)
        res.append(ALPHABET[r])
    # leading zeros
    pad = 0
    for ch in b:
        if ch == 0:
            pad += 1
        else:
            break
    res.extend(ALPHABET[0:1] * pad)
    return res[::-1].decode()

keyfile = sys.argv[1]
arr = json.load(open(keyfile))
sk = bytes(arr)
pk = sk[32:64]  # last 32 bytes is the public key
print(b58encode(pk))
PY
)

echo "👤 Uploader: $UPLOADER"

# Run solana-verify in an Ubuntu 24.04 container with the local AVM binary mounted
# This answers 'y' automatically to upload the verification params on-chain

docker run --rm \
  -e HOME=/home/twzrd \
  -e PROGRAM_ID="$PROGRAM_ID" \
  -e REPO_URL="$REPO_URL" \
  -e COMMIT="$COMMIT" \
  -e LIB_NAME="$LIB_NAME" \
  -e MOUNT_PATH="$MOUNT_PATH" \
  -e RPC_URL="$RPC_URL" \
  -e KEYPAIR="/root/.config/solana/id.json" \
  -v "${PWD}":/work \
  -v "$HOME/.avm/bin":/avm \
  -v "$HOME/.config/solana":/root/.config/solana \
  -v "$HOME/.config/solana":/home/twzrd/.config/solana \
  -v "/var/run/docker.sock":/var/run/docker.sock \
  -w /work ubuntu:24.04 bash -lc '
    set -euo pipefail
    apt-get update >/dev/null && apt-get install -y ca-certificates git expect docker.io >/dev/null && update-ca-certificates >/dev/null 2>&1 || true
    echo "🚀 Uploading verification data to chain..."
    expect -c " \
      log_user 1; \
      spawn /avm/solana-verify verify-from-repo --url \$env(RPC_URL) --program-id \$env(PROGRAM_ID) \$env(REPO_URL) --commit-hash \$env(COMMIT) --library-name \$env(LIB_NAME) --mount-path \$env(MOUNT_PATH) --skip-build -y -k \$env(KEYPAIR); \
      expect -re {Do you want to upload.*\(y/n\)} { send \"y\\r\" }; \
      expect eof; \
    "
  '

echo "✅ On-chain verification metadata uploaded."

if [[ $DO_REMOTE -eq 1 ]]; then
  echo "📡 Submitting remote verification job..."
  docker run --rm \
    -e HOME=/home/twzrd \
    -e PROGRAM_ID="$PROGRAM_ID" \
    -e UPLOADER="$UPLOADER" \
    -e RPC_URL="$RPC_URL" \
    -v "${PWD}":/work \
    -v "$HOME/.avm/bin":/avm \
    -v "$HOME/.config/solana":/home/twzrd/.config/solana \
    -v "/var/run/docker.sock":/var/run/docker.sock \
    -w /work ubuntu:24.04 bash -lc '
      set -euo pipefail
      /avm/solana-verify remote submit-job --program-id "$PROGRAM_ID" --uploader "$UPLOADER" --url "$RPC_URL"
    '
  echo "✅ Remote verification job submitted."
fi

echo "🏁 Done."
