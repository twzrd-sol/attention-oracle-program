#!/usr/bin/env bash
set -euo pipefail

# Guard against accidental mainnet deployments via `anchor test`.
# Set ALLOW_MAINNET_ANCHOR_TEST=1 to override explicitly.

ALLOW_MAINNET_ANCHOR_TEST="${ALLOW_MAINNET_ANCHOR_TEST:-}"
ANCHOR_PROVIDER_URL="${ANCHOR_PROVIDER_URL:-}"

cluster=""
if [[ -n "${ANCHOR_PROVIDER_URL}" ]]; then
  cluster="${ANCHOR_PROVIDER_URL}"
else
  if [[ -f "Anchor.toml" ]]; then
    cluster="$(rg -n '^[[:space:]]*cluster[[:space:]]*=' Anchor.toml | head -n1 | sed -E 's/.*=[[:space:]]*\"([^\"]+)\".*/\\1/')"
  fi
fi

is_mainnet=false
if [[ "${cluster}" == "mainnet" || "${cluster}" == "mainnet-beta" ]]; then
  is_mainnet=true
elif [[ "${cluster}" == *"mainnet"* ]]; then
  is_mainnet=true
fi

if [[ "${is_mainnet}" == "true" && "${ALLOW_MAINNET_ANCHOR_TEST}" != "1" ]]; then
  echo "Refusing to run 'anchor test' against mainnet."
  echo "Set ALLOW_MAINNET_ANCHOR_TEST=1 to override."
  exit 1
fi

exec anchor test "$@"
