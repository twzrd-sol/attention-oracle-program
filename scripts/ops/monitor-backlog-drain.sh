#!/bin/bash
# Live backlog drain monitor
# Shows real-time progress of publisher clearing the 163-epoch backlog

echo "🚨 Publisher Backlog Drain Monitor"
echo "Starting backlog: 882 epochs"
echo "Current target: 0 epochs"
echo ""
echo "Press Ctrl+C to exit"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

LAST_COUNT=0

while true; do
  # Get current unpublished count
  COUNT=$(psql "$DATABASE_URL" -At -c "SELECT COUNT(*) FROM sealed_epochs WHERE published IS NULL OR published = 0" 2>/dev/null)

  if [ -z "$COUNT" ]; then
    echo "⚠️  Database connection failed"
    sleep 5
    continue
  fi

  # Calculate drain rate
  if [ $LAST_COUNT -gt 0 ]; then
    DELTA=$((LAST_COUNT - COUNT))
    if [ $DELTA -gt 0 ]; then
      TREND="📉 -$DELTA"
    elif [ $DELTA -lt 0 ]; then
      TREND="📈 +${DELTA#-}"
    else
      TREND="⏸️  unchanged"
    fi
  else
    TREND="⏳ monitoring..."
    DELTA=0
  fi

  # Progress bar
  PROGRESS=$((100 - (COUNT * 100 / 882)))
  BAR_LENGTH=50
  FILLED=$((PROGRESS * BAR_LENGTH / 100))
  BAR=$(printf "█%.0s" $(seq 1 $FILLED))
  EMPTY=$(printf "░%.0s" $(seq 1 $((BAR_LENGTH - FILLED))))

  # Latest published epochs
  RECENT=$(psql "$DATABASE_URL" -At -c "SELECT channel FROM sealed_epochs WHERE published = 1 ORDER BY sealed_at DESC LIMIT 3" 2>/dev/null | tr '\n' ',' | sed 's/,$//')

  # Display
  clear
  echo "🚨 Publisher Backlog Drain Monitor"
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  echo ""
  echo "  📊 Progress: ${PROGRESS}% complete"
  echo "  [$BAR$EMPTY] "
  echo ""
  echo "  📉 Unpublished: $COUNT epochs"
  echo "  $TREND since last check"
  echo ""
  echo "  🎯 Target: 0 epochs (100% published)"
  echo "  📈 Cleared: $((882 - COUNT)) epochs"
  echo ""
  echo "  🕐 Last check: $(date '+%H:%M:%S')"
  echo "  ⏱️  ETA: ~$((COUNT / 10)) minutes (at 10/min)"
  echo ""
  echo "  ✅ Recently published: $RECENT"
  echo ""
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  echo "Press Ctrl+C to exit"

  LAST_COUNT=$COUNT
  sleep 10
done
