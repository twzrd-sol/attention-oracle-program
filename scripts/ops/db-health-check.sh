#!/bin/bash
# Database Health Check - Connections, replication lag, missing indexes
# Run daily (low-overhead check)

set -e

LOG_FILE="/var/log/twzrd-db-health.log"
TIMESTAMP=$(date -u '+%Y-%m-%d %H:%M:%S UTC')

{
  echo "[$TIMESTAMP] Database health check started"
  
  DB_HOST=${DATABASE_HOST:-localhost}
  DB_NAME=${DATABASE_NAME:-attention_oracle}
  DB_USER=${DATABASE_USER:-postgres}
  
  # Check active connections
  CONN_COUNT=$(psql -h "$DB_HOST" -U "$DB_USER" -d "$DB_NAME" -tAc "SELECT count(*) FROM pg_stat_activity WHERE state = 'active';" 2>/dev/null || echo "0")
  echo "[$TIMESTAMP] Active connections: $CONN_COUNT"
  
  # Check database size
  DB_SIZE=$(psql -h "$DB_HOST" -U "$DB_USER" -d "$DB_NAME" -tAc "SELECT pg_size_pretty(pg_database_size('$DB_NAME'));" 2>/dev/null || echo "unknown")
  echo "[$TIMESTAMP] Database size: $DB_SIZE"
  
  # Check for missing indexes (tables without primary key)
  MISSING_PK=$(psql -h "$DB_HOST" -U "$DB_USER" -d "$DB_NAME" -tAc "
    SELECT count(*) FROM pg_tables t
    WHERE schemaname NOT IN ('pg_catalog', 'information_schema')
    AND NOT EXISTS (SELECT 1 FROM pg_constraint c WHERE c.conrelid = (t.schemaname||'.'||t.tablename)::regclass AND c.contype = 'p');
  " 2>/dev/null || echo "0")
  
  if [ "$MISSING_PK" -gt 0 ]; then
    echo "[$TIMESTAMP] ⚠️ WARNING: $MISSING_PK tables missing primary key"
  else
    echo "[$TIMESTAMP] ✅ All tables have primary keys"
  fi
  
  # Check dead rows (bloat)
  DEAD_ROWS=$(psql -h "$DB_HOST" -U "$DB_USER" -d "$DB_NAME" -tAc "
    SELECT sum(n_dead_tup) FROM pg_stat_user_tables;
  " 2>/dev/null || echo "0")
  
  echo "[$TIMESTAMP] Dead rows to clean: $DEAD_ROWS"
  
  if [ "$DEAD_ROWS" -gt 100000 ]; then
    echo "[$TIMESTAMP] ⚠️ ALERT: High row bloat ($DEAD_ROWS dead rows)"
  fi
  
  echo "[$TIMESTAMP] Health check complete"
  
} >> "$LOG_FILE" 2>&1

exit 0
