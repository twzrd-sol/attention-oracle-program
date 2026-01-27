#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
cd "$REPO_ROOT"

RUST_TOOLCHAIN=$(sed -nE 's/^channel = "([^"]+)".*/\1/p' rust-toolchain.toml | head -n1 || true)
SOLANA_VERSION=$(sed -nE 's/^solana_version = "([^"]+)".*/\1/p' Solana.toml | head -n1 || true)
ANCHOR_VERSION=$(sed -nE 's/^anchor_version = "([^"]+)".*/\1/p' Solana.toml | head -n1 || true)
ANCHOR_JS_VERSION=$(sed -nE 's|.*"@coral-xyz/anchor": *"([^"]+)".*|\1|p' package.json | head -n1 || true)

echo "Dev doctor - solana rust"
echo "repo: $REPO_ROOT"
[[ -n "$RUST_TOOLCHAIN" ]] && echo "rust-toolchain: $RUST_TOOLCHAIN"
[[ -n "$SOLANA_VERSION" ]] && echo "solana_version: $SOLANA_VERSION"
[[ -n "$ANCHOR_VERSION" ]] && echo "anchor_version: $ANCHOR_VERSION"
[[ -n "$ANCHOR_JS_VERSION" ]] && echo "anchor_js: $ANCHOR_JS_VERSION"
echo

fail=0

check_version() {
  local label=$1
  local expected=$2
  local cmd=$3
  shift 3
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "[miss] $label ($cmd not found)"
    fail=1
    return 0
  fi
  local got
  got=$("$cmd" "$@" 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -n1 || true)
  if [[ -z "$got" ]]; then
    echo "[warn] $label (unable to parse version)"
    return 0
  fi
  if [[ -n "$expected" && "$got" != "$expected" ]]; then
    echo "[warn] $label $got (expected $expected)"
    fail=1
    return 0
  fi
  echo "[ok] $label $got"
}

check_presence() {
  local label=$1
  local cmd=$2
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "[miss] $label ($cmd not found)"
    fail=1
    return 0
  fi
  echo "[ok] $label"
}

check_version "rustc" "$RUST_TOOLCHAIN" rustc --version
check_version "cargo" "$RUST_TOOLCHAIN" cargo --version
check_version "solana" "$SOLANA_VERSION" solana --version
check_version "anchor" "$ANCHOR_VERSION" anchor --version

check_version "node" "" node --version
if command -v pnpm >/dev/null 2>&1; then
  check_version "pnpm" "" pnpm --version
else
  echo "[warn] pnpm (not found; pnpm-lock.yaml present)"
  fail=1
fi

if [[ -n "$ANCHOR_JS_VERSION" && -n "$ANCHOR_VERSION" ]]; then
  echo
  echo "note: anchor-js is $ANCHOR_JS_VERSION, anchor-cli is $ANCHOR_VERSION"
fi

if [[ $fail -ne 0 ]]; then
  echo
  echo "See docs/runbooks/dev-setup.md for setup notes."
fi

exit $fail
