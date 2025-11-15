#!/usr/bin/env bash
# MIT License
# Simple health check + alert/restart helper for TWZRD services.
# - Checks: gateway (8082), aggregator (8080), worker-v2 (8081), Postgres
# - Logs to: logs/monitor/health.log
# - Optional Slack: export SLACK_WEBHOOK in environment or .env
# - Auto-retry: if a service fails 3 consecutive checks → pm2 restart <service>

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LOG_DIR="$ROOT_DIR/logs/monitor"
STATE_DIR="$ROOT_DIR/scripts/monitor/.state"
ENV_FILE="$ROOT_DIR/.env"
mkdir -p "$LOG_DIR" "$STATE_DIR"
LOG_FILE="$LOG_DIR/health.log"

timestamp() { date -u +"%Y-%m-%dT%H:%M:%SZ"; }

# Load .env (non‑exported secrets remain local to this shell)
if [[ -f "$ENV_FILE" ]]; then
  # shellcheck disable=SC2046
  export $(grep -E '^[A-Z0-9_]+=' "$ENV_FILE" | sed 's/#.*//') || true
fi

VERBOSE=${VERBOSE:-0}

# Map logical service → health URL → pm2 name
declare -A URLS
declare -A PM2
URLS[gateway]="http://127.0.0.1:${PORT:-8082}/health"
PM2[gateway]="gateway"

URLS[aggregator]="http://127.0.0.1:${AGGREGATOR_PORT:-8080}/health"
PM2[aggregator]="milo-aggregator"

URLS[worker]="http://127.0.0.1:${WORKER_PORT:-8081}/health"
PM2[worker]="cls-worker"

# Curl helper
check_http() {
  local name="$1" url="$2"
  local out
  out=$(curl -sS -m 5 -o /dev/null -w 'code=%{http_code} time=%{time_total}' "$url" || echo "code=000 time=0")
  local code time
  code=$(awk '{print $1}' <<<"$out" | cut -d= -f2)
  time=$(awk '{print $2}' <<<"$out" | cut -d= -f2)
  [[ "$VERBOSE" == "1" ]] && echo "[$(timestamp)] $name $url $out" >&2
  if [[ "$code" == "200" ]]; then
    echo OK
  else
    echo FAIL:$code:$time
  fi
}

psql_check() {
  # Prefer DATABASE_URL; fallback to local connection
  local conn="${DATABASE_URL:-}"
  if [[ -n "$conn" ]]; then
    psql "$conn" -At -c 'select 1' >/dev/null 2>&1 && echo OK || echo FAIL
  else
    PGPASSWORD="${PGPASSWORD:-twzrd_password_2025}" psql -h 127.0.0.1 -U "${PGUSER:-twzrd}" -d "${PGDATABASE:-twzrd_oracle}" -At -c 'select 1' >/dev/null 2>&1 && echo OK || echo FAIL
  fi
}

log() { echo "[$(timestamp)] $*" | tee -a "$LOG_FILE" >/dev/null; }

fail_count_file() { echo "$STATE_DIR/${1}.fails"; }
inc_fail() { local f; f=$(fail_count_file "$1"); local n=0; [[ -f "$f" ]] && n=$(cat "$f"); n=$((n+1)); echo "$n" >"$f"; echo "$n"; }
reset_fail() { local f; f=$(fail_count_file "$1"); rm -f "$f"; }

maybe_slack() {
  local text="$1"
  if [[ -n "${SLACK_WEBHOOK:-}" ]]; then
    curl -sS -m 3 -H 'Content-type: application/json' --data "$(jq -nc --arg t "$text" '{text:$t}')" "$SLACK_WEBHOOK" >/dev/null || true
  fi
}

auto_restart() {
  local svc="$1"
  local pm2_name="${PM2[$svc]}"
  if [[ -n "$pm2_name" ]]; then
    log "auto_restart $svc → pm2 restart $pm2_name"
    pm2 restart "$pm2_name" >/dev/null 2>&1 || true
  fi
}

# Support test mode
if [[ "${1:-}" == "--test-alert" ]]; then
  log "TEST alert triggered by operator"
  maybe_slack "[TWZRD Monitor] Test alert at $(timestamp)"
  exit 0
fi

fail_total=0
failed_list=""

# Run HTTP checks
for svc in gateway aggregator worker; do
  status=$(check_http "$svc" "${URLS[$svc]}")
  if [[ "$status" == OK ]]; then
    log "$svc OK ${URLS[$svc]}"
    reset_fail "$svc"
  else
    code=$(cut -d: -f2 <<<"$status"); time=$(cut -d: -f3 <<<"$status")
    log "$svc FAIL code=$code time=$time ${URLS[$svc]}"
    n=$(inc_fail "$svc")
    fail_total=$((fail_total+1))
    if [[ -z "$failed_list" ]]; then failed_list="$svc($n)"; else failed_list="$failed_list $svc($n)"; fi
    if [[ "$n" -ge 3 ]]; then
      auto_restart "$svc"
      reset_fail "$svc"
    fi
  fi
done

# Postgres check
pgs=$(psql_check)
if [[ "$pgs" == OK ]]; then
  log "postgres OK"
else
  log "postgres FAIL"
  maybe_slack "[TWZRD Monitor] Postgres check FAILED at $(timestamp)"
fi

# Optional: certificate expiry check (14-day warning)
cert_path="${SSL_CERT_PATH:-/etc/letsencrypt/live/api.twzrd.xyz/fullchain.pem}"
if [[ -f "$cert_path" ]]; then
  enddate=$(openssl x509 -noout -enddate -in "$cert_path" 2>/dev/null | cut -d= -f2 || true)
  if [[ -n "$enddate" ]]; then
    exp_epoch=$(date -d "$enddate" +%s)
    now_epoch=$(date -u +%s)
    days_left=$(( (exp_epoch - now_epoch) / 86400 ))
    log "cert ${cert_path} expires in ${days_left}d"
    if (( days_left <= 14 )); then
      maybe_slack "[TWZRD Monitor] TLS cert expires in ${days_left} days (${cert_path})"
    fi
  fi
fi

# Optional: publish backlog / silence alert via Postgres heuristics
if [[ -n "${DATABASE_URL:-}" ]]; then
  # Total backlog for visibility
  unpublished_total=$(psql "$DATABASE_URL" -At -c "select count(*) from sealed_epochs where coalesce(published,0)=0" 2>/dev/null || echo "0")
  # MILO-only backlog (eligible for per-channel publishing)
  milo_channels_csv="${MILO_CHANNELS:-}"
  if [[ -n "$milo_channels_csv" ]]; then
    unpublished_milo=$(psql "$DATABASE_URL" -At -c "select count(*) from sealed_epochs where coalesce(published,0)=0 and channel = any(string_to_array('$milo_channels_csv', ','))" 2>/dev/null || echo "0")
  else
    unpublished_milo="$unpublished_total"
  fi
  silence_sec=$(psql "$DATABASE_URL" -At -c "select coalesce(extract(epoch from (now() - max(sealed_at))),999999) from sealed_epochs where coalesce(published,0)=1" 2>/dev/null || echo "999999")
  log "publisher backlog_total=${unpublished_total} backlog_milo=${unpublished_milo} silence=${silence_sec}s"
  # Alert based on MILO eligible backlog (category/CLS handled separately)
  if (( unpublished_milo > 50 && silence_sec > 600 )); then
    maybe_slack "[TWZRD Monitor] Publisher backlog=${unpublished_milo} (MILO-only) and silence>${silence_sec}s"
  fi
fi

# Optional: BullMQ queue depth (requires REDIS_URL and tsx)
if [[ -n "${REDIS_URL:-}" ]] && command -v tsx >/dev/null 2>&1; then
  qout=$(tsx "$ROOT_DIR/scripts/monitor/check-queue.ts" 2>/dev/null || true)
  if [[ -n "$qout" ]]; then
    log "queue $qout"
    qwait=$(awk '{for(i=1;i<=NF;i++){if($i ~ /^WAIT=/){split($i,a,"="); print a[2]}}}' <<<"$qout")
    if [[ -n "$qwait" ]] && (( qwait > ${QUEUE_WARN_WAIT:-5000} )); then
      maybe_slack "[TWZRD Monitor] Queue backlog WAIT=${qwait} (> ${QUEUE_WARN_WAIT:-5000})"
    fi
  fi
fi

# Slack summary when any HTTP failed (non-spammy; single line)
if [[ $fail_total -gt 0 ]]; then
  maybe_slack "[TWZRD Monitor] FAIL: ${failed_list} at $(timestamp)"
fi

exit 0
