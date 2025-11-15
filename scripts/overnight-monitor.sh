#!/bin/bash

# Overnight Monitoring Script
# Run every 2 hours to ensure system stability
# No service restarts unless critically necessary

LOG_DIR="/home/twzrd/milo-token/logs/monitoring"
mkdir -p "$LOG_DIR"

TIMESTAMP=$(date +"%Y-%m-%d_%H-%M-%S")
LOG_FILE="$LOG_DIR/monitor_${TIMESTAMP}.log"

echo "============================================" | tee -a "$LOG_FILE"
echo "Overnight Monitor - $TIMESTAMP" | tee -a "$LOG_FILE"
echo "============================================" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

# Database connection string
DB_URL="postgresql://doadmin:AVNS_7OLyCRhJkIPcAKrZMoi@twzrd-prod-postgres-do-user-21113270-0.f.db.ondigitalocean.com:25061/twzrd-oracle-pool?sslmode=require"

# ============================================
# 1. SERVICE HEALTH CHECK
# ============================================

echo "### 1. SERVICE HEALTH CHECK ###" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

pm2 status | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

# Check for any stopped services (excluding cls-discovery which is expected to be stopped)
STOPPED_COUNT=$(pm2 jlist | jq '[.[] | select(.pm2_env.status != "online" and .name != "cls-discovery")] | length')

if [ "$STOPPED_COUNT" -gt 0 ]; then
  echo "⚠️  WARNING: $STOPPED_COUNT service(s) not online!" | tee -a "$LOG_FILE"
  pm2 jlist | jq -r '.[] | select(.pm2_env.status != "online" and .name != "cls-discovery") | "\(.name): \(.pm2_env.status)"' | tee -a "$LOG_FILE"
else
  echo "✅ All critical services online" | tee -a "$LOG_FILE"
fi

# Note cls-discovery status (expected to be stopped)
CLS_DISCOVERY_STATUS=$(pm2 jlist | jq -r '.[] | select(.name == "cls-discovery") | .pm2_env.status')
if [ "$CLS_DISCOVERY_STATUS" == "stopped" ]; then
  echo "   cls-discovery: stopped (✅ CORRECT - Scout runs periodically, not 24/7)" | tee -a "$LOG_FILE"
elif [ "$CLS_DISCOVERY_STATUS" == "online" ]; then
  echo "   cls-discovery: online (ℹ️  Scout is currently running)" | tee -a "$LOG_FILE"
fi

echo "" | tee -a "$LOG_FILE"

# Check memory usage for critical services
echo "Memory usage for critical services:" | tee -a "$LOG_FILE"
pm2 jlist | jq -r '.[] | select(.name | test("aggregator|worker|tree-builder")) | "\(.name): \(.monit.memory / 1024 / 1024 | floor) MB"' | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

# ============================================
# 2. DATABASE GROWTH CHECK
# ============================================

echo "### 2. DATABASE GROWTH CHECK ###" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

# Disk usage
echo "Disk usage:" | tee -a "$LOG_FILE"
df -h / | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

# user_signals table row count
USER_SIGNALS_COUNT=$(psql "$DB_URL" -t -c "SELECT COUNT(*) FROM user_signals;")
echo "user_signals row count: $(echo $USER_SIGNALS_COUNT | xargs)" | tee -a "$LOG_FILE"

# Check growth rate (compare to previous log if exists)
PREV_LOG=$(ls -t "$LOG_DIR"/monitor_*.log 2>/dev/null | sed -n 2p)
if [ -n "$PREV_LOG" ]; then
  PREV_COUNT=$(grep "user_signals row count:" "$PREV_LOG" | tail -1 | awk '{print $NF}')
  if [ -n "$PREV_COUNT" ]; then
    GROWTH=$((USER_SIGNALS_COUNT - PREV_COUNT))
    echo "Growth since last check: +$GROWTH rows" | tee -a "$LOG_FILE"

    # Warning if growth is too high (>1M rows in 2h is suspicious)
    if [ "$GROWTH" -gt 1000000 ]; then
      echo "⚠️  WARNING: Unusually high growth rate!" | tee -a "$LOG_FILE"
    else
      echo "✅ Growth rate normal" | tee -a "$LOG_FILE"
    fi
  fi
fi

echo "" | tee -a "$LOG_FILE"

# Database size
echo "Database table sizes:" | tee -a "$LOG_FILE"
psql "$DB_URL" -c "
SELECT
  tablename,
  pg_size_pretty(pg_total_relation_size('public.'||tablename)) AS size
FROM pg_tables
WHERE schemaname = 'public'
ORDER BY pg_total_relation_size('public.'||tablename) DESC
LIMIT 5;
" | tee -a "$LOG_FILE"

echo "" | tee -a "$LOG_FILE"

# ============================================
# 3. NEW EPOCH SEALING CHECK
# ============================================

echo "### 3. NEW EPOCH SEALING CHECK ###" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

# Get latest sealed epochs for MILO and CLS
echo "Latest sealed epochs (MILO):" | tee -a "$LOG_FILE"
psql "$DB_URL" -c "
SELECT
  channel,
  MAX(epoch) as latest_epoch,
  TO_TIMESTAMP(MAX(epoch)) as epoch_time,
  EXTRACT(EPOCH FROM (NOW() - TO_TIMESTAMP(MAX(epoch))))/3600 as hours_ago
FROM sealed_epochs
WHERE token_group = 'MILO'
  AND channel IN ('lacy','jasontheween','adapt','kaysan','silky','yourragegaming','stableronaldo','threadguy','marlon','n3on','thesketchreal','orangieyt')
GROUP BY channel
ORDER BY latest_epoch DESC
LIMIT 5;
" | tee -a "$LOG_FILE"

echo "" | tee -a "$LOG_FILE"

echo "Latest sealed epochs (CLS top 5):" | tee -a "$LOG_FILE"
psql "$DB_URL" -c "
SELECT
  channel,
  MAX(epoch) as latest_epoch,
  TO_TIMESTAMP(MAX(epoch)) as epoch_time,
  EXTRACT(EPOCH FROM (NOW() - TO_TIMESTAMP(MAX(epoch))))/3600 as hours_ago
FROM sealed_epochs
WHERE token_group = 'CLS'
GROUP BY channel
ORDER BY latest_epoch DESC
LIMIT 5;
" | tee -a "$LOG_FILE"

echo "" | tee -a "$LOG_FILE"

# Check if new epochs are being sealed (within last 2 hours)
# Expected: 1 seal per hour = 2 seals in last 2 hours
CURRENT_EPOCH=$(($(date +%s) / 3600 * 3600))
RECENT_SEALED=$(psql "$DB_URL" -t -c "SELECT COUNT(DISTINCT epoch) FROM sealed_epochs WHERE epoch >= $CURRENT_EPOCH - 7200;")

echo "Epochs sealed in last 2 hours: $(echo $RECENT_SEALED | xargs) (expected: ~2)" | tee -a "$LOG_FILE"

if [ "$(echo $RECENT_SEALED | xargs)" -eq 0 ]; then
  echo "⚠️  WARNING: No new epochs sealed in last 2 hours!" | tee -a "$LOG_FILE"
elif [ "$(echo $RECENT_SEALED | xargs)" -ge 2 ]; then
  echo "✅ Sealing frequency normal (1 epoch/hour)" | tee -a "$LOG_FILE"
else
  echo "ℹ️  Note: Only $(echo $RECENT_SEALED | xargs) epoch(s) sealed (might be between hourly seals)" | tee -a "$LOG_FILE"
fi

echo "" | tee -a "$LOG_FILE"

# ============================================
# 4. REDIS HEALTH CHECK
# ============================================

echo "### 4. REDIS HEALTH CHECK ###" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

# Check Redis connectivity and key count
REDIS_KEYS=$(redis-cli -u "rediss://default:AVNS_5hrxWgtCINC5SIFKbe2@twzrd-bullmq-redis-do-user-21113270-0.i.db.ondigitalocean.com:25061" --tls DBSIZE 2>/dev/null | grep -oP '\d+')

if [ -n "$REDIS_KEYS" ]; then
  echo "✅ Redis connected - $REDIS_KEYS keys" | tee -a "$LOG_FILE"
else
  echo "⚠️  WARNING: Redis connection failed!" | tee -a "$LOG_FILE"
fi

echo "" | tee -a "$LOG_FILE"

# ============================================
# 5. CURRENT EPOCH STATUS
# ============================================

echo "### 5. CURRENT EPOCH STATUS ###" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

CURRENT_EPOCH_TIME=$(date -d @$CURRENT_EPOCH)
NEXT_EPOCH=$((CURRENT_EPOCH + 3600))
NEXT_EPOCH_TIME=$(date -d @$NEXT_EPOCH)

echo "Current epoch: $CURRENT_EPOCH ($CURRENT_EPOCH_TIME)" | tee -a "$LOG_FILE"
echo "Next epoch: $NEXT_EPOCH ($NEXT_EPOCH_TIME)" | tee -a "$LOG_FILE"

TIME_UNTIL_NEXT=$(((NEXT_EPOCH - $(date +%s)) / 60))
echo "Time until next seal: $TIME_UNTIL_NEXT minutes" | tee -a "$LOG_FILE"

echo "" | tee -a "$LOG_FILE"

# ============================================
# 6. SUMMARY
# ============================================

echo "### SUMMARY ###" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

# Count warnings
WARNING_COUNT=$(grep -c "⚠️" "$LOG_FILE" || echo "0")
SUCCESS_COUNT=$(grep -c "✅" "$LOG_FILE" || echo "0")

echo "Warnings: $WARNING_COUNT" | tee -a "$LOG_FILE"
echo "Checks passed: $SUCCESS_COUNT" | tee -a "$LOG_FILE"

if [ "$WARNING_COUNT" -eq 0 ]; then
  echo "" | tee -a "$LOG_FILE"
  echo "✅ ALL SYSTEMS NOMINAL - No action required" | tee -a "$LOG_FILE"
else
  echo "" | tee -a "$LOG_FILE"
  echo "⚠️  WARNINGS DETECTED - Review required" | tee -a "$LOG_FILE"
fi

echo "" | tee -a "$LOG_FILE"
echo "============================================" | tee -a "$LOG_FILE"
echo "Monitor run completed at $(date)" | tee -a "$LOG_FILE"
echo "============================================" | tee -a "$LOG_FILE"

# Keep only last 48 hours of logs (cleanup old logs)
find "$LOG_DIR" -name "monitor_*.log" -mtime +2 -delete

exit 0
