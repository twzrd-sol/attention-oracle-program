#!/usr/bin/env bash
set -euo pipefail

if [[ -f .env ]]; then
  echo ".env already exists; nothing to do."
  exit 0
fi

if [[ -f .env.example ]]; then
  cp .env.example .env
  echo "Created .env from .env.example. Edit it to fill values."
else
  echo "No .env.example found in repo root."
  exit 1
fi

