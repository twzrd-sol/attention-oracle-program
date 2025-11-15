#!/usr/bin/env npx tsx
/**
 * Off-Chain MILO Infrastructure Monitoring
 * Runs continuous health checks every 30 minutes
 */

import { Pool } from 'pg';
import { execSync } from 'child_process';
import * as fs from 'fs';

const DB_URL = process.env.DATABASE_URL!;
const CHECK_INTERVAL = 30 * 60 * 1000; // 30 minutes
const LOG_FILE = '/home/twzrd/.pm2/logs/off-chain-monitor.log';

interface HealthStatus {
  timestamp: string;
  processes: ProcessStatus[];
  database: DatabaseStatus;
  epochs: EpochStatus;
  participants: ParticipantStatus;
  resources: ResourceStatus;
  alerts: string[];
}

interface ProcessStatus {
  name: string;
  status: string;
  uptime: string;
  restarts: number;
  memory: string;
  cpu: string;
}

interface DatabaseStatus {
  connected: boolean;
  eventsLastHour: number;
  latestEpoch: number;
  latestEpochAge: string;
}

interface EpochStatus {
  recentEpochs: number;
  allPublished: boolean;
  publishedCount: number;
}

interface ParticipantStatus {
  avgParticipants: number;
  minParticipants: number;
  maxParticipants: number;
}

interface ResourceStatus {
  memoryFree: string;
  memoryAvailable: string;
  cpuLoad: number;
}

function log(msg: string) {
  const timestamp = new Date().toISOString();
  const logLine = `[${timestamp}] ${msg}`;
  console.log(logLine);
  fs.appendFileSync(LOG_FILE, logLine + '\n');
}

function executeCommand(cmd: string): string {
  try {
    return execSync(cmd, { encoding: 'utf-8' }).trim();
  } catch (e) {
    return 'ERROR';
  }
}

async function getProcessStatus(): Promise<ProcessStatus[]> {
  // Use grep to parse pm2 list output directly
  try {
    const processes = ['cls-worker', 'milo-aggregator', 'milo-worker-v2', 'stream-listener', 'tree-builder', 'gateway'];
    return processes.map(name => {
      const listOutput = executeCommand(`pm2 list 2>/dev/null | grep "${name}" || true`);

      // Parse pm2 list table output
      // Example: │ 6  │ cls-worker           │ default     │ 0.1.0   │ fork    │ 2845790  │ 14m    │ 130  │ online    │ 0%       │ 80.6mb   │ twzrd    │ disabled │
      const statusMatch = listOutput.match(/online|stopped|errored|crashed/);
      const restartsMatch = listOutput.match(/\│\s*(\d+)\s*\│[^│]*\│[^│]*\│[^│]*\│[^│]*\│[^│]*\│\s*(\d+)\s*\│/);
      const memoryMatch = listOutput.match(/(\d+\.?\d*)mb/i);

      return {
        name,
        status: statusMatch ? statusMatch[0] : 'unknown',
        uptime: 'N/A',
        restarts: restartsMatch ? parseInt(restartsMatch[2]) : 0,
        memory: memoryMatch ? memoryMatch[1] + 'MB' : 'N/A',
        cpu: 'N/A'
      };
    });
  } catch (e) {
    return [];
  }
}

async function getDatabaseStatus(): Promise<DatabaseStatus> {
  const pool = new Pool({
    connectionString: DB_URL,
    ssl: { rejectUnauthorized: false }
  });

  try {
    const res = await pool.query('SELECT COUNT(*) as count FROM channel_participation WHERE epoch >= $1', [
      Math.floor(Date.now() / 1000) - 3600
    ]);
    const eventsLastHour = parseInt(res.rows[0].count);

    const epochRes = await pool.query('SELECT MAX(epoch) as latest FROM sealed_epochs');
    const latestEpoch = epochRes.rows[0].latest;
    const epochAge = Math.floor((Date.now() / 1000 - latestEpoch) / 60);

    return {
      connected: true,
      eventsLastHour,
      latestEpoch,
      latestEpochAge: `${epochAge} minutes ago`
    };
  } catch (e) {
    return {
      connected: false,
      eventsLastHour: 0,
      latestEpoch: 0,
      latestEpochAge: 'N/A'
    };
  } finally {
    await pool.end();
  }
}

async function getEpochStatus(): Promise<EpochStatus> {
  const pool = new Pool({
    connectionString: DB_URL,
    ssl: { rejectUnauthorized: false }
  });

  try {
    const res = await pool.query('SELECT COUNT(*) as count, SUM(CASE WHEN published THEN 1 ELSE 0 END) as published FROM sealed_epochs WHERE sealed_at > $1', [
      Math.floor(Date.now() / 1000) - 7200
    ]);

    return {
      recentEpochs: parseInt(res.rows[0].count),
      publishedCount: parseInt(res.rows[0].published || 0),
      allPublished: parseInt(res.rows[0].published || 0) === parseInt(res.rows[0].count)
    };
  } catch (e) {
    return {
      recentEpochs: 0,
      publishedCount: 0,
      allPublished: false
    };
  } finally {
    await pool.end();
  }
}

async function getParticipantStatus(): Promise<ParticipantStatus> {
  const pool = new Pool({
    connectionString: DB_URL,
    ssl: { rejectUnauthorized: false }
  });

  try {
    const res = await pool.query(`
      SELECT
        COUNT(sp.user_hash)::int as participant_count
      FROM sealed_epochs se
      LEFT JOIN sealed_participants sp ON se.epoch = sp.epoch AND se.channel = sp.channel
      WHERE se.sealed_at > $1
    `, [Math.floor(Date.now() / 1000) - 7200]);

    const counts = res.rows[0].participant_count || 0;
    return {
      avgParticipants: counts > 0 ? Math.round(counts / 15) : 0,
      minParticipants: counts,
      maxParticipants: counts
    };
  } catch (e) {
    return {
      avgParticipants: 0,
      minParticipants: 0,
      maxParticipants: 0
    };
  } finally {
    await pool.end();
  }
}

function getResourceStatus(): ResourceStatus {
  const memOutput = executeCommand('free -h | grep Mem');
  const topOutput = executeCommand('top -bn1 | grep "load average"');

  const memMatch = memOutput.match(/(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)/);
  const cpuMatch = topOutput.match(/load average: ([\d.]+),/);

  return {
    memoryFree: memMatch ? memMatch[4] : 'N/A',
    memoryAvailable: memMatch ? memMatch[6] : 'N/A',
    cpuLoad: cpuMatch ? parseFloat(cpuMatch[1]) : 0
  };
}

function generateAlerts(status: HealthStatus): string[] {
  const alerts: string[] = [];
  const { processes, database, epochs, participants, resources } = status;

  // Process checks
  const offlineProcesses = processes.filter(p => p.status !== 'online');
  if (offlineProcesses.length > 0) {
    alerts.push(`❌ CRITICAL: ${offlineProcesses.length} processes offline: ${offlineProcesses.map(p => p.name).join(', ')}`);
  }

  const highRestarts = processes.filter(p => p.restarts > 50);
  if (highRestarts.length > 0) {
    alerts.push(`⚠️ WARNING: High restart counts: ${highRestarts.map(p => `${p.name}(${p.restarts})`).join(', ')}`);
  }

  // Database checks
  if (!database.connected) {
    alerts.push('❌ CRITICAL: Database unreachable');
  }
  if (database.eventsLastHour === 0) {
    alerts.push('⚠️ WARNING: No new data in last hour');
  }

  // Epoch checks
  if (database.latestEpoch === 0) {
    alerts.push('⚠️ WARNING: No sealed epochs found');
  }
  const epochAgeMinutes = parseInt(database.latestEpochAge);
  if (epochAgeMinutes > 120) {
    alerts.push(`⚠️ WARNING: Latest epoch is ${epochAgeMinutes} minutes old (>2 hours)`);
  }

  // Participant checks
  if (participants.avgParticipants === 0) {
    alerts.push('⚠️ WARNING: Zero participants detected');
  }

  // Resource checks
  if (resources.cpuLoad > 90) {
    alerts.push(`⚠️ WARNING: CPU load very high: ${resources.cpuLoad}`);
  }

  return alerts;
}

async function runHealthCheck(): Promise<HealthStatus> {
  const status: HealthStatus = {
    timestamp: new Date().toISOString(),
    processes: await getProcessStatus(),
    database: await getDatabaseStatus(),
    epochs: await getEpochStatus(),
    participants: await getParticipantStatus(),
    resources: getResourceStatus(),
    alerts: []
  };

  status.alerts = generateAlerts(status);
  return status;
}

function formatReport(status: HealthStatus): string {
  let report = `\n================== Off-Chain Health Report ==================\nTimestamp: ${status.timestamp}\n\n`;

  report += '### Process Status\n';
  status.processes.forEach(p => {
    const icon = p.status === 'online' ? '✅' : '❌';
    report += `${icon} ${p.name.padEnd(20)} status=${p.status} uptime=${p.uptime} restarts=${p.restarts} mem=${p.memory} cpu=${p.cpu}\n`;
  });

  report += '\n### Data Flow\n';
  report += `✅ Events (last hour): ${status.database.eventsLastHour}\n`;
  report += `✅ Latest sealed epoch: ${status.database.latestEpoch} (${status.database.latestEpochAge})\n`;
  report += `✅ Recent epochs: ${status.epochs.recentEpochs}, Published: ${status.epochs.publishedCount}\n`;
  report += `✅ Avg participants/epoch: ${status.participants.avgParticipants}\n`;

  report += '\n### Resource Usage\n';
  report += `Memory: Free=${status.resources.memoryFree}, Available=${status.resources.memoryAvailable}\n`;
  report += `CPU Load: ${status.resources.cpuLoad}\n`;

  if (status.alerts.length > 0) {
    report += '\n### ⚠️  ALERTS\n';
    status.alerts.forEach(a => report += `${a}\n`);
  } else {
    report += '\n### Status: ALL GREEN ✅\n';
  }

  report += '\n=============================================================\n';
  return report;
}

async function main() {
  log('Off-Chain Monitor Started');

  while (true) {
    try {
      process.env.NODE_TLS_REJECT_UNAUTHORIZED = '0';
      const status = await runHealthCheck();
      const report = formatReport(status);
      console.log(report);
      log(`Health check completed. Alerts: ${status.alerts.length}`);

      if (status.alerts.some(a => a.includes('CRITICAL'))) {
        log('🚨 CRITICAL ALERT DETECTED - Check logs for details');
      }
    } catch (e) {
      log(`ERROR during health check: ${e instanceof Error ? e.message : String(e)}`);
    }

    log(`Next health check in 30 minutes...`);
    await new Promise(resolve => setTimeout(resolve, CHECK_INTERVAL));
  }
}

main().catch(e => {
  log(`Fatal error: ${e instanceof Error ? e.message : String(e)}`);
  process.exit(1);
});
