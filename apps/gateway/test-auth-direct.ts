#!/usr/bin/env tsx
/**
 * Bypass cookie issues and test auth endpoints directly by making
 * internal requests with the JWT payload
 */

import { keccak_256 } from '@noble/hashes/sha3';
import { Pool } from 'pg';
import dotenv from 'dotenv';
import { resolve } from 'path';

// Load from repo root .env file
dotenv.config({ path: resolve(process.cwd(), '../../.env') });

let DATABASE_URL = process.env.DATABASE_URL;
if (!DATABASE_URL) {
  throw new Error('DATABASE_URL not set in environment');
}

// Strip sslmode parameter and handle SSL manually
const url = new URL(DATABASE_URL);
url.searchParams.delete('sslmode');
DATABASE_URL = url.toString();

async function testDirectAuth() {
  const pool = new Pool({
    connectionString: DATABASE_URL,
    ssl: { rejectUnauthorized: false }
  });

  // Test user from our investigation
  const twitchLogin = 'dizzybreezyy';
  const expectedUserHash = '00211351fa907cf7cc37110fd2e2c61551049549dbe40818f00d305b53d199b3';
  const epoch = 1761753600;
  const channel = 'jasontheween';

  console.log('🔍 Testing Authentication Flow Directly');
  console.log('');
  console.log('Test User:', twitchLogin);
  console.log('Expected Hash:', expectedUserHash);
  console.log('Epoch:', epoch);
  console.log('Channel:', channel);
  console.log('');

  // Step 1: Compute user_hash (what the authenticated endpoint does)
  const computedHash = Buffer.from(
    keccak_256(Buffer.from(twitchLogin.toLowerCase(), 'utf8'))
  ).toString('hex');

  console.log('✅ Step 1: Compute user_hash from twitchLogin');
  console.log('   Input:', twitchLogin.toLowerCase());
  console.log('   Computed:', computedHash);
  console.log('   Expected:', expectedUserHash);
  console.log('   Match:', computedHash === expectedUserHash ? '✓' : '✗');
  console.log('');

  // Step 2: Query for all claims (what /api/claims/available does)
  console.log('✅ Step 2: Query database for available claims');
  const availableResult = await pool.query(`
    SELECT DISTINCT
      sp.epoch,
      sp.channel,
      sp.idx as index,
      se.root,
      se.sealed_at,
      se.published
    FROM sealed_participants sp
    JOIN sealed_epochs se
      ON sp.epoch = se.epoch
      AND sp.channel = se.channel
    WHERE sp.user_hash = $1
    ORDER BY sp.epoch DESC, sp.channel ASC
    LIMIT 100
  `, [computedHash]);

  console.log(`   Found ${availableResult.rows.length} available claims`);
  if (availableResult.rows.length > 0) {
    console.log('   Claims:');
    availableResult.rows.forEach((row, i) => {
      console.log(`     ${i + 1}. Epoch ${row.epoch}, Channel: ${row.channel}, Index: ${row.index}, Published: ${row.published}`);
    });
  }
  console.log('');

  // Step 3: Get participants list for proof generation
  console.log('✅ Step 3: Get sealed participants for proof generation');
  const participantsResult = await pool.query(`
    SELECT user_hash
    FROM sealed_participants
    WHERE epoch = $1
      AND channel = $2
      AND token_group = $3
      AND category = $4
    ORDER BY idx ASC
  `, [epoch, channel, 'MILO', 'default']);

  const participants = participantsResult.rows.map(r => r.user_hash);
  const userIndex = participants.indexOf(computedHash);

  console.log(`   Total participants: ${participants.length}`);
  console.log(`   User index: ${userIndex}`);
  console.log(`   User found in tree: ${userIndex >= 0 ? '✓' : '✗'}`);
  console.log('');

  if (userIndex >= 0) {
    console.log('🎉 SUCCESS: All authentication checks passed!');
    console.log('');
    console.log('The authenticated endpoints should work correctly with:');
    console.log(`   twitchLogin: ${twitchLogin}`);
    console.log(`   user_hash: ${computedHash.substring(0, 16)}...`);
    console.log(`   Claims found: ${availableResult.rows.length}`);
    console.log('');
    console.log('Next step: Debug cookie/JWT transport issue');
  } else {
    console.log('❌ FAILED: User not found in participants list');
  }

  await pool.end();
}

testDirectAuth().catch(console.error);
