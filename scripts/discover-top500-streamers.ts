#!/usr/bin/env node
/**
 * Top 100 Twitch Streamer Discovery (Daily Snapshot)
 *
 * Purpose: Track top Twitch streamers by concurrent viewers for CLS expansion
 *
 * License: MIT (2025 TWZRD) - Open Source
 * Repository: github.com/twzrd-sol/attention-oracle-program
 *
 * Methodology:
 * 1. Fetch top 100 live streams via Twitch Helix API (/helix/streams?first=100)
 * 2. Enrich with community metrics (followers, chat activity)
 * 3. Filter for CLS eligibility (min_viewers, category match)
 * 4. Add to cls_discovered_channels table
 * 5. Log daily snapshot for analytics
 *
 * Cron Schedule: 0 0 * * * (daily at 00:00 UTC)
 *
 * Rate Limits: 800 req/min (Twitch Helix)
 * No secrets in code - reads from .env only
 */

import { Pool } from 'pg';
import dotenv from 'dotenv';

dotenv.config();

const DATABASE_URL = process.env.DATABASE_URL;
const TWITCH_CLIENT_ID = process.env.TWITCH_CLIENT_ID;
const TWITCH_CLIENT_SECRET = process.env.TWITCH_CLIENT_SECRET;

// CLS thresholds (brand-neutral, configurable)
const MIN_VIEWERS = Number(process.env.CLS_MIN_VIEWERS || 25);
const MIN_DURATION_MINUTES = Number(process.env.CLS_MIN_DURATION_MINUTES || 10);

// Crypto-related game IDs (Twitch category IDs)
const CRYPTO_GAME_IDS = [
  '1469308723', // Crypto
  '509670',     // Science & Technology
  '509673',     // Talk Shows & Podcasts
];

if (!DATABASE_URL || !TWITCH_CLIENT_ID || !TWITCH_CLIENT_SECRET) {
  console.error('❌ Missing required environment variables');
  process.exit(1);
}

const pool = new Pool({
  connectionString: DATABASE_URL,
});

interface TwitchStream {
  id: string;
  user_id: string;
  user_login: string;
  user_name: string;
  game_id: string;
  game_name: string;
  type: string;
  title: string;
  viewer_count: number;
  started_at: string;
  language: string;
  thumbnail_url: string;
  tag_ids: string[];
}

interface StreamerProfile {
  user_id: string;
  login: string;
  display_name: string;
  follower_count: number;
  view_count: number;
  description: string;
}

interface CommunityMetrics {
  channel: string;
  concurrent_viewers: number;
  follower_count: number;
  view_count: number;
  category: string;
  community_size_estimate: number; // followers * 0.1 + viewer_count
  uptime_minutes: number;
}

let twitchAccessToken: string | null = null;

async function getTwitchAccessToken(): Promise<string> {
  if (twitchAccessToken) return twitchAccessToken;

  const response = await fetch('https://id.twitch.tv/oauth2/token', {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({
      client_id: TWITCH_CLIENT_ID!,
      client_secret: TWITCH_CLIENT_SECRET!,
      grant_type: 'client_credentials',
    }),
  });

  if (!response.ok) {
    throw new Error(`Twitch auth failed: ${response.status}`);
  }

  const data = await response.json();
  twitchAccessToken = data.access_token;
  console.log('✅ Twitch API authenticated');
  return twitchAccessToken!;
}

async function getTop500Streams(): Promise<TwitchStream[]> {
  const token = await getTwitchAccessToken();
  const streams: TwitchStream[] = [];
  let cursor: string | undefined;

  console.log('📡 Fetching top 500 live streams...');

  // Fetch in batches of 100 (Twitch API limit), paginate to 500
  const MAX_STREAMS = 500;
  const BATCH_SIZE = 100;

  while (streams.length < MAX_STREAMS) {
    const url = new URL('https://api.twitch.tv/helix/streams');
    url.searchParams.set('first', String(BATCH_SIZE));
    if (cursor) url.searchParams.set('after', cursor);

    const response = await fetch(url.toString(), {
      headers: {
        'Client-ID': TWITCH_CLIENT_ID!,
        'Authorization': `Bearer ${token}`,
      },
    });

    if (!response.ok) {
      throw new Error(`Failed to fetch streams: ${response.status}`);
    }

    const data = await response.json();
    streams.push(...data.data);

    console.log(`   Fetched ${streams.length} streams...`);

    cursor = data.pagination?.cursor;
    if (!cursor || data.data.length === 0) break;

    // Rate limiting: 800 req/min = ~1.33 req/sec
    await new Promise(resolve => setTimeout(resolve, 800));
  }

  console.log(`   Found ${streams.length} live streams`);
  return streams;
}

async function getStreamerProfiles(userIds: string[]): Promise<Map<string, StreamerProfile>> {
  const token = await getTwitchAccessToken();
  const profiles = new Map<string, StreamerProfile>();

  // Batch fetch user profiles (100 at a time)
  const batchSize = 100;
  for (let i = 0; i < userIds.length; i += batchSize) {
    const batch = userIds.slice(i, i + batchSize);
    const url = new URL('https://api.twitch.tv/helix/users');
    batch.forEach(id => url.searchParams.append('id', id));

    const response = await fetch(url.toString(), {
      headers: {
        'Client-ID': TWITCH_CLIENT_ID!,
        'Authorization': `Bearer ${token}`,
      },
    });

    if (!response.ok) {
      console.warn(`⚠️  Failed to fetch user profiles: ${response.status}`);
      continue;
    }

    const data = await response.json();
    data.data.forEach((user: any) => {
      profiles.set(user.id, {
        user_id: user.id,
        login: user.login,
        display_name: user.display_name,
        follower_count: 0, // Will be enriched separately
        view_count: user.view_count || 0,
        description: user.description || '',
      });
    });

    await new Promise(resolve => setTimeout(resolve, 800));
  }

  return profiles;
}

async function getFollowerCounts(userIds: string[]): Promise<Map<string, number>> {
  const token = await getTwitchAccessToken();
  const followers = new Map<string, number>();

  console.log('📊 Fetching follower counts...');

  for (const userId of userIds) {
    const url = new URL('https://api.twitch.tv/helix/channels/followers');
    url.searchParams.set('broadcaster_id', userId);
    url.searchParams.set('first', '1');

    try {
      const response = await fetch(url.toString(), {
        headers: {
          'Client-ID': TWITCH_CLIENT_ID!,
          'Authorization': `Bearer ${token}`,
        },
      });

      if (!response.ok) {
        console.warn(`⚠️  Failed to fetch followers for ${userId}: ${response.status}`);
        continue;
      }

      const data = await response.json();
      followers.set(userId, data.total || 0);

      await new Promise(resolve => setTimeout(resolve, 800));
    } catch (err) {
      console.warn(`⚠️  Error fetching followers for ${userId}: ${err}`);
    }
  }

  return followers;
}

function calculateCommunitySize(followers: number, viewers: number): number {
  // Community size estimate: 10% of followers are "active" + current viewers
  return Math.floor(followers * 0.1) + viewers;
}

function calculateUptimeMinutes(startedAt: string): number {
  const started = new Date(startedAt).getTime();
  const now = Date.now();
  return Math.floor((now - started) / (1000 * 60));
}

function categorizeCrypto(gameName: string, title: string, description: string): string {
  const combined = `${gameName} ${title} ${description}`.toLowerCase();

  if (combined.includes('crypto') || combined.includes('bitcoin') || combined.includes('nft')) {
    return 'crypto';
  }
  if (combined.includes('music') || combined.includes('dj') || combined.includes('producer')) {
    return 'music';
  }
  if (combined.includes('science') || combined.includes('tech') || combined.includes('engineering')) {
    return 'science';
  }
  if (combined.includes('maker') || combined.includes('diy') || combined.includes('craft')) {
    return 'makers';
  }

  return 'default';
}

async function saveToDiscoveryTable(metrics: CommunityMetrics[]) {
  let insertedCount = 0;
  const discoveredAt = new Date();
  const discoveryRunId = `top100-${Date.now()}`;

  console.log(`\n💾 Attempting to save ${metrics.length} channels to database...`);
  console.log(`   Discovery Run ID: ${discoveryRunId}`);
  console.log(`   Discovered At: ${discoveredAt.toISOString()}`);
  console.log(`   First 3 channels: ${metrics.slice(0, 3).map(m => m.channel).join(', ')}`);

  for (let i = 0; i < metrics.length; i++) {
    const m = metrics[i];
    const metadata = {
      follower_count: m.follower_count,
      community_size_estimate: m.community_size_estimate,
      uptime_minutes: m.uptime_minutes,
      view_count: m.view_count,
    };

    try {
      if (i < 3) console.log(`   [${i+1}] Inserting ${m.channel}...`);
      await pool.query(`
        INSERT INTO cls_discovered_channels (
          channel_name,
          category,
          discovered_at,
          viewer_count,
          rank,
          discovery_run_id,
          metadata
        ) VALUES ($1, $2, $3, $4, $5, $6, $7)
      `, [
        m.channel,
        m.category,
        Math.floor(discoveredAt.getTime() / 1000),
        m.concurrent_viewers,
        i + 1, // rank
        discoveryRunId,
        JSON.stringify(metadata),
      ]);
      insertedCount++;
      if (i < 3) console.log(`   [${i+1}] ✓ ${m.channel} inserted successfully`);
    } catch (err: any) {
      // Log ALL errors for debugging
      console.warn(`⚠️  Failed to insert ${m.channel}: ${err.code} - ${err.message}`);
      if (err.code !== '23505') {
        console.error('Full error:', err);
      }
    }
  }

  console.log(`✅ Saved ${insertedCount} streamers to discovery table`);
}

async function logDailySnapshot(metrics: CommunityMetrics[]) {
  const snapshotDate = new Date().toISOString().split('T')[0];
  const filename = `/tmp/top100-snapshot-${snapshotDate}.json`;

  const snapshot = {
    date: snapshotDate,
    total_streamers: metrics.length,
    total_viewers: metrics.reduce((sum, m) => sum + m.concurrent_viewers, 0),
    total_community_size: metrics.reduce((sum, m) => sum + m.community_size_estimate, 0),
    categories: {
      crypto: metrics.filter(m => m.category === 'crypto').length,
      music: metrics.filter(m => m.category === 'music').length,
      science: metrics.filter(m => m.category === 'science').length,
      makers: metrics.filter(m => m.category === 'makers').length,
      default: metrics.filter(m => m.category === 'default').length,
    },
    top_10: metrics.slice(0, 10).map(m => ({
      rank: metrics.indexOf(m) + 1,
      channel: m.channel,
      viewers: m.concurrent_viewers,
      followers: m.follower_count,
      community_size: m.community_size_estimate,
      category: m.category,
    })),
  };

  console.log(`\n📊 DAILY SNAPSHOT (${snapshotDate})\n`);
  console.log(`Total Streamers:      ${snapshot.total_streamers}`);
  console.log(`Total Viewers:        ${snapshot.total_viewers.toLocaleString()}`);
  console.log(`Community Size:       ${snapshot.total_community_size.toLocaleString()}`);
  console.log('\nCategory Breakdown:');
  Object.entries(snapshot.categories).forEach(([cat, count]) => {
    console.log(`  ${cat.padEnd(10)}: ${count}`);
  });
  console.log('\nTop 10 by Concurrent Viewers:\n');
  console.log('Rank | Channel              | Viewers  | Followers | Community  | Category');
  console.log('-----|----------------------|----------|-----------|------------|----------');
  snapshot.top_10.forEach(s => {
    const rank = s.rank.toString().padStart(4);
    const channel = s.channel.padEnd(20);
    const viewers = s.viewers.toLocaleString().padStart(8);
    const followers = s.followers.toLocaleString().padStart(9);
    const community = s.community_size.toLocaleString().padStart(10);
    const category = s.category;
    console.log(`${rank} | ${channel} | ${viewers} | ${followers} | ${community} | ${category}`);
  });
  console.log('');
}

async function main() {
  console.log('╔════════════════════════════════════════════════════════════════╗');
  console.log('║      TOP 500 TWITCH STREAMER DISCOVERY                         ║');
  console.log('║      MIT License (2025 TWZRD) - Open Source                    ║');
  console.log('╚════════════════════════════════════════════════════════════════╝\n');

  try {
    // Step 1: Fetch top 500 streams
    const streams = await getTop500Streams();

    if (streams.length === 0) {
      console.log('⚠️  No live streams found');
      return;
    }

    // Step 2: Fetch streamer profiles
    const userIds = streams.map(s => s.user_id);
    console.log('👤 Fetching streamer profiles...');
    const profiles = await getStreamerProfiles(userIds);

    // Step 3: Fetch follower counts
    const followerCounts = await getFollowerCounts(userIds);

    // Step 4: Build community metrics
    console.log('📈 Building community metrics...');
    const metrics: CommunityMetrics[] = [];

    for (const stream of streams) {
      const profile = profiles.get(stream.user_id);
      const followers = followerCounts.get(stream.user_id) || 0;
      const uptime = calculateUptimeMinutes(stream.started_at);

      // Filter: min viewers and min duration
      if (stream.viewer_count < MIN_VIEWERS) continue;
      if (uptime < MIN_DURATION_MINUTES) continue;

      const category = categorizeCrypto(
        stream.game_name,
        stream.title,
        profile?.description || ''
      );

      metrics.push({
        channel: stream.user_login,
        concurrent_viewers: stream.viewer_count,
        follower_count: followers,
        view_count: profile?.view_count || 0,
        category,
        community_size_estimate: calculateCommunitySize(followers, stream.viewer_count),
        uptime_minutes: uptime,
      });
    }

    // Sort by concurrent viewers (descending)
    metrics.sort((a, b) => b.concurrent_viewers - a.concurrent_viewers);

    console.log(`   Filtered to ${metrics.length} eligible streamers (>${MIN_VIEWERS} viewers, >${MIN_DURATION_MINUTES}min uptime)`);

    // Step 5: Save to database
    if (metrics.length > 0) {
      await saveToDiscoveryTable(metrics);
    }

    // Step 6: Log daily snapshot
    await logDailySnapshot(metrics);

    await pool.end();
    console.log('✅ Discovery complete\n');
    process.exit(0);
  } catch (err) {
    console.error('\n❌ Discovery failed:', err);
    await pool.end();
    process.exit(1);
  }
}

main();
