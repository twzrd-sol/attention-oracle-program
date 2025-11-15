#!/usr/bin/env tsx
/**
 * CLS Top 100 Discovery Script
 *
 * Queries Twitch API for the current top 100 streams by viewer count,
 * logs the discovery to PostgreSQL for audit trail, and updates the
 * cls-channels.txt file for dynamic stream-listener subscription.
 *
 * Designed to run hourly via cron/PM2.
 */

import { Pool } from 'pg';
import { randomUUID } from 'crypto';
import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';
import dotenv from 'dotenv';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Load .env from project root
dotenv.config({ path: path.resolve(__dirname, '../../.env') });

// Configuration
const DATABASE_URL = process.env.DATABASE_URL || 'postgresql://twzrd:twzrd_password_2025@localhost:6432/twzrd_oracle';
const TWITCH_CLIENT_ID = process.env.TWITCH_CLIENT_ID!;
const TWITCH_CLIENT_SECRET = process.env.TWITCH_CLIENT_SECRET!;

if (!TWITCH_CLIENT_ID || !TWITCH_CLIENT_SECRET) {
  console.error('[CLS Discovery] ERROR: Missing TWITCH_CLIENT_ID or TWITCH_CLIENT_SECRET in .env');
  process.exit(1);
}
const BLOCKLIST = new Set(
  (process.env.MILO_BLOCKLIST || '').split(',').map((c) => c.trim().toLowerCase()).filter(Boolean)
);
const CLS_CHANNELS_TXT = process.env.CLS_CHANNELS_TXT || './config/cls-channels.txt';
const CLS_CHANNELS_JSON = process.env.CLS_CHANNELS_FILE || './config/cls-channels.json';

interface TwitchStream {
  user_login: string;
  user_name: string;
  viewer_count: number;
  game_name: string;
  title: string;
  started_at: string;
}

interface TwitchAuthResponse {
  access_token: string;
  expires_in: number;
  token_type: string;
}

/**
 * Get Twitch OAuth token for API access
 */
async function getTwitchToken(): Promise<string> {
  const url = 'https://id.twitch.tv/oauth2/token';
  const params = new URLSearchParams({
    client_id: TWITCH_CLIENT_ID,
    client_secret: TWITCH_CLIENT_SECRET,
    grant_type: 'client_credentials',
  });

  const response = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: params,
  });

  if (!response.ok) {
    throw new Error(`Failed to get Twitch token: ${response.statusText}`);
  }

  const data = (await response.json()) as TwitchAuthResponse;
  return data.access_token;
}

/**
 * Fetch top 100 streams from Twitch API, sorted by viewer count
 */
async function fetchTop100Streams(token: string): Promise<TwitchStream[]> {
  const streams: TwitchStream[] = [];
  let cursor: string | undefined;

  // Twitch API returns max 100 per request, we need exactly 100
  const url = 'https://api.twitch.tv/helix/streams';
  const headers = {
    'Client-ID': TWITCH_CLIENT_ID,
    'Authorization': `Bearer ${token}`,
  };

  // First request: get 100 streams
  const params = new URLSearchParams({ first: '100' });
  const response = await fetch(`${url}?${params}`, { headers });

  if (!response.ok) {
    throw new Error(`Twitch API error: ${response.statusText}`);
  }

  const data = await response.json();
  streams.push(...(data.data as TwitchStream[]));

  console.log(`[CLS Discovery] Fetched ${streams.length} streams from Twitch API`);

  // Sort by viewer_count descending (API should already do this, but ensure)
  streams.sort((a, b) => b.viewer_count - a.viewer_count);

  // Take top 100
  return streams.slice(0, 100);
}

/**
 * Insert discovered channels into PostgreSQL audit table
 */
async function recordDiscovery(
  pool: Pool,
  streams: TwitchStream[],
  discoveryRunId: string,
  timestamp: number
): Promise<void> {
  const query = `
    INSERT INTO cls_discovered_channels
    (channel_name, viewer_count, rank, discovered_at, discovery_run_id, metadata, category)
    VALUES ($1, $2, $3, $4, $5, $6, $7)
  `;

  // Batch insert for efficiency
  const client = await pool.connect();
  try {
    await client.query('BEGIN');

    for (let i = 0; i < streams.length; i++) {
      const stream = streams[i];
      const category = stream.game_name || 'Just Chatting'; // Default to Just Chatting if no game
      const metadata = {
        game_name: stream.game_name,
        title: stream.title,
        started_at: stream.started_at,
        user_name: stream.user_name,
      };

      await client.query(query, [
        stream.user_login.toLowerCase(),
        stream.viewer_count,
        i + 1, // Rank: 1-100
        timestamp,
        discoveryRunId,
        JSON.stringify(metadata),
        category,
      ]);
    }

    await client.query('COMMIT');
    console.log(`[CLS Discovery] Recorded ${streams.length} channels to database (run_id: ${discoveryRunId})`);
  } catch (error) {
    await client.query('ROLLBACK');
    throw error;
  } finally {
    client.release();
  }
}

/**
 * Apply blocklist filtering
 */
function applyBlocklist(streams: TwitchStream[]): TwitchStream[] {
  const filtered = streams.filter((s) => !BLOCKLIST.has(s.user_login.toLowerCase()));
  const removed = streams.length - filtered.length;

  if (removed > 0) {
    console.log(`[CLS Discovery] Blocklist removed ${removed} channels`);
  }

  return filtered;
}

/**
 * Write channel list to config files
 */
function writeChannelFilesFromLogins(channelNames: string[]): void {

  // Write to .txt (one per line, for stream-listener)
  const txtPath = path.resolve(process.cwd(), CLS_CHANNELS_TXT);
  fs.writeFileSync(txtPath, channelNames.join('\n') + '\n', 'utf8');
  console.log(`[CLS Discovery] Wrote ${channelNames.length} channels to ${txtPath}`);

  // Write to .json (array format, for other tools)
  const jsonPath = path.resolve(process.cwd(), CLS_CHANNELS_JSON);
  fs.writeFileSync(jsonPath, JSON.stringify(channelNames, null, 2) + '\n', 'utf8');
  console.log(`[CLS Discovery] Wrote ${channelNames.length} channels to ${jsonPath}`);
}

/**
 * Read current adopted channels (if any) from cls-channels.txt to avoid churn
 */
function readCurrentAdopted(): Set<string> {
  try {
    const txtPath = path.resolve(process.cwd(), CLS_CHANNELS_TXT);
    const content = fs.readFileSync(txtPath, 'utf8');
    return new Set(content.split(/\r?\n/).map(s => s.trim().toLowerCase()).filter(Boolean));
  } catch {
    return new Set();
  }
}

/**
 * Apply persistence rule: only adopt channels that persist across snapshots.
 * - Look back across the last SNAPSHOT_WINDOW snapshots (distinct discovered_at)
 * - Require at least PERSIST_MIN appearances in that window
 * - Always keep previously adopted channels to avoid churn
 */
async function applyPersistenceRule(pool: Pool, streams: TwitchStream[]): Promise<string[]> {
  const SNAPSHOT_WINDOW = Number(process.env.CLS_DISCOVERY_SNAPSHOT_WINDOW || 6); // last 6 snapshots
  const PERSIST_MIN = Number(process.env.CLS_DISCOVERY_PERSIST_MIN || 3); // must appear >= 3 times
  const current = readCurrentAdopted();

  // Load last N snapshot timestamps
  const snaps = await pool.query(
    `SELECT DISTINCT discovered_at
     FROM cls_discovered_channels
     ORDER BY discovered_at DESC
     LIMIT $1`, [SNAPSHOT_WINDOW]
  );
  const times = snaps.rows.map((r: any) => Number(r.discovered_at));

  // Count appearances per channel in the window
  const countsRes = await pool.query(
    `SELECT channel_name, COUNT(*) AS appearances
     FROM cls_discovered_channels
     WHERE discovered_at = ANY($1)
     GROUP BY channel_name`, [times]
  );
  const counts = new Map<string, number>();
  for (const row of countsRes.rows) {
    counts.set(String(row.channel_name).toLowerCase(), Number(row.appearances));
  }

  const proposed = streams.map(s => s.user_login.toLowerCase());
  const adopted: string[] = [];

  for (const login of proposed) {
    const c = counts.get(login) || 0;
    if (c >= PERSIST_MIN || current.has(login)) {
      adopted.push(login);
    }
  }

  // Keep the list bounded to 100, but preserve existing entries first
  const existing = Array.from(current).filter(c => adopted.includes(c));
  const newcomers = adopted.filter(c => !current.has(c));
  const final = [...existing, ...newcomers].slice(0, 100);

  console.log(`[CLS Discovery] Persistence filter -> kept ${final.length} (existing=${existing.length}, newcomers=${newcomers.length})`);
  return final;
}

/**
 * Main discovery routine
 */
async function main() {
  console.log('[CLS Discovery] Starting Top 100 discovery...');
  const startTime = Date.now();
  const timestamp = Math.floor(startTime / 1000);
  const discoveryRunId = randomUUID();

  // Quiet hours gating (UTC)
  const startUtc = Number(process.env.CLS_DISCOVERY_ACTIVE_START_UTC ?? '14'); // default 14:00 UTC (08:00 CST)
  const endUtc = Number(process.env.CLS_DISCOVERY_ACTIVE_END_UTC ?? '5');      // default 05:00 UTC (23:00 CST)
  const nowUtcHour = new Date().getUTCHours();
  const wraps = startUtc > endUtc; // e.g., 14 -> 5 crosses midnight
  const within = wraps
    ? (nowUtcHour >= startUtc || nowUtcHour <= endUtc)
    : (nowUtcHour >= startUtc && nowUtcHour <= endUtc);
  if (!within) {
    console.log(`[CLS Discovery] Quiet hours: skipping run. UTC hour=${nowUtcHour}, active=${startUtc}-${endUtc}`);
    return;
  }

  // Initialize database
  const pool = new Pool({
    connectionString: DATABASE_URL,
    max: 5,
    idleTimeoutMillis: 30000,
    connectionTimeoutMillis: 5000,
  });

  try {
    // Step 1: Get Twitch OAuth token
    console.log('[CLS Discovery] Getting Twitch OAuth token...');
    const token = await getTwitchToken();

    // Step 2: Fetch top 100 streams
    console.log('[CLS Discovery] Fetching top 100 streams...');
    let streams = await fetchTop100Streams(token);

    // Step 3: Apply blocklist
    streams = applyBlocklist(streams);

    if (streams.length === 0) {
      console.warn('[CLS Discovery] WARNING: No streams after filtering. Exiting.');
      return;
    }

    // Step 4: Record to database (audit trail)
    console.log('[CLS Discovery] Recording discovery to PostgreSQL...');
    await recordDiscovery(pool, streams, discoveryRunId, timestamp);

    // Step 5: Apply persistence and update files
    console.log('[CLS Discovery] Applying persistence rule and updating files...');
    const adopted = await applyPersistenceRule(pool, streams);
    if (adopted.length === 0) {
      console.warn('[CLS Discovery] WARNING: No channels passed persistence filter; keeping current list.');
    } else {
      writeChannelFilesFromLogins(adopted);
    }

    // Summary
    const duration = Date.now() - startTime;
    const viewerRange = streams.length > 0
      ? `${streams[streams.length - 1].viewer_count} - ${streams[0].viewer_count}`
      : 'N/A';

    console.log(`
╔════════════════════════════════════════════════════════════════
║ CLS Discovery Complete
╠════════════════════════════════════════════════════════════════
║ Run ID:        ${discoveryRunId}
║ Timestamp:     ${new Date(startTime).toISOString()}
║ Duration:      ${duration}ms
║ Channels:      ${streams.length}
║ Viewer Range:  ${viewerRange}
║ Files Updated: cls-channels.txt, cls-channels.json
╚════════════════════════════════════════════════════════════════
    `);

    // Restart cls-worker to pick up new channel list
    console.log('[CLS Discovery] Restarting cls-worker to load new channels...');
    try {
      const { execSync } = await import('child_process');
      execSync('pm2 restart cls-worker --update-env', { stdio: 'inherit' });
      console.log('[CLS Discovery] ✓ cls-worker restarted successfully');
    } catch (err) {
      console.error('[CLS Discovery] WARNING: Failed to restart cls-worker:', err);
      console.error('[CLS Discovery] You may need to manually restart: pm2 restart cls-worker');
    }

  } catch (error) {
    console.error('[CLS Discovery] ERROR:', error);
    process.exit(1);
  } finally {
    await pool.end();
  }
}

// Run if executed directly
if (require.main === module) {
  main().catch((err) => {
    console.error('[CLS Discovery] Fatal error:', err);
    process.exit(1);
  });
}
