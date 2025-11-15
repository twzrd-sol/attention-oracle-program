#!/usr/bin/env tsx
/**
 * Generate proof for a specific epoch/channel with a low-index user (< 1024)
 * This bypasses the CHANNEL_MAX_CLAIMS bug for testing
 */

import { DbReaderPg } from '../apps/gateway/src/lib/db-reader-pg.js';
import { Pool } from 'pg';
import dotenv from 'dotenv';
import { resolve } from 'path';
import * as fs from 'fs';

dotenv.config({ path: resolve(__dirname, '../.env') });

async function main() {
  const channel = process.argv[2] || 'jasontheween';
  const epoch = parseInt(process.argv[3] || '1762362000');

  console.log('🎯 Generating Valid Proof for On-Chain Epoch');
  console.log('');
  console.log('Channel:', channel);
  console.log('Epoch:', epoch);
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

  try {
    // Step 1: Verify epoch is published
    console.log('📋 Step 1: Verifying epoch is published...');
    const epochResult = await pool.query(`
      SELECT epoch, root, published, sealed_at
      FROM sealed_epochs
      WHERE channel = $1 AND epoch = $2
    `, [channel, epoch]);

    if (epochResult.rows.length === 0) {
      console.log('❌ Epoch not found in database');
      await pool.end();
      return;
    }

    const epochData = epochResult.rows[0];
    console.log(`✅ Epoch found: published=${epochData.published}, root=0x${epochData.root.substring(0, 12)}...`);
    console.log('');

    // Step 2: Find a participant with low index (< 1024)
    console.log('📋 Step 2: Finding participant with index < 1024...');
    const participantsResult = await pool.query(`
      SELECT user_hash, idx
      FROM sealed_participants
      WHERE epoch = $1 AND channel = $2 AND idx < 1024
      ORDER BY idx ASC
      LIMIT 1
    `, [epoch, channel]);

    if (participantsResult.rows.length === 0) {
      console.log('❌ No participants found with index < 1024');
      await pool.end();
      return;
    }

    const participant = participantsResult.rows[0];
    const userHash = participant.user_hash;
    const index = participant.idx;

    console.log(`✅ Using participant at index ${index}: ${userHash.substring(0, 16)}...`);
    console.log('');

    // Step 3: Generate proof using DbReaderPg
    console.log(`🔍 Step 3: Generating Merkle proof...`);
    const dbReader = new DbReaderPg();
    dbReader.pool = pool; // Reuse existing SSL-configured pool

    const proofData = await dbReader.generateProof(epoch, channel, index, 'MILO', 'default');

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
    console.log(`   Index: ${index}`);

    // Verify root matches on-chain root
    if (proofData.root !== epochData.root) {
      console.log('');
      console.log('⚠️  WARNING: Root mismatch!');
      console.log(`   Generated: 0x${proofData.root}`);
      console.log(`   Database:  0x${epochData.root}`);
    }
    console.log('');

    // Save to file for claim submission
    const claimData = {
      channel,
      epoch,
      index,
      root: `0x${proofData.root}`,
      proof: proofData.proof.map(p => `0x${p}`),
      user_hash: proofData.user_hash,
      amount: "1000000000", // 1 MILO (9 decimals)
    };

    const outputPath = `/tmp/claim-${channel}-${epoch}-valid.json`;
    fs.writeFileSync(outputPath, JSON.stringify(claimData, null, 2));

    console.log('💾 Claim data saved to:', outputPath);
    console.log('');
    console.log('🚀 Next step: Submit claim to mainnet');
    console.log(`   cd /home/twzrd/milo-token && npx tsx scripts/claims/claim-direct.ts ${outputPath}`);
    console.log('');

    await dbReader.close();
    await pool.end();
  } catch (error) {
    console.error('❌ Error:', error);
    await pool.end();
    process.exit(1);
  }
}

main().catch(console.error);
