#!/bin/bash
# Database Maintenance - Vacuum & Analyze
# Run weekly (low-traffic window: Wed 03:00 UTC)
# Cleans up dead rows, updates query planner statistics

set -e

LOG_FILE="/var/log/twzrd-db-maint.log"
TIMESTAMP=$(date -u '+%Y-%m-%d %H:%M:%S UTC')

{
  echo "[$TIMESTAMP] Starting PostgreSQL maintenance..."
  
  # Get database connection info from environment
  DB_HOST=${DATABASE_HOST:-localhost}
  DB_NAME=${DATABASE_NAME:-attention_oracle}
  DB_USER=${DATABASE_USER:-postgres}
  
  # Run VACUUM ANALYZE on all tables
  psql -h "$DB_HOST" -U "$DB_USER" -d "$DB_NAME" -c "VACUUM ANALYZE;" 2>&1 && \
    echo "[$TIMESTAMP] ✅ VACUUM ANALYZE completed" || \
    echo "[$TIMESTAMP] ⚠️ VACUUM ANALYZE failed"
  
  # Get table sizes
  echo "[$TIMESTAMP] Database stats:"
  psql -h "$DB_HOST" -U "$DB_USER" -d "$DB_NAME" -c "
    SELECT 
      schemaname,
      tablename,
      pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) as size
    FROM pg_tables 
    WHERE schemaname NOT IN ('pg_catalog', 'information_schema')
    ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;
  " 2>&1 || echo "[$TIMESTAMP] ⚠️ Failed to get table stats"
  
  echo "[$TIMESTAMP] PostgreSQL maintenance complete"
  
} >> "$LOG_FILE" 2>&1

exit 0
