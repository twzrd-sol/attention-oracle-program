#!/usr/bin/env tsx
/**
 * Generate proof for test user bypassing HTTP layer
 * This proves the core logic works even if cookie transport has issues
 */

import { keccak_256 } from '@noble/hashes/sha3';
import { DbReaderPg } from './src/lib/db-reader-pg.js';
import { Pool } from 'pg';
import dotenv from 'dotenv';
import { resolve } from 'path';
import * as fs from 'fs';

dotenv.config({ path: resolve(process.cwd(), '../../.env') });

async function main() {
  const twitchLogin = 'dizzybreezyy';

  // Compute user_hash (same as authenticated endpoint)
  const user_hash = Buffer.from(
    keccak_256(Buffer.from(twitchLogin.toLowerCase(), 'utf8'))
  ).toString('hex');

  console.log('🎯 Generating Proof for Test User');
  console.log('');
  console.log('User:', twitchLogin);
  console.log('Hash:', user_hash.substring(0, 16) + '...');
  console.log('');

  // Connect to database
  let DATABASE_URL = process.env.DATABASE_URL!;
  const url = new URL(DATABASE_URL);
  url.searchParams.delete('sslmode');
  DATABASE_URL = url.toString();

  const pool = new Pool({
    connectionString: DATABASE_URL,
    ssl: { rejectUnauthorized: false }
  });

  // Step 1: Find available claims
  console.log('📋 Step 1: Finding available claims...');
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
      AND se.published = 1
    ORDER BY sp.epoch DESC, sp.channel ASC
    LIMIT 10
  `, [user_hash]);

  if (availableResult.rows.length === 0) {
    console.log('❌ No claims found');
    await pool.end();
    return;
  }

  console.log(`✅ Found ${availableResult.rows.length} published claims`);
  availableResult.rows.forEach((row, i) => {
    console.log(`   ${i + 1}. Epoch ${row.epoch}, Channel: ${row.channel}, Index: ${row.index}`);
  });
  console.log('');

  // Use most recent claim
  const claim = availableResult.rows[0];
  const epoch = Number(claim.epoch);
  const channel = claim.channel;
  const expectedIndex = claim.index;

  console.log(`🔍 Step 2: Generating proof for epoch ${epoch}, channel ${channel}...`);

  // Get participants for this epoch/channel
  const participantsResult = await pool.query(`
    SELECT user_hash, idx
    FROM sealed_participants
    WHERE epoch = $1
      AND channel = $2
    ORDER BY idx ASC
  `, [epoch, channel]);

  const participants = participantsResult.rows.map(r => r.user_hash);
  const userIndex = participants.indexOf(user_hash);

  console.log(`   Total participants: ${participants.length}`);
  console.log(`   User index: ${userIndex}`);
  console.log(`   Expected index: ${expectedIndex}`);

  if (userIndex === -1) {
    console.log('❌ User not found in participants list');
    await pool.end();
    return;
  }

  if (userIndex !== expectedIndex) {
    console.log(`⚠️  Warning: Index mismatch! DB says ${expectedIndex} but computed ${userIndex}`);
  }

  // Generate merkle proof using db-reader with existing pool
  const dbReader = new DbReaderPg();
  dbReader.pool = pool; // Reuse existing SSL-configured pool
  const proofData = await dbReader.generateProof(epoch, channel, userIndex, 'MILO', 'default');

  if (!proofData) {
    console.log('❌ Proof generation failed');
    await dbReader.close();
    await pool.end();
    return;
  }

  console.log('✅ Proof generated successfully!');
  console.log('');
  console.log('📄 Proof Data:');
  console.log(`   Root: 0x${proofData.root}`);
  console.log(`   Proof siblings: ${proofData.proof.length}`);
  console.log(`   User hash: ${proofData.user_hash.substring(0, 16)}...`);
  console.log('');

  // Save to file for claim submission
  const claimData = {
    twitchLogin,
    channel,
    epoch,
    index: userIndex,
    root: `0x${proofData.root}`,
    proof: proofData.proof.map(p => `0x${p}`),
    user_hash: proofData.user_hash,
    participantCount: participants.length
  };

  const outputPath = `/tmp/claim-${twitchLogin}-${epoch}.json`;
  fs.writeFileSync(outputPath, JSON.stringify(claimData, null, 2));

  console.log('💾 Claim data saved to:', outputPath);
  console.log('');
  console.log('🚀 Next step: Submit claim to mainnet');
  console.log(`   npx tsx scripts/claims/claim-direct.ts ${outputPath}`);

  await dbReader.close();
  await pool.end();
}

main().catch(console.error);
