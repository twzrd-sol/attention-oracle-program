#!/usr/bin/env bash
# Load env vars and run command
set -a
source /home/twzrd/milo-token/.env 2>/dev/null || true
set +a
exec "$@"
