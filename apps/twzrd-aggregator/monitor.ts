#!/usr/bin/env npx tsx
/**
 * Off-Chain Monitoring Script for CHAT Token Protocol
 *
 * Monitors:
 * - PM2 process health
 * - Database connectivity & data flow
 * - Epoch sealing cadence
 * - Participant counts
 * - Resource usage
 *
 * Runs continuously, reports every 30 minutes or on alert
 */

import 'dotenv/config';
import { exec } from 'child_process';
import { promisify } from 'util';
import { Pool } from 'pg';
import * as fs from 'fs';
import * as path from 'path';

const execAsync = promisify(exec);

// Configuration
const CONFIG = {
  // Interval (minutes) can be overridden via env MONITOR_INTERVAL_MINUTES
  checkInterval: (parseInt(process.env.MONITOR_INTERVAL_MINUTES || '10', 10)) * 60 * 1000,
  alertInterval: 2 * 60 * 1000,  // Check alerts every 2 minutes
  logFile: path.join(process.env.HOME || '/home/twzrd', '.pm2/monitor.log'),

  thresholds: {
    memoryWarning: 3 * 1024 * 1024 * 1024,      // 3GB
    memoryCritical: 4 * 1024 * 1024 * 1024,     // 4GB
    cpuCritical: 90,                             // 90%
    epochStaleness: 2 * 3600,                    // 2 hours
    epochCritical: 4 * 3600,                     // 4 hours
    eventsPerHourMin: 100,                       // Minimum events expected per hour
    participantDropPercent: 50,                  // Alert if >50% drop
    restartRateWarning: 20,                      // Restarts/hour
  },

  // Track real PM2 process names in production
  processes: [
    'cls-aggregator',
    'tree-builder',
    'cls-worker-s0',
    'cls-worker-s1',
    'cls-worker-s2',
    'epoch-watcher',
    'gateway',
    'stream-listener',
  ],
};

// Simple webhook alerts (Slack/Discord compatible)
const ALERT_WEBHOOK_URL = (process.env.ALERT_WEBHOOK_URL || '').trim();
const ALERT_MIN_SEVERITY = (process.env.ALERT_MIN_SEVERITY || 'WARNING').toUpperCase() as 'INFO' | 'WARNING' | 'CRITICAL';
const severityRank = { INFO: 0, WARNING: 1, CRITICAL: 2 } as const;

async function sendAlertWebhook(summary: string, alerts: Alert[]): Promise<void> {
  if (!ALERT_WEBHOOK_URL || alerts.length === 0) return;
  const worst = alerts.reduce((a, b) => (severityRank[a.severity] >= severityRank[b.severity] ? a : b));
  if (severityRank[worst.severity] < severityRank[ALERT_MIN_SEVERITY]) return;
  const timestamp = new Date().toISOString();
  const text = `[offchain] ${worst.severity} @ ${timestamp}\n${summary}\n` + alerts.map(a => `- [${a.severity}] ${a.component}: ${a.message}`).join('\n');
  try {
    await fetch(ALERT_WEBHOOK_URL, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ text, content: text }), // Slack uses text; Discord uses content
    });
  } catch (e) {
    // Non-fatal: log failure but don't crash monitor
    console.error('❌ Failed to send alert webhook:', (e as Error)?.message || e);
  }
}

// Types
interface ProcessStatus {
  name: string;
  status: 'online' | 'offline' | 'errored' | 'stopped';
  uptime: number;
  restarts: number;
  cpu: number;
  memory: number;
  pid: number;
}

interface DatabaseHealth {
  connected: boolean;
  eventsLastHour: number;
  latestEpoch: number;
  latestEpochAge: number;
}

interface EpochData {
  epoch: number;
  channel: string;
  participantCount: number;
  sealed: string;
  published: boolean;
}

interface HealthReport {
  timestamp: string;
  processes: ProcessStatus[];
  database: DatabaseHealth;
  recentEpochs: EpochData[];
  alerts: Alert[];
  systemResources: {
    memoryUsed: string;
    memoryFree: string;
    cpuLoad: string;
  };
}

interface Alert {
  severity: 'CRITICAL' | 'WARNING' | 'INFO';
  component: string;
  message: string;
  timestamp: string;
}

// Colors for output
const colors = {
  reset: '\x1b[0m',
  bright: '\x1b[1m',
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  green: '\x1b[32m',
  blue: '\x1b[36m',
};

// Alert tracking to avoid spam
const alertCache = new Map<string, number>();
const ALERT_SUPPRESS_DURATION = 30 * 60 * 1000; // 30 minutes

/**
 * Check PM2 process status
 */
async function checkProcessStatus(): Promise<ProcessStatus[]> {
  try {
    const { stdout } = await execAsync('pm2 jlist');
    const processes = JSON.parse(stdout);

    return CONFIG.processes
      .map((name) => {
        const proc = processes.find((p: any) => p.name === name);
        if (!proc) {
          return {
            name,
            status: 'offline',
            uptime: 0,
            restarts: 0,
            cpu: 0,
            memory: 0,
            pid: 0,
          };
        }
        const pmUptime = proc.pm2_env?.pm_uptime || 0; // ms since epoch when started
        const uptime = pmUptime ? Date.now() - pmUptime : 0; // ms running
        return {
          name: proc.name,
          status: proc.pm2_env?.status || 'unknown',
          uptime,
          restarts: proc.pm2_env?.restart_time ?? proc.restart_time ?? 0,
          cpu: proc.monit?.cpu || 0,
          memory: proc.monit?.memory || 0,
          pid: proc.pid,
        };
      });
  } catch (error) {
    console.error('❌ Failed to check PM2 status:', error);
    return CONFIG.processes.map((name) => ({
      name,
      status: 'unknown',
      uptime: 0,
      restarts: 0,
      cpu: 0,
      memory: 0,
      pid: 0,
    }));
  }
}

/**
 * Check database health and data flow
 */
async function checkDatabaseHealth(): Promise<DatabaseHealth> {
  // Normalize DATABASE_URL to avoid sslmode conflicts and self-signed errors
  const raw = process.env.DATABASE_URL || '';
  const u = new URL(raw);
  u.searchParams.delete('sslmode');
  const pool = new Pool({
    host: u.hostname,
    port: u.port ? parseInt(u.port, 10) : 5432,
    user: decodeURIComponent(u.username),
    password: decodeURIComponent(u.password),
    database: decodeURIComponent(u.pathname.replace(/^\//, '')),
    ssl: (u.hostname === 'localhost' || u.hostname === '127.0.0.1') ? false : { rejectUnauthorized: false },
  });

  try {
    // Check connection
    await pool.query('SELECT 1');

    // Check events in last hour
    const oneHourAgo = Math.floor(Date.now() / 1000) - 3600;
    const eventsRes = await pool.query(
      'SELECT COUNT(*) as count FROM channel_participation WHERE epoch >= $1',
      [oneHourAgo]
    );
    const eventsLastHour = parseInt(eventsRes.rows[0].count, 10);

    // Check latest sealed epoch
    const epochRes = await pool.query('SELECT epoch, sealed_at FROM sealed_epochs ORDER BY sealed_at DESC LIMIT 1');
    const latestEpoch = epochRes.rows[0]?.epoch || 0;
    const latestEpochTime = epochRes.rows[0]?.sealed_at ? new Date(epochRes.rows[0].sealed_at * 1000).getTime() : 0;
    const latestEpochAge = latestEpochTime ? Math.floor((Date.now() - latestEpochTime) / 1000) : 0;

    return {
      connected: true,
      eventsLastHour,
      latestEpoch,
      latestEpochAge,
    };
  } catch (error) {
    console.error('❌ Database error:', error);
    return {
      connected: false,
      eventsLastHour: 0,
      latestEpoch: 0,
      latestEpochAge: 999999,
    };
  } finally {
    await pool.end();
  }
}

/**
 * Check recent epoch data
 */
async function checkRecentEpochs(): Promise<EpochData[]> {
  const raw = process.env.DATABASE_URL || '';
  const u = new URL(raw);
  u.searchParams.delete('sslmode');
  const pool = new Pool({
    host: u.hostname,
    port: u.port ? parseInt(u.port, 10) : 5432,
    user: decodeURIComponent(u.username),
    password: decodeURIComponent(u.password),
    database: decodeURIComponent(u.pathname.replace(/^\//, '')),
    ssl: (u.hostname === 'localhost' || u.hostname === '127.0.0.1') ? false : { rejectUnauthorized: false },
  });

  try {
    const res = await pool.query(`
      SELECT
        se.epoch,
        se.channel,
        COUNT(sp.user_hash) as participant_count,
        TO_CHAR(TO_TIMESTAMP(se.sealed_at), 'YYYY-MM-DD HH24:MI:SS') as sealed,
        se.published
      FROM sealed_epochs se
      LEFT JOIN sealed_participants sp ON se.epoch = sp.epoch AND se.channel = sp.channel
      WHERE se.sealed_at > (EXTRACT(EPOCH FROM NOW()) - 7200)::int
      GROUP BY se.epoch, se.channel, se.sealed_at, se.published
      ORDER BY se.epoch DESC
      LIMIT 20
    `);

    return res.rows.map((r: any) => ({
      epoch: r.epoch,
      channel: r.channel,
      participantCount: parseInt(r.participant_count, 10),
      sealed: r.sealed,
      published: r.published,
    }));
  } catch (error) {
    console.error('❌ Failed to check epochs:', error);
    return [];
  } finally {
    await pool.end();
  }
}

/**
 * Check system resources
 */
async function checkSystemResources(): Promise<{ memoryUsed: string; memoryFree: string; cpuLoad: string }> {
  try {
    const { stdout: freeOutput } = await execAsync('free -h');
    const lines = freeOutput.split('\n');
    const memLine = lines[1].split(/\s+/);
    const memoryUsed = memLine[2];
    const memoryFree = memLine[6];

    const { stdout: loadOutput } = await execAsync('cat /proc/loadavg');
    const cpuLoad = loadOutput.split(' ').slice(0, 3).join(', ');

    return { memoryUsed, memoryFree, cpuLoad };
  } catch (error) {
    return { memoryUsed: 'unknown', memoryFree: 'unknown', cpuLoad: 'unknown' };
  }
}

/**
 * Generate alerts based on thresholds
 */
function generateAlerts(
  processes: ProcessStatus[],
  database: DatabaseHealth,
  epochs: EpochData[]
): Alert[] {
  const alerts: Alert[] = [];
  const now = new Date().toISOString();

  // Check process status
  const offlineProcesses = processes.filter((p) => p.status !== 'online');
  if (offlineProcesses.length === processes.length) {
    alerts.push({
      severity: 'CRITICAL',
      component: 'processes',
      message: 'All workers offline',
      timestamp: now,
    });
  } else if (offlineProcesses.length > 0) {
    alerts.push({
      severity: 'WARNING',
      component: 'processes',
      message: `${offlineProcesses.map((p) => p.name).join(', ')} offline`,
      timestamp: now,
    });
  }

  // Check restart rates
  processes.forEach((p) => {
    const restartRate = p.restarts; // Rough estimate
    if (restartRate > CONFIG.thresholds.restartRateWarning) {
      alerts.push({
        severity: 'WARNING',
        component: 'processes',
        message: `${p.name} restarted ${restartRate} times (threshold: ${CONFIG.thresholds.restartRateWarning}/hour)`,
        timestamp: now,
      });
    }
  });

  // Check memory usage
  processes.forEach((p) => {
    if (p.memory > CONFIG.thresholds.memoryCritical) {
      alerts.push({
        severity: 'CRITICAL',
        component: p.name,
        message: `Memory usage critical: ${(p.memory / 1024 / 1024 / 1024).toFixed(2)}GB (threshold: 4GB)`,
        timestamp: now,
      });
    } else if (p.memory > CONFIG.thresholds.memoryWarning) {
      alerts.push({
        severity: 'WARNING',
        component: p.name,
        message: `High memory usage: ${(p.memory / 1024 / 1024 / 1024).toFixed(2)}GB (threshold: 3GB)`,
        timestamp: now,
      });
    }
  });

  // Check CPU usage
  processes.forEach((p) => {
    if (p.cpu > CONFIG.thresholds.cpuCritical) {
      alerts.push({
        severity: 'CRITICAL',
        component: p.name,
        message: `CPU usage critical: ${p.cpu}% (threshold: ${CONFIG.thresholds.cpuCritical}%)`,
        timestamp: now,
      });
    }
  });

  // Check database connectivity
  if (!database.connected) {
    alerts.push({
      severity: 'CRITICAL',
      component: 'database',
      message: 'Database unreachable',
      timestamp: now,
    });
  }

  // Check data flow
  if (database.eventsLastHour < CONFIG.thresholds.eventsPerHourMin) {
    alerts.push({
      severity: 'WARNING',
      component: 'database',
      message: `Low event rate: ${database.eventsLastHour} events in last hour (expected: >${CONFIG.thresholds.eventsPerHourMin})`,
      timestamp: now,
    });
  }

  // Check epoch staleness
  if (database.latestEpochAge > CONFIG.thresholds.epochCritical) {
    alerts.push({
      severity: 'CRITICAL',
      component: 'sealing',
      message: `No sealed epochs in ${Math.floor(database.latestEpochAge / 3600)} hours (threshold: ${CONFIG.thresholds.epochCritical / 3600} hours)`,
      timestamp: now,
    });
  } else if (database.latestEpochAge > CONFIG.thresholds.epochStaleness) {
    alerts.push({
      severity: 'WARNING',
      component: 'sealing',
      message: `Stale epochs: latest is ${Math.floor(database.latestEpochAge / 60)} minutes old (threshold: ${CONFIG.thresholds.epochStaleness / 60} minutes)`,
      timestamp: now,
    });
  }

  // Check participant counts
  const avgParticipants =
    epochs.length > 0
      ? epochs.reduce((sum, e) => sum + e.participantCount, 0) / epochs.length
      : 0;

  epochs.forEach((epoch) => {
    if (epoch.participantCount === 0) {
      alerts.push({
        severity: 'WARNING',
        component: 'sealing',
        message: `Zero participants in epoch ${epoch.epoch} on ${epoch.channel}`,
        timestamp: now,
      });
    }
  });

  return alerts;
}

/**
 * Format and display health report
 */
function formatReport(report: HealthReport): string {
  const sep = '═'.repeat(80);
  let output = `\n${sep}\n`;
  output += `${colors.bright}OFF-CHAIN HEALTH REPORT${colors.reset}\n`;
  output += `Timestamp: ${report.timestamp}\n`;
  output += `${sep}\n\n`;

  // Process Status
  output += `${colors.bright}📊 PROCESS STATUS${colors.reset}\n`;
  output += '─'.repeat(80) + '\n';
  report.processes.forEach((p) => {
    const statusColor =
      p.status === 'online' ? colors.green : p.status === 'offline' ? colors.red : colors.yellow;
    const statusIcon = p.status === 'online' ? '✅' : p.status === 'offline' ? '❌' : '⚠️';
    output += `${statusIcon} ${colors.bright}${p.name}${colors.reset}\n`;
    output += `   Status: ${statusColor}${p.status}${colors.reset} | Uptime: ${formatUptime(p.uptime)} | Restarts: ${p.restarts}\n`;
    output += `   Memory: ${(p.memory / 1024 / 1024).toFixed(0)}MB | CPU: ${p.cpu}%\n\n`;
  });

  // Database Health
  output += `${colors.bright}🗄️  DATABASE HEALTH${colors.reset}\n`;
  output += '─'.repeat(80) + '\n';
  if (report.database.connected) {
    output += `${colors.green}✅ Connected${colors.reset}\n`;
    output += `Events (last hour): ${report.database.eventsLastHour}\n`;
    output += `Latest epoch: ${report.database.latestEpoch} (${report.database.latestEpochAge} seconds old)\n\n`;
  } else {
    output += `${colors.red}❌ Disconnected${colors.reset}\n\n`;
  }

  // Recent Epochs
  if (report.recentEpochs.length > 0) {
    output += `${colors.bright}📈 RECENT EPOCHS${colors.reset}\n`;
    output += '─'.repeat(80) + '\n';
    output += 'Epoch    Channel         Participants  Sealed              Published\n';
    output += '─'.repeat(80) + '\n';
    report.recentEpochs.slice(0, 10).forEach((e) => {
      const pubIcon = e.published ? '✅' : '⏳';
      output += `${String(e.epoch).padEnd(8)} ${e.channel.padEnd(15)} ${String(e.participantCount).padEnd(12)} ${e.sealed.padEnd(19)} ${pubIcon}\n`;
    });
    output += '\n';
  }

  // Alerts
  if (report.alerts.length > 0) {
    output += `${colors.bright}🚨 ALERTS${colors.reset}\n`;
    output += '─'.repeat(80) + '\n';
    report.alerts.forEach((alert) => {
      let alertIcon = '⚠️';
      let alertColor = colors.yellow;
      if (alert.severity === 'CRITICAL') {
        alertIcon = '❌';
        alertColor = colors.red;
      } else if (alert.severity === 'INFO') {
        alertIcon = 'ℹ️';
        alertColor = colors.blue;
      }
      output += `${alertIcon} ${alertColor}[${alert.severity}]${colors.reset} ${alert.component}: ${alert.message}\n`;
    });
    output += '\n';
  } else {
    output += `${colors.green}✅ No alerts${colors.reset}\n\n`;
  }

  // System Resources
  output += `${colors.bright}💾 SYSTEM RESOURCES${colors.reset}\n`;
  output += '─'.repeat(80) + '\n';
  output += `Memory: ${report.systemResources.memoryUsed} used | ${report.systemResources.memoryFree} free\n`;
  output += `CPU Load (1/5/15 min): ${report.systemResources.cpuLoad}\n`;
  output += `${sep}\n`;

  return output;
}

function formatUptime(milliseconds: number): string {
  const seconds = Math.floor(milliseconds / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);

  if (days > 0) return `${days}d ${hours % 24}h`;
  if (hours > 0) return `${hours}h ${minutes % 60}m`;
  if (minutes > 0) return `${minutes}m`;
  return `${seconds}s`;
}

/**
 * Main monitoring loop
 */
async function monitor() {
  console.log(`${colors.green}✅ Monitoring started at ${new Date().toISOString()}${colors.reset}`);
  console.log(`Check interval: ${CONFIG.checkInterval / 60000} minutes\n`);

  // Ensure .env is loaded
  if (!process.env.DATABASE_URL) {
    console.error(`${colors.red}❌ DATABASE_URL not set. Please source .env first:${colors.reset}`);
    console.error('  source .env');
    process.exit(1);
  }

  let checkCount = 0;

  while (true) {
    try {
      checkCount++;
      const report: HealthReport = {
        timestamp: new Date().toISOString(),
        processes: await checkProcessStatus(),
        database: await checkDatabaseHealth(),
        recentEpochs: await checkRecentEpochs(),
        alerts: [],
        systemResources: await checkSystemResources(),
      };

      report.alerts = generateAlerts(report.processes, report.database, report.recentEpochs);

      // Display report
      const reportText = formatReport(report);
      console.log(reportText);

      // Log to file
      fs.appendFileSync(CONFIG.logFile, reportText);

      // Send webhook if any alert exceeds threshold
      if (report.alerts.length > 0) {
        const shortSummary = `proc: ${report.processes.filter(p=>p.status!=='online').map(p=>p.name).join(',') || 'all-online'} | events/hr: ${report.database.eventsLastHour} | latest_epoch_age_s: ${report.database.latestEpochAge}`;
        await sendAlertWebhook(shortSummary, report.alerts);
      }

      // Check for critical alerts
      const criticalAlerts = report.alerts.filter((a) => a.severity === 'CRITICAL');
      if (criticalAlerts.length > 0) {
        console.error(
          `${colors.red}🚨 CRITICAL ALERTS DETECTED - Check logs for details${colors.reset}\n`
        );

        // Print escalation template
        console.error(`\n${colors.red}ESCALATION TEMPLATE:${colors.reset}`);
        console.error(
          `ESCALATION: Off-Chain Monitoring\n` +
            `Severity: CRITICAL\n` +
            `Timestamp: ${new Date().toISOString()}\n` +
            `Alerts:\n` +
            criticalAlerts.map((a) => `  - ${a.component}: ${a.message}`).join('\n') +
            `\nCheck: tail -f ${CONFIG.logFile}\n`
        );
      }

      // Wait for next check
      console.log(`Next check in ${CONFIG.checkInterval / 60000} minutes...\n`);
      await new Promise((resolve) => setTimeout(resolve, CONFIG.checkInterval));
    } catch (error) {
      console.error(`${colors.red}❌ Monitor error: ${error}${colors.reset}`);
      await new Promise((resolve) => setTimeout(resolve, 5000)); // Wait 5 seconds before retry
    }
  }
}

// Run monitor
monitor().catch((error) => {
  console.error('Fatal error:', error);
  process.exit(1);
});
