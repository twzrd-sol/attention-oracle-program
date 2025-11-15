#!/usr/bin/env node
/**
 * Twitch API Recovery Script
 *
 * Purpose: Recover orphaned user_hash -> username mappings by scraping current Twitch chatters
 *
 * Background:
 * - 420,480 sealed_participants records exist with NULL usernames
 * - user_mapping table is empty (0 rows)
 * - Historical data (Oct 26-30) cannot be claimed without this mapping
 *
 * Strategy:
 * 1. Query all unique user_hash values from sealed_participants
 * 2. For each tracked channel, fetch current chatters via Twitch API
 * 3. Hash each username and match against database
 * 4. Insert successful matches into user_mapping
 * 5. Update sealed_participants to backfill usernames
 *
 * Limitations:
 * - Can only recover users still active in tracked channels
 * - Estimated recovery rate: 70-85%
 * - Users who stopped watching after Oct 30 are unrecoverable
 */

import { Client, Pool } from 'pg';
import { keccak_256 } from 'js-sha3';
import dotenv from 'dotenv';

dotenv.config();

const DATABASE_URL = process.env.DATABASE_URL;
const TWITCH_CLIENT_ID = process.env.TWITCH_CLIENT_ID;
const TWITCH_CLIENT_SECRET = process.env.TWITCH_CLIENT_SECRET;

if (!DATABASE_URL || !TWITCH_CLIENT_ID || !TWITCH_CLIENT_SECRET) {
  console.error('Missing required environment variables');
  process.exit(1);
}

// Initialize database pool
const pool = new Pool({
  connectionString: DATABASE_URL,
});

// Hash function (must match aggregator's implementation)
function hashUser(username: string): string {
  const lower = username.toLowerCase();
  return Buffer.from(keccak_256(Buffer.from(lower))).toString('hex');
}

// Twitch API authentication
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
    throw new Error(`Twitch auth failed: ${response.status} ${await response.text()}`);
  }

  const data = await response.json();
  twitchAccessToken = data.access_token;
  console.log('✅ Twitch API authenticated');
  return twitchAccessToken!;
}

// Get broadcaster ID for a channel
async function getBroadcasterID(channel: string): Promise<string | null> {
  const token = await getTwitchAccessToken();

  const response = await fetch(`https://api.twitch.tv/helix/users?login=${channel}`, {
    headers: {
      'Client-ID': TWITCH_CLIENT_ID!,
      'Authorization': `Bearer ${token}`,
    },
  });

  if (!response.ok) {
    console.warn(`⚠️  Failed to get broadcaster ID for ${channel}: ${response.status}`);
    return null;
  }

  const data = await response.json();
  if (data.data.length === 0) return null;

  return data.data[0].id;
}

// Get current chatters in a channel
async function getChatters(channel: string): Promise<string[]> {
  const broadcasterId = await getBroadcasterID(channel);
  if (!broadcasterId) return [];

  const token = await getTwitchAccessToken();
  const chatters: string[] = [];
  let cursor: string | undefined;

  try {
    do {
      const url = new URL('https://api.twitch.tv/helix/chat/chatters');
      url.searchParams.set('broadcaster_id', broadcasterId);
      url.searchParams.set('moderator_id', broadcasterId);
      url.searchParams.set('first', '1000');
      if (cursor) url.searchParams.set('after', cursor);

      const response = await fetch(url.toString(), {
        headers: {
          'Client-ID': TWITCH_CLIENT_ID!,
          'Authorization': `Bearer ${token}`,
        },
      });

      if (!response.ok) {
        console.warn(`⚠️  Failed to fetch chatters for ${channel}: ${response.status}`);
        break;
      }

      const data = await response.json();
      chatters.push(...data.data.map((c: any) => c.user_login));
      cursor = data.pagination?.cursor;

      // Rate limiting: 800 requests/min = ~1.33 req/sec
      await new Promise(resolve => setTimeout(resolve, 800));
    } while (cursor);
  } catch (err) {
    console.error(`❌ Error fetching chatters for ${channel}:`, err);
  }

  return chatters;
}

// Main recovery logic
async function recoverMappings() {
  console.log('🔍 Starting recovery process...\n');

  // Step 1: Get all unique user_hash values from sealed_participants
  console.log('📊 Querying database for orphaned user_hash values...');
  const hashResult = await pool.query(`
    SELECT DISTINCT user_hash
    FROM sealed_participants
    WHERE username IS NULL
  `);

  const orphanedHashes = new Set(hashResult.rows.map(r => r.user_hash));
  console.log(`   Found ${orphanedHashes.size.toLocaleString()} unique orphaned user_hash values\n`);

  if (orphanedHashes.size === 0) {
    console.log('✅ No orphaned records found. Exiting.');
    return;
  }

  // Step 2: Get tracked channels from environment
  const channelsEnv = process.env.MILO_CHANNELS || process.env.CHANNELS || 'lacy,adapt';
  const channels = channelsEnv.split(',').map(c => c.trim().toLowerCase());
  console.log(`🎮 Tracked channels (${channels.length}): ${channels.join(', ')}\n`);

  // Step 3: Scrape current chatters and build mappings
  const discoveredMappings = new Map<string, string>(); // user_hash -> username
  let totalChattersScraped = 0;

  for (const channel of channels) {
    console.log(`🔎 Scraping ${channel}...`);
    const chatters = await getChatters(channel);
    totalChattersScraped += chatters.length;

    console.log(`   Found ${chatters.length} current chatters`);

    let matchCount = 0;
    for (const username of chatters) {
      const user_hash = hashUser(username);

      if (orphanedHashes.has(user_hash) && !discoveredMappings.has(user_hash)) {
        discoveredMappings.set(user_hash, username);
        matchCount++;
      }
    }

    console.log(`   ✅ Matched ${matchCount} chatters to orphaned records`);
    console.log(`   📈 Total recovered: ${discoveredMappings.size.toLocaleString()} / ${orphanedHashes.size.toLocaleString()} (${((discoveredMappings.size / orphanedHashes.size) * 100).toFixed(1)}%)\n`);
  }

  console.log(`\n📊 Recovery Summary:`);
  console.log(`   Total chatters scraped: ${totalChattersScraped.toLocaleString()}`);
  console.log(`   Orphaned records: ${orphanedHashes.size.toLocaleString()}`);
  console.log(`   Recovered mappings: ${discoveredMappings.size.toLocaleString()}`);
  console.log(`   Recovery rate: ${((discoveredMappings.size / orphanedHashes.size) * 100).toFixed(2)}%\n`);

  if (discoveredMappings.size === 0) {
    console.log('⚠️  No mappings recovered. This could mean:');
    console.log('   1. No tracked channels are currently live');
    console.log('   2. All current chatters are new (not in historical data)');
    console.log('   3. Twitch API rate limits blocked scraping');
    console.log('\nTry running this script when more channels are live.');
    return;
  }

  // Step 4: Insert into user_mapping
  console.log('💾 Inserting recovered mappings into database...');
  const client = await pool.connect();

  try {
    await client.query('BEGIN');

    let insertedCount = 0;
    const timestamp = Math.floor(Date.now() / 1000);

    for (const [user_hash, username] of discoveredMappings.entries()) {
      await client.query(
        `INSERT INTO user_mapping (user_hash, username, first_seen)
         VALUES ($1, $2, $3)
         ON CONFLICT (user_hash) DO UPDATE
         SET username = EXCLUDED.username`,
        [user_hash, username, timestamp]
      );
      insertedCount++;

      if (insertedCount % 1000 === 0) {
        console.log(`   Progress: ${insertedCount.toLocaleString()} / ${discoveredMappings.size.toLocaleString()}`);
      }
    }

    await client.query('COMMIT');
    console.log(`   ✅ Inserted ${insertedCount.toLocaleString()} mappings\n`);
  } catch (err) {
    await client.query('ROLLBACK');
    console.error('❌ Failed to insert mappings:', err);
    throw err;
  } finally {
    client.release();
  }

  // Step 5: Update sealed_participants to backfill usernames
  console.log('🔄 Backfilling usernames in sealed_participants...');

  try {
    const updateResult = await pool.query(`
      UPDATE sealed_participants sp
      SET username = um.username
      FROM user_mapping um
      WHERE sp.user_hash = um.user_hash
        AND sp.username IS NULL
    `);

    console.log(`   ✅ Updated ${updateResult.rowCount?.toLocaleString() || 0} sealed_participants records\n`);
  } catch (err) {
    console.error('❌ Failed to update sealed_participants:', err);
  }

  // Step 6: Final statistics
  console.log('📊 Final Database State:');

  const userMappingCount = await pool.query('SELECT COUNT(*) FROM user_mapping');
  console.log(`   user_mapping: ${parseInt(userMappingCount.rows[0].count).toLocaleString()} rows`);

  const nullUsernameCount = await pool.query('SELECT COUNT(*) FROM sealed_participants WHERE username IS NULL');
  console.log(`   sealed_participants (NULL username): ${parseInt(nullUsernameCount.rows[0].count).toLocaleString()} rows`);

  const populatedUsernameCount = await pool.query('SELECT COUNT(*) FROM sealed_participants WHERE username IS NOT NULL');
  console.log(`   sealed_participants (username populated): ${parseInt(populatedUsernameCount.rows[0].count).toLocaleString()} rows`);

  console.log('\n✅ Recovery complete!');
  console.log('\n💡 Next steps:');
  console.log('   1. Run this script again during peak hours to recover more users');
  console.log('   2. Monitor new participation to ensure username mapping is working');
  console.log('   3. Consider scheduling this as a daily cron job to catch returning users');
}

// Run recovery
recoverMappings()
  .then(() => {
    console.log('\n🎉 Script finished successfully');
    process.exit(0);
  })
  .catch((err) => {
    console.error('\n❌ Script failed:', err);
    process.exit(1);
  });
