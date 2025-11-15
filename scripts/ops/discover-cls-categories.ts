#!/usr/bin/env tsx
/**
 * Multi-Category CLS Channel Discovery
 *
 * Discovers channels across multiple builder categories:
 * - Crypto & Web3
 * - Science & Technology
 * - Music Production
 * - Makers & Crafting
 * - Game Development
 * - Creative Coding
 * - Founders & Entrepreneurship (curated)
 *
 * Outputs:
 * - config/cls-{category}-channels.json (per category)
 * - config/cls-all-channels.json (master manifest with category tags)
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import dotenv from 'dotenv';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

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

const CONFIG_FILE = path.resolve(__dirname, '../../config/cls-categories.json');
const OUTPUT_DIR = path.resolve(__dirname, '../../config');

type CategoryConfig = {
  id: string;
  name: string;
  description: string;
  discovery: {
    method: 'game' | 'tags' | 'curated';
    game_name?: string;
    tags?: string[];
    curated_list?: string;
    min_viewers: number;
    min_duration_minutes: number;
    max_streams: number;
  };
  splits: {
    viewer_ratio: number;
    streamer_ratio: number;
  };
  enabled: boolean;
  blocklist: string[];
  notes?: string;
};

type CategoriesConfig = {
  categories: CategoryConfig[];
  global: {
    default_viewer_ratio: number;
    default_streamer_ratio: number;
    max_total_streams_across_categories: number;
  };
};

type TwitchStream = {
  user_login: string;
  viewer_count: number;
  started_at: string;
  game_name?: string;
  title?: string;
};

type DiscoveredChannel = {
  username: string;
  category: string;
  viewer_count: number;
  uptime_minutes: number;
  discovered_at: number;
};

// Load category configuration
function loadCategoryConfig(): CategoriesConfig {
  try {
    const raw = fs.readFileSync(CONFIG_FILE, 'utf8');
    return JSON.parse(raw) as CategoriesConfig;
  } catch (err) {
    console.error(`Failed to load category config from ${CONFIG_FILE}:`, err);
    process.exit(1);
  }
}

// Twitch API helpers
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

async function getGameId(token: string, name: string): Promise<string | null> {
  const url = new URL('https://api.twitch.tv/helix/games');
  url.searchParams.set('name', name);

  const resp = await fetch(url, {
    headers: {
      'Client-Id': TWITCH_CLIENT_ID,
      Authorization: `Bearer ${token}`,
    },
  });

  if (!resp.ok) {
    console.warn(`Failed to fetch game ID for "${name}": ${resp.status}`);
    return null;
  }

  const json = await resp.json() as { data?: Array<{ id: string }> };
  if (!json.data?.length) {
    console.warn(`Twitch game not found for name "${name}"`);
    return null;
  }

  return json.data[0]!.id;
}

async function fetchStreamsByGame(token: string, gameId: string, maxStreams: number): Promise<TwitchStream[]> {
  const results: TwitchStream[] = [];
  let cursor: string | undefined;

  while (results.length < maxStreams * 2) {
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
      console.warn(`Failed to fetch streams: ${resp.status}`);
      break;
    }

    const json = await resp.json() as { data?: TwitchStream[]; pagination?: { cursor?: string } };
    if (!json.data?.length) break;

    results.push(...json.data);
    cursor = json.pagination?.cursor;
    if (!cursor) break;
  }

  return results;
}

async function fetchStreamsByTags(token: string, tags: string[], maxStreams: number): Promise<TwitchStream[]> {
  // Note: Twitch deprecated tag-based search in their API
  // This would require searching all live streams and filtering by title/tags
  // For now, return empty and recommend game-based or curated discovery
  console.warn('Tag-based discovery not yet implemented (Twitch API limitation)');
  return [];
}

function readCuratedList(filePath: string): string[] {
  try {
    const fullPath = path.resolve(__dirname, '../../', filePath);
    const raw = fs.readFileSync(fullPath, 'utf8');
    return raw.split(/\r?\n/).map((c) => c.trim().toLowerCase()).filter(Boolean);
  } catch (err) {
    console.warn(`Failed to read curated list from ${filePath}:`, err);
    return [];
  }
}

function filterStreams(
  streams: TwitchStream[],
  category: CategoryConfig,
  categoryBlocklist: Set<string>
): DiscoveredChannel[] {
  const now = Date.now();
  const minDurationMs = category.discovery.min_duration_minutes * 60 * 1000;
  const minViewers = category.discovery.min_viewers;

  const filtered = streams
    .filter((s) => s.viewer_count >= minViewers)
    .filter((s) => {
      const started = Date.parse(s.started_at);
      if (Number.isNaN(started)) return false;
      return now - started >= minDurationMs;
    })
    .map((s) => ({
      username: s.user_login.toLowerCase(),
      category: category.id,
      viewer_count: s.viewer_count,
      uptime_minutes: Math.floor((now - Date.parse(s.started_at)) / 60000),
      discovered_at: now,
    }))
    .filter((ch) => !MILO_CHANNELS.has(ch.username))
    .filter((ch) => !categoryBlocklist.has(ch.username));

  const unique = Array.from(
    new Map(filtered.map((ch) => [ch.username, ch])).values()
  );

  unique.sort((a, b) => b.viewer_count - a.viewer_count);
  return unique.slice(0, category.discovery.max_streams);
}

async function discoverCategory(
  token: string,
  category: CategoryConfig
): Promise<DiscoveredChannel[]> {
  console.log(`\n🔍 Discovering: ${category.name} (${category.id})`);
  console.log(`   Method: ${category.discovery.method}`);
  console.log(`   Min viewers: ${category.discovery.min_viewers}`);
  console.log(`   Min duration: ${category.discovery.min_duration_minutes} min`);
  console.log(`   Max streams: ${category.discovery.max_streams}`);

  const categoryBlocklist = new Set(
    category.blocklist.map((c) => c.toLowerCase())
  );

  let streams: TwitchStream[] = [];

  if (category.discovery.method === 'game' && category.discovery.game_name) {
    const gameId = await getGameId(token, category.discovery.game_name);
    if (!gameId) {
      console.warn(`   ⚠️  Skipping ${category.id}: game not found`);
      return [];
    }
    streams = await fetchStreamsByGame(token, gameId, category.discovery.max_streams);
  } else if (category.discovery.method === 'tags' && category.discovery.tags) {
    streams = await fetchStreamsByTags(token, category.discovery.tags, category.discovery.max_streams);
  } else if (category.discovery.method === 'curated' && category.discovery.curated_list) {
    const curated = readCuratedList(category.discovery.curated_list);
    console.log(`   📋 Loaded ${curated.length} curated channels`);
    // For curated lists, we don't need to discover - just validate they're live
    // For now, return curated list as-is (TODO: validate live status)
    return curated.map((username) => ({
      username,
      category: category.id,
      viewer_count: 0,
      uptime_minutes: 0,
      discovered_at: Date.now(),
    }));
  }

  const discovered = filterStreams(streams, category, categoryBlocklist);
  console.log(`   ✅ Found ${discovered.length} channels`);

  if (discovered.length > 0) {
    console.log(`   Top 3: ${discovered.slice(0, 3).map((ch) => `${ch.username} (${ch.viewer_count}v)`).join(', ')}`);
  }

  return discovered;
}

function writeOutputFiles(
  allChannels: Map<string, DiscoveredChannel[]>,
  config: CategoriesConfig
): void {
  fs.mkdirSync(OUTPUT_DIR, { recursive: true });

  // Write per-category files
  for (const [categoryId, channels] of allChannels.entries()) {
    const categoryFile = path.join(OUTPUT_DIR, `cls-${categoryId}-channels.json`);
    const usernames = channels.map((ch) => ch.username);
    fs.writeFileSync(categoryFile, JSON.stringify(usernames, null, 2));
    console.log(`\n📝 Wrote ${channels.length} channels to cls-${categoryId}-channels.json`);
  }

  // Write master manifest with metadata
  const masterFile = path.join(OUTPUT_DIR, 'cls-all-channels.json');
  const masterData = {
    discovered_at: Date.now(),
    total_channels: Array.from(allChannels.values()).reduce((sum, chs) => sum + chs.length, 0),
    categories: Object.fromEntries(
      Array.from(allChannels.entries()).map(([id, channels]) => [
        id,
        {
          count: channels.length,
          channels: channels.map((ch) => ({
            username: ch.username,
            viewer_count: ch.viewer_count,
            uptime_minutes: ch.uptime_minutes,
          })),
        },
      ])
    ),
    config_snapshot: {
      enabled_categories: config.categories.filter((c) => c.enabled).map((c) => c.id),
      global_limits: config.global,
    },
  };

  fs.writeFileSync(masterFile, JSON.stringify(masterData, null, 2));
  console.log(`\n📊 Wrote master manifest to cls-all-channels.json`);
  console.log(`   Total channels across all categories: ${masterData.total_channels}`);
}

async function main(): Promise<void> {
  console.log('🚀 Multi-Category CLS Discovery\n');

  const config = loadCategoryConfig();
  const enabledCategories = config.categories.filter((c) => c.enabled);

  console.log(`📋 Enabled categories: ${enabledCategories.map((c) => c.id).join(', ')}`);
  console.log(`🌍 Global limit: ${config.global.max_total_streams_across_categories} total streams\n`);

  const token = await getAppAccessToken();
  const allChannels = new Map<string, DiscoveredChannel[]>();

  for (const category of enabledCategories) {
    try {
      const discovered = await discoverCategory(token, category);
      allChannels.set(category.id, discovered);
    } catch (err: any) {
      console.error(`❌ Failed to discover ${category.id}:`, err?.message ?? err);
    }
  }

  // Check global limit
  const totalDiscovered = Array.from(allChannels.values()).reduce((sum, chs) => sum + chs.length, 0);
  if (totalDiscovered > config.global.max_total_streams_across_categories) {
    console.warn(`\n⚠️  WARNING: Discovered ${totalDiscovered} channels, exceeds global limit of ${config.global.max_total_streams_across_categories}`);
    console.warn('   Consider raising per-category limits or adjusting thresholds.');
  }

  writeOutputFiles(allChannels, config);

  console.log('\n✅ Discovery complete!');
  console.log('\nNext steps:');
  console.log('  1. Review channel lists in config/cls-*-channels.json');
  console.log('  2. Run sanity check on discovered channels');
  console.log('  3. Update worker configuration to use new category files');
  console.log('  4. Restart cls-worker with new channel lists');
}

void main();
