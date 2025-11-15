#!/usr/bin/env tsx
/**
 * 💓 Publisher Heartbeat Bot
 * Sends periodic updates on publish activity and backlog drain progress
 * Run via cron every 30 minutes during drain, then hourly
 */

import 'dotenv/config';
import { Pool } from 'pg';

const SLACK_WEBHOOK = process.env.SLACK_WEBHOOK;
const DATABASE_URL = process.env.DATABASE_URL || 'postgresql://twzrd:twzrd_password_2025@localhost:6432/twzrd_oracle';

interface HeartbeatData {
  unpublished_count: number;
  milo_unpublished_count: number;
  published_last_hour: number;
  drain_rate_per_hour: number;
  eta_minutes: number;
  latest_channels: string[];
}

async function getHeartbeatData(pool: Pool): Promise<HeartbeatData> {
  // Get unpublished count
  const unpublished = await pool.query(
    `SELECT COUNT(*) as count FROM sealed_epochs WHERE published IS NULL OR published = 0`
  );
  const unpublished_count = parseInt(unpublished.rows[0]?.count || '0');

  // MILO-only backlog via allowlist
  const miloChannelsCsv = process.env.MILO_CHANNELS || '';
  let milo_unpublished_count = unpublished_count;
  if (miloChannelsCsv) {
    const miloQ = await pool.query(
      `SELECT COUNT(*) as count FROM sealed_epochs
       WHERE published = 0 AND channel = ANY(string_to_array($1, ','))`,
      [miloChannelsCsv]
    );
    milo_unpublished_count = parseInt(miloQ.rows[0]?.count || '0');
  }

  // Get published in last hour
  const now = Math.floor(Date.now() / 1000);
  const oneHourAgo = now - 3600;

  const recentPublished = await pool.query(
    `SELECT COUNT(*) as count FROM sealed_epochs WHERE published_at > to_timestamp($1)`,
    [oneHourAgo]
  );
  const published_last_hour = parseInt(recentPublished.rows[0]?.count || '0');

  // Calculate drain rate (epochs per hour)
  // We started with 882, now at unpublished_count
  const drain_rate_per_hour = published_last_hour; // Simple: last hour's throughput

  // ETA in minutes
  const eta_minutes = drain_rate_per_hour > 0
    ? Math.ceil((unpublished_count / drain_rate_per_hour) * 60)
    : 999;

  // Latest 3 published channels
  const latestChannels = await pool.query(`
    SELECT DISTINCT channel, published_at
    FROM sealed_epochs
    WHERE published_at IS NOT NULL
    ORDER BY published_at DESC
    LIMIT 3
  `);
  const latest_channels = latestChannels.rows.map(r => r.channel);

  return {
    unpublished_count,
    milo_unpublished_count,
    published_last_hour,
    drain_rate_per_hour,
    eta_minutes,
    latest_channels,
  };
}

async function sendHeartbeat(data: HeartbeatData) {
  if (!SLACK_WEBHOOK) {
    console.log('No SLACK_WEBHOOK configured');
    return;
  }

  // Dynamic baseline kept on disk
  const fs = await import('node:fs/promises');
  const path = await import('node:path');
  const baseDir = path.resolve(process.cwd(), 'logs/ops');
  const baseFile = path.join(baseDir, 'heartbeat.baseline');
  try { await fs.mkdir(baseDir, { recursive: true }); } catch {}
  let baseline = data.unpublished_count;
  try {
    const raw = await fs.readFile(baseFile, 'utf-8');
    const n = parseInt(raw.trim() || '0');
    if (Number.isFinite(n) && n > 0) baseline = n;
  } catch {}
  if (data.unpublished_count > baseline) {
    baseline = data.unpublished_count;
    try { await fs.writeFile(baseFile, String(baseline)); } catch {}
  }
  const cleared = baseline - data.unpublished_count;
  const progress = baseline > 0 ? Math.round((cleared / baseline) * 100) : 100;

  let statusEmoji = '💓';
  let statusText = 'Healthy';

  if (data.unpublished_count === 0) {
    statusEmoji = '✅';
    statusText = 'COMPLETE';
  } else if (data.drain_rate_per_hour === 0) {
    statusEmoji = '⚠️';
    statusText = 'STALLED';
  } else if (data.drain_rate_per_hour < 5) {
    statusEmoji = '🐌';
    statusText = 'Slow Drain';
  }

  const message = {
    text: `${statusEmoji} Publisher Heartbeat - ${statusText}`,
    blocks: [
      {
        type: 'section',
        text: {
          type: 'mrkdwn',
          text: `${statusEmoji} *Publisher Heartbeat* - ${statusText}`
        }
      },
      {
        type: 'section',
        fields: [
          {
            type: 'mrkdwn',
            text: `*Progress:*\n${progress}% complete (${cleared}/${baseline})`
          },
          {
            type: 'mrkdwn',
            text: `*Backlog:*\n${data.unpublished_count} total (MILO: ${data.milo_unpublished_count})`
          },
          {
            type: 'mrkdwn',
            text: `*Drain Rate:*\n${data.drain_rate_per_hour}/hour`
          },
          {
            type: 'mrkdwn',
            text: `*ETA:*\n~${data.eta_minutes} min`
          }
        ]
      },
      {
        type: 'context',
        elements: [
          {
            type: 'mrkdwn',
            text: `Latest: ${data.latest_channels.join(', ')} • ${new Date().toISOString()}`
          }
        ]
      }
    ]
  };

  try {
    await fetch(SLACK_WEBHOOK, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(message)
    });
    console.log('✅ Heartbeat sent to Slack');
  } catch (err) {
    console.error('Failed to send heartbeat:', err);
  }
}

async function main() {
  const pool = new Pool({
    connectionString: DATABASE_URL,
    max: 2,
  });

  try {
    const data = await getHeartbeatData(pool);

    console.log('Publisher Heartbeat:');
    console.log(`  Unpublished: ${data.unpublished_count}`);
    console.log(`  Published (last hour): ${data.published_last_hour}`);
    console.log(`  Drain rate: ${data.drain_rate_per_hour}/hour`);
    console.log(`  ETA: ${data.eta_minutes} minutes`);

    await sendHeartbeat(data);
  } finally {
    await pool.end();
  }
}

main().catch(console.error);
