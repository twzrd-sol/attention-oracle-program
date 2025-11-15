#!/bin/bash
# Quick monitoring summary dashboard for TWZRD operators
# Shows latest status from all AI monitoring agents

echo "╔════════════════════════════════════════════════════════════════╗"
echo "║         🤖 TWZRD AI MONITORING AGENTS - STATUS SUMMARY         ║"
echo "╚════════════════════════════════════════════════════════════════╝"
echo ""

# Latest monitoring run
if [ -f /home/twzrd/milo-token/logs/ops/monitor-health.log ]; then
  echo "📊 Latest Health Check:"
  tail -20 /home/twzrd/milo-token/logs/ops/monitor-health.log | grep -E "(Total alerts|Critical|Warning|Info|healthy)" | tail -5
  echo ""
fi

# Reconciliation status
if [ -f /home/twzrd/milo-token/logs/ops/reconcile.log ]; then
  echo "🔗 Latest Reconciliation:"
  tail -5 /home/twzrd/milo-token/logs/ops/reconcile.log
  echo ""
fi

# CLS Discovery status
if [ -f /home/twzrd/milo-token/logs/cls-discovery.log ]; then
  echo "🔍 Latest CLS Discovery:"
  tail -10 /home/twzrd/milo-token/logs/cls-discovery.log | grep -E "(Complete|Channels|Duration)" | tail -4
  echo ""
fi

# PM2 status
echo "⚙️  Worker Status:"
pm2 list | grep -E "(milo-aggregator|worker|stream-listener|gateway)" | head -10
echo ""

# Database quick stats
echo "💾 Database Quick Stats:"
psql "postgresql://twzrd:twzrd_password_2025@localhost:6432/twzrd_oracle" -t -c "
  SELECT
    'Sealed epochs (last 24h): ' || COUNT(*)
  FROM sealed_epochs
  WHERE sealed_at > extract(epoch from now()) - 86400;

  SELECT
    'Unpublished epochs: ' || COUNT(*)
  FROM sealed_epochs
  WHERE (published IS NULL OR published = 0);

  SELECT
    'L2 cache entries: ' || COUNT(*)
  FROM l2_tree_cache;
" 2>/dev/null || echo "  (Database connection failed)"

echo ""
echo "════════════════════════════════════════════════════════════════"
echo "Run 'npx tsx scripts/ops/monitor-health.ts' for detailed check"
echo "View logs: tail -f logs/ops/monitor-health.log"
echo "════════════════════════════════════════════════════════════════"
