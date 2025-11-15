#!/usr/bin/env tsx
/**
 * TWZRD AI Monitoring Agents 🤖
 * A suite of personality-driven agents watching over the protocol
 *
 * Each agent has a specific domain and personality:
 * - Chief Database Officer: Database health and performance
 * - Epoch Sentinel: Epoch sealing and finalization
 * - Publisher Watchdog: On-chain publish pipeline
 * - Twitch Guardian Angel: Worker and IRC connection health
 * - Discovery Scout: CLS Top 100 discovery monitoring
 */

import 'dotenv/config';
import { Pool } from 'pg';
import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

const SLACK_WEBHOOK = process.env.SLACK_WEBHOOK;
const DATABASE_URL = process.env.DATABASE_URL || 'postgresql://twzrd:twzrd_password_2025@localhost:6432/twzrd_oracle';
const EPOCH_SECONDS = Number(process.env.EPOCH_SECONDS || 3600);

interface Alert {
  agent: string;
  emoji: string;
  severity: 'info' | 'warning' | 'critical';
  message: string;
  context?: any;
}

async function slack(alert: Alert) {
  if (!SLACK_WEBHOOK) return;

  const severityEmoji = {
    info: ':information_source:',
    warning: ':warning:',
    critical: ':rotating_light:',
  };

  const text = `${alert.emoji} *${alert.agent}* ${severityEmoji[alert.severity]}\n${alert.message}`;

  try {
    await fetch(SLACK_WEBHOOK, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        text,
        blocks: alert.context ? [
          { type: 'section', text: { type: 'mrkdwn', text } },
          { type: 'context', elements: [{ type: 'mrkdwn', text: `\`\`\`${JSON.stringify(alert.context, null, 2)}\`\`\`` }] }
        ] : undefined
      })
    });
  } catch (err) {
    console.error('Failed to send Slack alert:', err);
  }
}

/**
 * 👔 Chief Database Officer
 * Monitors database health, connection pool, query latency
 */
async function chiefDatabaseOfficer(pool: Pool): Promise<Alert[]> {
  const alerts: Alert[] = [];

  try {
    // Check connection pool health
    const poolStats = {
      totalCount: pool.totalCount,
      idleCount: pool.idleCount,
      waitingCount: pool.waitingCount,
    };

    if (poolStats.waitingCount > 5) {
      alerts.push({
        agent: 'Chief Database Officer',
        emoji: ':man_office_worker:',
        severity: 'warning',
        message: `Connection pool congestion detected. ${poolStats.waitingCount} queries waiting for connections.`,
        context: poolStats
      });
    }

    // Check query latency
    const start = Date.now();
    await pool.query('SELECT 1');
    const latency = Date.now() - start;

    if (latency > 500) {
      alerts.push({
        agent: 'Chief Database Officer',
        emoji: ':man_office_worker:',
        severity: 'warning',
        message: `Database query latency elevated: ${latency}ms (normal <100ms)`,
        context: { latency_ms: latency }
      });
    }

    // Check recent sealed epochs
    const result = await pool.query(`
      SELECT COUNT(*) as count
      FROM sealed_epochs
      WHERE sealed_at > extract(epoch from now()) - 7200
    `);
    const recentSeals = parseInt(result.rows[0]?.count || '0');

    if (recentSeals === 0) {
      alerts.push({
        agent: 'Chief Database Officer',
        emoji: ':man_office_worker:',
        severity: 'critical',
        message: 'No sealed epochs in the last 2 hours! Auto-finalizer may be down.',
      });
    }

    // Check database size (alert if growing too fast)
    const sizeResult = await pool.query(`
      SELECT pg_size_pretty(pg_database_size(current_database())) as size,
             pg_database_size(current_database()) as bytes
    `);
    const dbSize = sizeResult.rows[0];

    // If we had previous metrics, we could alert on growth rate
    // For now, just log if over 10GB
    if (parseInt(dbSize.bytes) > 10 * 1024 * 1024 * 1024) {
      alerts.push({
        agent: 'Chief Database Officer',
        emoji: ':man_office_worker:',
        severity: 'info',
        message: `Database size: ${dbSize.size}. Consider archival strategy.`,
      });
    }

  } catch (err: any) {
    alerts.push({
      agent: 'Chief Database Officer',
      emoji: ':man_office_worker:',
      severity: 'critical',
      message: `Database health check failed: ${err.message}`,
    });
  }

  return alerts;
}

/**
 * 🛡️ Epoch Sentinel
 * Watches epoch sealing, missing epochs, and finalization
 */
async function epochSentinel(pool: Pool): Promise<Alert[]> {
  const alerts: Alert[] = [];

  try {
    const now = Math.floor(Date.now() / 1000);
    const currentEpoch = Math.floor(now / EPOCH_SECONDS) * EPOCH_SECONDS;
    const prevEpoch = currentEpoch - EPOCH_SECONDS;
    const twoEpochsAgo = currentEpoch - (EPOCH_SECONDS * 2);

    // Check if previous epoch is sealed for all active channels
    const activeChannels = await pool.query(`
      SELECT DISTINCT channel
      FROM channel_participation
      WHERE epoch = $1
    `, [prevEpoch]);

    const sealedChannels = await pool.query(`
      SELECT DISTINCT channel
      FROM sealed_epochs
      WHERE epoch = $1
    `, [prevEpoch]);

    const active = new Set(activeChannels.rows.map(r => r.channel));
    const sealed = new Set(sealedChannels.rows.map(r => r.channel));

    const unsealed = [...active].filter(ch => !sealed.has(ch));

    if (unsealed.length > 0) {
      alerts.push({
        agent: 'Epoch Sentinel',
        emoji: ':shield:',
        severity: 'warning',
        message: `${unsealed.length} channel(s) have unsea led epoch ${prevEpoch}`,
        context: { unsealed_channels: unsealed.slice(0, 5), epoch: prevEpoch }
      });
    }

    // Check for missing epochs (gaps in sealed_epochs)
    const gapCheck = await pool.query(`
      WITH epoch_range AS (
        SELECT DISTINCT epoch
        FROM sealed_epochs
        WHERE epoch >= $1 AND epoch < $2
        ORDER BY epoch
      ),
      expected AS (
        SELECT generate_series($1, $2 - $3, $3) as epoch
      )
      SELECT e.epoch as missing_epoch
      FROM expected e
      LEFT JOIN epoch_range r ON e.epoch = r.epoch
      WHERE r.epoch IS NULL
    `, [twoEpochsAgo, currentEpoch, EPOCH_SECONDS]);

    if (gapCheck.rows.length > 0) {
      alerts.push({
        agent: 'Epoch Sentinel',
        emoji: ':shield:',
        severity: 'warning',
        message: `Detected ${gapCheck.rows.length} missing epoch(s) in recent history`,
        context: { missing_epochs: gapCheck.rows.map(r => r.missing_epoch) }
      });
    }

  } catch (err: any) {
    alerts.push({
      agent: 'Epoch Sentinel',
      emoji: ':shield:',
      severity: 'critical',
      message: `Epoch monitoring failed: ${err.message}`,
    });
  }

  return alerts;
}

/**
 * 🚨 Publisher Watchdog
 * Monitors on-chain publish backlog and failures
 */
async function publisherWatchdog(pool: Pool): Promise<Alert[]> {
  const alerts: Alert[] = [];

  try {
    const now = Math.floor(Date.now() / 1000);
    const currentEpoch = Math.floor(now / EPOCH_SECONDS) * EPOCH_SECONDS;

    // Check unpublished backlog
    const backlog = await pool.query(`
      SELECT COUNT(*) as count
      FROM sealed_epochs
      WHERE epoch < $1
      AND (published IS NULL OR published = 0)
    `, [currentEpoch]);

    const backlogCount = parseInt(backlog.rows[0]?.count || '0');

    // Check if publisher is actively draining (count published in last hour)
    const recentlyPublished = await pool.query(`
      SELECT COUNT(*) as count
      FROM sealed_epochs
      WHERE published_at > NOW() - INTERVAL '1 hour'
    `);
    const publishedLastHour = parseInt(recentlyPublished.rows[0]?.count || '0');
    const drainRate = publishedLastHour; // epochs per hour

    // Smart thresholds based on drain activity
    if (backlogCount > 500 && drainRate < 10) {
      alerts.push({
        agent: 'Publisher Watchdog',
        emoji: ':dog:',
        severity: 'critical',
        message: `CRITICAL: Large backlog (${backlogCount}) with low drain rate (${drainRate}/hr). Publisher may be stuck!`,
        context: { backlog_count: backlogCount, drain_rate_per_hour: drainRate }
      });
    } else if (backlogCount > 1000) {
      alerts.push({
        agent: 'Publisher Watchdog',
        emoji: ':dog:',
        severity: 'warning',
        message: `Large publish backlog: ${backlogCount} epochs (draining at ${drainRate}/hr, ETA: ${Math.ceil(backlogCount/Math.max(drainRate,1))} hours)`,
        context: { backlog_count: backlogCount, drain_rate_per_hour: drainRate }
      });
    } else if (backlogCount > 100 && drainRate === 0) {
      alerts.push({
        agent: 'Publisher Watchdog',
        emoji: ':dog:',
        severity: 'warning',
        message: `Publisher stalled: ${backlogCount} epochs queued but no publishing activity in last hour`,
        context: { backlog_count: backlogCount, drain_rate_per_hour: 0 }
      });
    }

    // Check L2 tree cache rate
    const cacheStats = await pool.query(`
      SELECT
        COUNT(*) as total_sealed,
        SUM(CASE WHEN l2.root IS NOT NULL THEN 1 ELSE 0 END) as cached_count
      FROM sealed_epochs se
      LEFT JOIN l2_tree_cache l2 ON se.epoch = l2.epoch AND se.channel = l2.channel
      WHERE se.epoch >= $1
    `, [currentEpoch - (EPOCH_SECONDS * 10)]);

    const stats = cacheStats.rows[0];
    const cacheRate = stats.total_sealed > 0 ? (stats.cached_count / stats.total_sealed) * 100 : 0;

    if (cacheRate < 50) {
      alerts.push({
        agent: 'Publisher Watchdog',
        emoji: ':dog:',
        severity: 'warning',
        message: `L2 tree cache rate low: ${cacheRate.toFixed(1)}% (${stats.cached_count}/${stats.total_sealed} sealed epochs cached)`,
      });
    }

  } catch (err: any) {
    alerts.push({
      agent: 'Publisher Watchdog',
      emoji: ':dog:',
      severity: 'critical',
      message: `Publisher monitoring failed: ${err.message}`,
    });
  }

  return alerts;
}

/**
 * 👼 Twitch Guardian Angel
 * Monitors worker health and IRC connections
 */
async function twitchGuardianAngel(): Promise<Alert[]> {
  const alerts: Alert[] = [];

  try {
    // Check PM2 process status
    const { stdout } = await execAsync('pm2 jlist');
    const processes = JSON.parse(stdout);

    const workers = processes.filter((p: any) =>
      p.name?.includes('worker') || p.name?.includes('stream-listener')
    );

    const offline = workers.filter((p: any) => p.pm2_env?.status !== 'online');

    if (offline.length > 0) {
      alerts.push({
        agent: 'Twitch Guardian Angel',
        emoji: ':angel:',
        severity: 'critical',
        message: `${offline.length} worker(s) offline! ${offline.map((p: any) => p.name).join(', ')}`,
        context: { offline_workers: offline.map((p: any) => ({ name: p.name, status: p.pm2_env?.status })) }
      });
    }

    // Check restart counts (frequent restarts indicate crashes)
    const restarting = workers.filter((p: any) => (p.pm2_env?.restart_time || 0) > 10);

    if (restarting.length > 0) {
      alerts.push({
        agent: 'Twitch Guardian Angel',
        emoji: ':angel:',
        severity: 'warning',
        message: `${restarting.length} worker(s) restarting frequently`,
        context: { workers: restarting.map((p: any) => ({ name: p.name, restarts: p.pm2_env?.restart_time })) }
      });
    }

    // Check aggregator health endpoint
    const aggPort = process.env.PORT || '8082';
    try {
      const response = await fetch(`http://localhost:${aggPort}/health`, { signal: AbortSignal.timeout(5000) });
      if (!response.ok) {
        alerts.push({
          agent: 'Twitch Guardian Angel',
          emoji: ':angel:',
          severity: 'critical',
          message: `Aggregator health check failed: HTTP ${response.status}`,
        });
      }
    } catch (err: any) {
      alerts.push({
        agent: 'Twitch Guardian Angel',
        emoji: ':angel:',
        severity: 'critical',
        message: `Cannot reach aggregator at localhost:${aggPort}: ${err.message}`,
      });
    }

  } catch (err: any) {
    alerts.push({
      agent: 'Twitch Guardian Angel',
      emoji: ':angel:',
      severity: 'critical',
      message: `Worker health check failed: ${err.message}`,
    });
  }

  return alerts;
}

/**
 * 🔍 Discovery Scout
 * Monitors CLS Top 100 discovery runs
 */
async function discoveryScout(pool: Pool): Promise<Alert[]> {
  const alerts: Alert[] = [];

  try {
    // Check if cls_discovered_channels table exists
    const tableCheck = await pool.query(`
      SELECT EXISTS (
        SELECT FROM information_schema.tables
        WHERE table_schema = 'public'
        AND table_name = 'cls_discovered_channels'
      ) as exists
    `);

    if (!tableCheck.rows[0]?.exists) {
      alerts.push({
        agent: 'Discovery Scout',
        emoji: ':mag:',
        severity: 'info',
        message: 'CLS discovery table not found. CLS Top 100 feature not deployed.',
      });
      return alerts;
    }

    // Check last discovery run
    const lastRun = await pool.query(`
      SELECT MAX(discovered_at) as last_discovery
      FROM cls_discovered_channels
    `);

    const lastDiscovery = parseInt(lastRun.rows[0]?.last_discovery || '0');
    const timeSinceDiscovery = Math.floor(Date.now() / 1000) - lastDiscovery;

    if (timeSinceDiscovery > 7200) { // 2 hours
      alerts.push({
        agent: 'Discovery Scout',
        emoji: ':mag:',
        severity: 'warning',
        message: `No CLS discovery run in ${Math.floor(timeSinceDiscovery / 3600)} hours. Discovery cron may be down.`,
        context: { last_discovery: new Date(lastDiscovery * 1000).toISOString() }
      });
    }

    // Check discovery channel count trends
    const recentRuns = await pool.query(`
      SELECT discovery_run_id, COUNT(*) as channels, discovered_at
      FROM cls_discovered_channels
      WHERE discovered_at > extract(epoch from now()) - 86400
      GROUP BY discovery_run_id, discovered_at
      ORDER BY discovered_at DESC
      LIMIT 5
    `);

    if (recentRuns.rows.length > 0) {
      const avgChannels = recentRuns.rows.reduce((sum, r) => sum + parseInt(r.channels), 0) / recentRuns.rows.length;
      const latest = parseInt(recentRuns.rows[0].channels);

      if (latest < avgChannels * 0.5) {
        alerts.push({
          agent: 'Discovery Scout',
          emoji: ':mag:',
          severity: 'warning',
          message: `Latest discovery run found only ${latest} channels (avg: ${avgChannels.toFixed(0)}). Twitch API issue?`,
        });
      }
    }

  } catch (err: any) {
    // Don't alert on errors for optional CLS feature
    console.log('Discovery Scout check skipped:', err.message);
  }

  return alerts;
}

/**
 * 💾 Storage Custodian
 * Monitors disk space and prevents catastrophic storage failures
 */
async function storageCustodian(): Promise<Alert[]> {
  const alerts: Alert[] = [];

  try {
    // Check disk usage
    const { stdout } = await execAsync("df -h / | tail -1 | awk '{print $5 \" \" $4}'");
    const [usagePercent, available] = stdout.trim().split(' ');
    const usage = parseInt(usagePercent);

    if (usage >= 90) {
      alerts.push({
        agent: 'Storage Custodian',
        emoji: ':floppy_disk:',
        severity: 'critical',
        message: `CRITICAL: Disk usage at ${usage}%! Only ${available} remaining.`,
        context: { usage_percent: usage, available }
      });
    } else if (usage >= 80) {
      alerts.push({
        agent: 'Storage Custodian',
        emoji: ':floppy_disk:',
        severity: 'warning',
        message: `Disk usage at ${usage}%. ${available} remaining.`,
        context: { usage_percent: usage, available }
      });
    }

    // Check database file size growth rate
    const { stdout: dbSizeOut } = await execAsync(
      "psql \"$DATABASE_URL\" -At -c \"SELECT pg_size_pretty(pg_database_size(current_database())), pg_database_size(current_database())\" 2>/dev/null || echo '0 GB|0'"
    );
    const [prettySize, bytes] = dbSizeOut.trim().split('|');
    const dbGB = parseInt(bytes) / (1024 * 1024 * 1024);

    if (dbGB > 50) {
      alerts.push({
        agent: 'Storage Custodian',
        emoji: ':floppy_disk:',
        severity: 'warning',
        message: `Database size: ${prettySize}. Consider archival strategy.`,
      });
    }

  } catch (err: any) {
    alerts.push({
      agent: 'Storage Custodian',
      emoji: ':floppy_disk:',
      severity: 'critical',
      message: `Storage monitoring failed: ${err.message}`,
    });
  }

  return alerts;
}

/**
 * 🌐 RPC Navigator
 * Monitors Solana RPC endpoint health and latency
 */
async function rpcNavigator(): Promise<Alert[]> {
  const alerts: Alert[] = [];

  try {
    const rpcUrls = [
      process.env.RPC_URL,
      process.env.PUBLISHER_RPC_URLS?.split(',')[0],
      process.env.AGGREGATOR_RPC_URLS?.split(',')[0],
    ].filter(Boolean) as string[];

    if (rpcUrls.length === 0) {
      rpcUrls.push('https://api.mainnet-beta.solana.com');
    }

    const uniqueRpcs = [...new Set(rpcUrls)];
    let failedRpcs = 0;
    let slowRpcs = 0;

    for (const rpcUrl of uniqueRpcs.slice(0, 3)) { // Check first 3
      try {
        const start = Date.now();
        const response = await fetch(rpcUrl, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            jsonrpc: '2.0',
            id: 1,
            method: 'getHealth'
          }),
          signal: AbortSignal.timeout(10000),
        });

        const latency = Date.now() - start;

        if (!response.ok) {
          failedRpcs++;
        } else if (latency > 3000) {
          slowRpcs++;
        }
      } catch (err: any) {
        failedRpcs++;
      }
    }

    if (failedRpcs === uniqueRpcs.length) {
      alerts.push({
        agent: 'RPC Navigator',
        emoji: ':globe_with_meridians:',
        severity: 'critical',
        message: `ALL RPC endpoints unreachable! Publishing blocked.`,
        context: { checked_rpcs: uniqueRpcs.length, failed: failedRpcs }
      });
    } else if (failedRpcs > 0) {
      alerts.push({
        agent: 'RPC Navigator',
        emoji: ':globe_with_meridians:',
        severity: 'warning',
        message: `${failedRpcs}/${uniqueRpcs.length} RPC endpoints failing. Failover active.`,
        context: { failed_count: failedRpcs, total_count: uniqueRpcs.length }
      });
    }

    if (slowRpcs > uniqueRpcs.length / 2) {
      alerts.push({
        agent: 'RPC Navigator',
        emoji: ':globe_with_meridians:',
        severity: 'warning',
        message: `${slowRpcs} RPC endpoint(s) experiencing high latency (>3s)`,
      });
    }

  } catch (err: any) {
    alerts.push({
      agent: 'RPC Navigator',
      emoji: ':globe_with_meridians:',
      severity: 'critical',
      message: `RPC monitoring failed: ${err.message}`,
    });
  }

  return alerts;
}

/**
 * Main monitoring orchestrator
 */
async function main() {
  console.log('🤖 TWZRD AI Monitoring Agents - Health Check Starting...\n');

  const pool = new Pool({
    connectionString: DATABASE_URL,
    max: 5,
    idleTimeoutMillis: 30000,
    connectionTimeoutMillis: 5000,
  });

  try {
    const allAlerts: Alert[] = [];

    // Run all agents in parallel
    const [dboAlerts, sentinelAlerts, watchdogAlerts, angelAlerts, scoutAlerts, storageAlerts, rpcAlerts] = await Promise.all([
      chiefDatabaseOfficer(pool),
      epochSentinel(pool),
      publisherWatchdog(pool),
      twitchGuardianAngel(),
      discoveryScout(pool),
      storageCustodian(),
      rpcNavigator(),
    ]);

    allAlerts.push(...dboAlerts, ...sentinelAlerts, ...watchdogAlerts, ...angelAlerts, ...scoutAlerts, ...storageAlerts, ...rpcAlerts);

    // Print summary
    console.log('='.repeat(60));
    console.log(`Total alerts: ${allAlerts.length}`);
    console.log(`  Critical: ${allAlerts.filter(a => a.severity === 'critical').length}`);
    console.log(`  Warning:  ${allAlerts.filter(a => a.severity === 'warning').length}`);
    console.log(`  Info:     ${allAlerts.filter(a => a.severity === 'info').length}`);
    console.log('='.repeat(60));

    // Send alerts to Slack
    for (const alert of allAlerts) {
      console.log(`${alert.emoji} [${alert.severity.toUpperCase()}] ${alert.agent}: ${alert.message}`);
      await slack(alert);
    }

    if (allAlerts.length === 0) {
      console.log('✅ All systems healthy!');

      // Send periodic "all clear" to Slack (only once per day)
      const lastAllClearFile = '/tmp/twzrd-last-allclear';
      try {
        const fs = await import('fs/promises');
        const lastAllClear = await fs.readFile(lastAllClearFile, 'utf8').catch(() => '0');
        const timeSinceAllClear = Date.now() / 1000 - parseInt(lastAllClear);

        if (timeSinceAllClear > 86400) { // 24 hours
          await slack({
            agent: 'System Health',
            emoji: ':white_check_mark:',
            severity: 'info',
            message: 'All TWZRD monitoring agents report healthy status. Everything is running smoothly!',
          });
          await fs.writeFile(lastAllClearFile, String(Math.floor(Date.now() / 1000)));
        }
      } catch {}
    }

    // Exit with error code if critical alerts exist
    const hasCritical = allAlerts.some(a => a.severity === 'critical');
    process.exitCode = hasCritical ? 1 : 0;

  } catch (err: any) {
    console.error('Monitoring orchestrator failed:', err);
    await slack({
      agent: 'Monitoring System',
      emoji: ':robot_face:',
      severity: 'critical',
      message: `Monitoring orchestrator crashed: ${err.message}`,
    });
    process.exit(2);
  } finally {
    await pool.end();
  }
}

main().catch(console.error);
