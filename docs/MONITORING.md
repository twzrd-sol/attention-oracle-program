# 🤖 TWZRD AI Monitoring Agents

A suite of personality-driven monitoring agents that watch over the TWZRD protocol and send alerts to Slack.

## Quick Start

```bash
# View current system status
./scripts/ops/monitor-summary.sh

# Run full health check manually
npx tsx scripts/ops/monitor-health.ts

# View monitoring logs
tail -f logs/ops/monitor-health.log

# Test Slack alert
cd /home/twzrd/milo-token && npx tsx scripts/ops/reconcile-roots.ts --test-alert
```

## The Agents

### 👔 Chief Database Officer
**Domain**: Database health and performance
**Personality**: Professional, detail-oriented database administrator
**Monitors**:
- Connection pool health (waiting queries, pool saturation)
- Query latency (alerts if >500ms)
- Recent seal activity (alerts if no seals in 2 hours)
- Database size growth (alerts if >10GB)

**Alerts**:
- `WARNING`: Connection pool congestion, elevated latency
- `CRITICAL`: No sealed epochs in 2 hours, database unreachable

---

### 🛡️ Epoch Sentinel
**Domain**: Epoch sealing and finalization
**Personality**: Vigilant guardian of temporal integrity
**Monitors**:
- Unsealed channels for previous epoch
- Missing epochs (gaps in sealed_epochs)
- Epoch finalization lag

**Alerts**:
- `WARNING`: Channels with unsealed epochs, gaps detected
- `CRITICAL`: Critical epoch data missing

---

### 🚨 Publisher Watchdog
**Domain**: On-chain publish pipeline
**Personality**: Alert, responsive monitoring dog
**Monitors**:
- Unpublished sealed epochs backlog
- L2 tree cache hit rate
- Publishing throughput

**Alerts**:
- `WARNING`: Backlog >20 epochs, cache rate <50%
- `CRITICAL`: Backlog >100 epochs (publisher stuck)

---

### 👼 Twitch Guardian Angel
**Domain**: Worker health and service availability
**Personality**: Protective, caring angel watching over services
**Monitors**:
- PM2 worker status (online/offline/errored)
- Frequent restart detection (crash loop)
- Aggregator health endpoint
- Gateway availability

**Alerts**:
- `WARNING`: Frequent worker restarts
- `CRITICAL`: Workers offline, services unreachable

---

### 🔍 Discovery Scout
**Domain**: CLS Top 100 discovery monitoring
**Personality**: Curious explorer tracking trends
**Monitors**:
- Last discovery run timestamp
- Channel count trends (detects Twitch API issues)
- Discovery cron health

**Alerts**:
- `WARNING`: No discovery in 2+ hours, channel count drops
- `INFO`: CLS feature not deployed

---

### 💾 Storage Custodian
**Domain**: Disk space and storage health
**Personality**: Careful archivist protecting data longevity
**Monitors**:
- Root disk usage (/, /home, /var)
- Database file size growth
- Storage capacity trends

**Alerts**:
- `WARNING`: Disk >80% full, DB >50GB
- `CRITICAL`: Disk >90% full

---

### 🌐 RPC Navigator
**Domain**: Solana RPC endpoint health
**Personality**: Skilled navigator charting blockchain connectivity
**Monitors**:
- RPC endpoint availability
- Request latency (alerts >3s)
- Failover status

**Alerts**:
- `WARNING`: Some RPC endpoints failing, high latency
- `CRITICAL`: All RPC endpoints unreachable (publishing blocked)

---

## Alerting Schedule

Monitoring runs automatically via cron:

```cron
# Health monitoring - every 15 minutes
*/15 * * * * npx tsx scripts/ops/monitor-health.ts

# Reconciliation check - hourly at :20
20 * * * * npx tsx scripts/ops/reconcile-roots.ts

# CLS discovery - hourly at :00
0 * * * * npx tsx scripts/ops/discover-top100-cls.ts
```

## Slack Integration

All alerts are sent to the `#all-twzrd` Slack channel via webhook.

**Alert Severity Levels**:
- 🔵 `INFO`: Informational updates, daily health summaries
- ⚠️ `WARNING`: Issues requiring attention but not critical
- 🚨 `CRITICAL`: Immediate action required

**Daily "All Clear"**: If no issues are detected, agents send a daily health summary confirming everything is running smoothly.

## Alert Examples

### Critical Alert
```
🚨 Publisher Watchdog 🚨
CRITICAL: Publish backlog at 882 sealed epochs. Publisher may be stuck!
```

### Warning Alert
```
⚠️ Chief Database Officer ⚠️
Connection pool congestion detected. 7 queries waiting for connections.
```

### Info Alert
```
ℹ️ System Health ℹ️
All TWZRD monitoring agents report healthy status. Everything is running smoothly!
```

## Troubleshooting

### Agent shows false positives
Check the agent's source code in `scripts/ops/monitor-health.ts` and adjust thresholds.

### Missing Slack alerts
1. Verify `SLACK_WEBHOOK` in `.env`
2. Test with: `npx tsx scripts/ops/reconcile-roots.ts --test-alert`
3. Check Slack webhook permissions

### High alert volume
Adjust monitoring frequency in crontab or increase alert thresholds.

## Extending the System

To add a new agent:

1. Add function in `monitor-health.ts`:
```typescript
async function myNewAgent(pool: Pool): Promise<Alert[]> {
  const alerts: Alert[] = [];

  // Your monitoring logic here

  return alerts;
}
```

2. Add to orchestrator:
```typescript
const [dboAlerts, ..., myAlerts] = await Promise.all([
  chiefDatabaseOfficer(pool),
  // ...
  myNewAgent(pool),
]);
```

3. Give it a personality and emoji!

---

**Built with ❤️ for TWZRD protocol transparency and reliability**
