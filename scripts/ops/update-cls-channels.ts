#!/usr/bin/env tsx
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { execSync } from 'node:child_process';
import dotenv from 'dotenv';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Load root .env so we have Twitch credentials and channel lists
dotenv.config({ path: path.resolve(__dirname, '../../.env') });

const TWITCH_CLIENT_ID = process.env.TWITCH_CLIENT_ID ?? '';
const TWITCH_CLIENT_SECRET = process.env.TWITCH_CLIENT_SECRET ?? '';

if (!TWITCH_CLIENT_ID || !TWITCH_CLIENT_SECRET) {
  console.error('Missing TWITCH_CLIENT_ID/TWITCH_CLIENT_SECRET in environment.');
  process.exit(1);
}

const MILO_CHANNELS = new Set(
  (process.env.MILO_CHANNELS || '')
    .split(',')
    .map((c) => c.trim().toLowerCase())
    .filter(Boolean),
);

const CLS_BLOCKLIST = new Set(
  (process.env.CLS_BLOCKLIST || 'threadguy,threadguys,thethreadguy,notthreadguy,counterpartytv')
    .split(',')
    .map((c) => c.trim().toLowerCase())
    .filter(Boolean),
);

const CLS_MIN_VIEWERS = Number(process.env.CLS_MIN_VIEWERS || '25');
const CLS_MIN_DURATION_MINUTES = Number(process.env.CLS_MIN_DURATION_MINUTES || '10');
const CLS_MAX_STREAMS = Number(process.env.CLS_MAX_STREAMS || '60');

const CLS_CHANNELS_JSON = path.resolve(process.env.CLS_CHANNELS_FILE || path.resolve(__dirname, '../../config/cls-channels.json'));
const CLS_CHANNELS_TXT = path.resolve(process.env.CLS_CHANNELS_TXT || path.resolve(__dirname, '../../config/cls-channels.txt'));

const GAME_NAME = process.env.CLS_TWITCH_GAME_NAME || 'Crypto';

type TwitchStream = {
  user_login: string;
  viewer_count: number;
  started_at: string;
};

async function getAppAccessToken(): Promise<string> {
  const params = new URLSearchParams({
    client_id: TWITCH_CLIENT_ID,
    client_secret: TWITCH_CLIENT_SECRET,
    grant_type: 'client_credentials',
  });

  const resp = await fetch('https://id.twitch.tv/oauth2/token', {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: params,
  });

  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(`Failed to fetch Twitch token: ${resp.status} ${text}`);
  }

  const json = await resp.json() as { access_token?: string };
  if (!json.access_token) throw new Error('Twitch token response missing access_token');
  return json.access_token;
}

async function getGameId(token: string, name: string): Promise<string> {
  const url = new URL('https://api.twitch.tv/helix/games');
  url.searchParams.set('name', name);
  const resp = await fetch(url, {
    headers: {
      'Client-Id': TWITCH_CLIENT_ID,
      Authorization: `Bearer ${token}`,
    },
  });

  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(`Failed to fetch Twitch game id: ${resp.status} ${text}`);
  }

  const json = await resp.json() as { data?: Array<{ id: string }> };
  if (!json.data?.length) throw new Error(`Twitch game not found for name "${name}"`);
  return json.data[0]!.id;
}

async function fetchStreams(token: string, gameId: string): Promise<TwitchStream[]> {
  const results: TwitchStream[] = [];
  let cursor: string | undefined;

  while (true) {
    const url = new URL('https://api.twitch.tv/helix/streams');
    url.searchParams.set('game_id', gameId);
    url.searchParams.set('first', '100');
    if (cursor) url.searchParams.set('after', cursor);

    const resp = await fetch(url, {
      headers: {
        'Client-Id': TWITCH_CLIENT_ID,
        Authorization: `Bearer ${token}`,
      },
    });

    if (!resp.ok) {
      const text = await resp.text();
      throw new Error(`Failed to fetch streams: ${resp.status} ${text}`);
    }

    const json = await resp.json() as { data?: TwitchStream[]; pagination?: { cursor?: string } };
    if (!json.data?.length) break;

    results.push(...json.data);
    cursor = json.pagination?.cursor;
    if (!cursor) break;
    if (results.length >= CLS_MAX_STREAMS * 2) break; // hard cap safety
  }

  return results;
}

function filterStreams(streams: TwitchStream[]): string[] {
  const now = Date.now();
  const minDurationMs = CLS_MIN_DURATION_MINUTES * 60 * 1000;

  const filtered = streams
    .filter((s) => s.viewer_count >= CLS_MIN_VIEWERS)
    .filter((s) => {
      const started = Date.parse(s.started_at);
      if (Number.isNaN(started)) return false;
      return now - started >= minDurationMs;
    })
    .map((s) => s.user_login.toLowerCase())
    .filter((login) => !MILO_CHANNELS.has(login))
    .filter((login) => !CLS_BLOCKLIST.has(login));

  const unique = Array.from(new Set(filtered));
  unique.sort((a, b) => a.localeCompare(b));
  return unique.slice(0, CLS_MAX_STREAMS);
}

function readExistingList(filePath: string): string[] {
  try {
    const raw = fs.readFileSync(filePath, 'utf8');
    const parsed = JSON.parse(raw);
    if (Array.isArray(parsed)) {
      return parsed.map((v) => String(v).toLowerCase()).filter(Boolean);
    }
  } catch (err) {
    // Ignore missing/invalid file
  }
  return [];
}

function listsDiffer(a: string[], b: string[]): boolean {
  if (a.length !== b.length) return true;
  const sa = new Set(a);
  const sb = new Set(b);
  if (sa.size !== sb.size) return true;
  for (const item of sa) {
    if (!sb.has(item)) return true;
  }
  return false;
}

function writeChannelLists(channels: string[]): void {
  const dir = path.dirname(CLS_CHANNELS_JSON);
  fs.mkdirSync(dir, { recursive: true });

  const existing = readExistingList(CLS_CHANNELS_JSON);
  const changed = listsDiffer(existing, channels);

  if (!changed) {
    console.log(`CLS channel list unchanged (${channels.length} entries).`);
    return;
  }

  fs.writeFileSync(CLS_CHANNELS_JSON, JSON.stringify(channels, null, 2));
  fs.writeFileSync(CLS_CHANNELS_TXT, channels.join('\n'));

  console.log(`CLS channel list updated (${channels.length} entries):`);
  channels.forEach((c) => console.log(` - ${c}`));

  // Restart throttling: only restart worker if material change
  const additions = channels.filter((c) => !existing.includes(c));
  const removals = existing.filter((c) => !channels.includes(c));
  const delta = additions.length + removals.length;
  const MIN_CHANGES = Number(process.env.CLS_RESTART_MIN_CHANGES || '3');

  if (delta < MIN_CHANGES) {
    console.log(`Change delta (${delta}) < MIN_CHANGES (${MIN_CHANGES}); skip worker restart.`);
    return;
  }

  if (CLS_RESTART_COMMAND) {
    try {
      console.log(`Executing restart command: ${CLS_RESTART_COMMAND}`);
      execSync(CLS_RESTART_COMMAND, { stdio: 'inherit' });
    } catch (err) {
      console.error('Failed to restart CLS worker:', err);
    }
  }
}

async function main(): Promise<void> {
  try {
    const token = await getAppAccessToken();
    const gameId = await getGameId(token, GAME_NAME);
    const streams = await fetchStreams(token, gameId);
    const channels = filterStreams(streams);

    writeChannelLists(channels);
  } catch (err: any) {
    console.error('CLS channel discovery failed:', err?.message ?? err);
    process.exit(1);
  }
}

void main();
const CLS_RESTART_COMMAND = process.env.CLS_RESTART_COMMAND || 'pm2 restart cls-worker-s0 cls-worker-s1';
