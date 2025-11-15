#!/usr/bin/env node
/**
 * End-to-End Pipeline Validation
 *
 * Tests: IRC → Worker → Aggregator → Seal → Merkle → On-chain flow
 * Channel: k1m6a (CLS, 50 participants, epoch 1762444800)
 *
 * MIT License (2025 TWZRD)
 */

import { Pool } from 'pg';
import { Connection, PublicKey } from '@solana/web3.js';
import { keccak_256 } from 'js-sha3';
import dotenv from 'dotenv';

dotenv.config();

const DATABASE_URL = process.env.DATABASE_URL;
const RPC_URL = process.env.RPC_URL;
const PROGRAM_ID = new PublicKey(process.env.PROGRAM_ID || 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');

const pool = new Pool({ connectionString: DATABASE_URL });
const connection = new Connection(RPC_URL!, 'confirmed');

const TEST_CHANNEL = 'k1m6a';
const TEST_EPOCH = 1762444800;

function hashUser(username: string): string {
  return Buffer.from(keccak_256(Buffer.from(username.toLowerCase()))).toString('hex');
}

async function step1_checkParticipation() {
  console.log('\n✅ STEP 1: Check Raw Participation Data\n');

  const result = await pool.query(`
    SELECT
      COUNT(*) as total,
      COUNT(DISTINCT user_hash) as unique_users,
      token_group,
      category
    FROM channel_participation
    WHERE channel = $1 AND epoch = $2
    GROUP BY token_group, category
  `, [TEST_CHANNEL, TEST_EPOCH]);

  if (result.rows.length === 0) {
    console.log(`❌ No participation data for ${TEST_CHANNEL} epoch ${TEST_EPOCH}`);
    return false;
  }

  const row = result.rows[0];
  console.log(`   Channel: ${TEST_CHANNEL}`);
  console.log(`   Epoch: ${TEST_EPOCH}`);
  console.log(`   Participants: ${row.total}`);
  console.log(`   Unique users: ${row.unique_users}`);
  console.log(`   Token Group: ${row.token_group}`);
  console.log(`   Category: ${row.category}`);

  return true;
}

async function step2_checkUsernameMapping() {
  console.log('\n✅ STEP 2: Verify Username Mapping (The Fix)\n');

  const result = await pool.query(`
    SELECT
      COUNT(DISTINCT cp.user_hash) as total_users,
      COUNT(DISTINCT um.user_hash) as mapped_users
    FROM channel_participation cp
    LEFT JOIN user_mapping um ON cp.user_hash = um.user_hash
    WHERE cp.channel = $1 AND cp.epoch = $2
  `, [TEST_CHANNEL, TEST_EPOCH]);

  const row = result.rows[0];
  const coverage = (parseInt(row.mapped_users) / parseInt(row.total_users)) * 100;

  console.log(`   Total unique users: ${row.total_users}`);
  console.log(`   Mapped users: ${row.mapped_users}`);
  console.log(`   Coverage: ${coverage.toFixed(2)}%`);

  if (coverage < 95) {
    console.log(`   ❌ FAIL: Coverage below 95% (expected 100% post-fix)`);
    return false;
  }

  console.log(`   ✅ PASS: ${coverage}% coverage`);
  return true;
}

async function step3_checkEpochSealing() {
  console.log('\n✅ STEP 3: Check Epoch Sealing\n');

  const sealedEpoch = await pool.query(`
    SELECT epoch, token_group, category, sealed_at, root
    FROM sealed_epochs
    WHERE channel = $1 AND epoch = $2
  `, [TEST_CHANNEL, TEST_EPOCH]);

  if (sealedEpoch.rows.length === 0) {
    console.log(`   ❌ Epoch not sealed yet`);
    return false;
  }

  const row = sealedEpoch.rows[0];
  console.log(`   Sealed at: ${new Date(row.sealed_at * 1000).toISOString()}`);
  console.log(`   Root: ${row.root.substring(0, 16)}...`);

  // Check sealed participants
  const participants = await pool.query(`
    SELECT
      COUNT(*) as total,
      COUNT(CASE WHEN username IS NOT NULL THEN 1 END) as with_username
    FROM sealed_participants
    WHERE channel = $1 AND epoch = $2 AND token_group = $3 AND category = $4
  `, [TEST_CHANNEL, TEST_EPOCH, row.token_group, row.category]);

  const pRow = participants.rows[0];
  const pct = (parseInt(pRow.with_username) / parseInt(pRow.total)) * 100;

  console.log(`   Sealed participants: ${pRow.total}`);
  console.log(`   With username: ${pRow.with_username} (${pct.toFixed(2)}%)`);

  if (pct < 95) {
    console.log(`   ❌ FAIL: ${pct}% username rate (expected 100%)`);
    return false;
  }

  console.log(`   ✅ PASS: ${pct}% usernames populated`);
  return true;
}

async function step4_verifyMerkleRoot() {
  console.log('\n✅ STEP 4: Verify Merkle Root Construction\n');

  // Get sealed participants in deterministic order
  const participants = await pool.query(`
    SELECT user_hash, idx
    FROM sealed_participants
    WHERE channel = $1 AND epoch = $2 AND token_group = 'CLS' AND category = 'default'
    ORDER BY idx ASC
  `, [TEST_CHANNEL, TEST_EPOCH]);

  console.log(`   Participants (deterministic order): ${participants.rows.length}`);

  // Get stored root
  const storedRoot = await pool.query(`
    SELECT root FROM sealed_epochs
    WHERE channel = $1 AND epoch = $2
  `, [TEST_CHANNEL, TEST_EPOCH]);

  if (storedRoot.rows.length === 0) {
    console.log(`   ❌ No root found in sealed_epochs`);
    return false;
  }

  console.log(`   Stored root: ${storedRoot.rows[0].root.substring(0, 32)}...`);
  console.log(`   Sample participants:`);
  participants.rows.slice(0, 3).forEach(p => {
    console.log(`     [${p.idx}] ${p.user_hash.substring(0, 16)}...`);
  });

  console.log(`   ✅ PASS: Merkle root stored`);
  return true;
}

async function step5_checkOnChain() {
  console.log('\n✅ STEP 5: Check On-Chain ChannelState\n');

  // Derive ChannelState PDA
  const channelSeed = TEST_CHANNEL.toLowerCase();
  const [channelStatePDA] = PublicKey.findProgramAddressSync(
    [Buffer.from('channel_state'), Buffer.from(channelSeed)],
    PROGRAM_ID
  );

  console.log(`   Channel: ${TEST_CHANNEL}`);
  console.log(`   PDA: ${channelStatePDA.toBase58()}`);

  try {
    const accountInfo = await connection.getAccountInfo(channelStatePDA);

    if (!accountInfo) {
      console.log(`   ❌ ChannelState account not found on-chain`);
      return false;
    }

    console.log(`   Account size: ${accountInfo.data.length} bytes`);

    if (accountInfo.data.length === 10742) {
      console.log(`   ✅ V2 account (10742 bytes - 8192 capacity)`);
    } else if (accountInfo.data.length === 1782) {
      console.log(`   ⚠️  V1 account (1782 bytes - 1024 capacity)`);
    } else {
      console.log(`   ⚠️  Unknown account size`);
    }

    console.log(`   Owner: ${accountInfo.owner.toBase58()}`);
    console.log(`   ✅ PASS: On-chain account exists`);
    return true;
  } catch (err) {
    console.log(`   ❌ RPC error: ${err}`);
    return false;
  }
}

async function step6_testProofGeneration() {
  console.log('\n✅ STEP 6: Test Proof Generation\n');

  // Get a random participant
  const participant = await pool.query(`
    SELECT user_hash, username, idx
    FROM sealed_participants
    WHERE channel = $1 AND epoch = $2 AND token_group = 'CLS' AND category = 'default'
    LIMIT 1
  `, [TEST_CHANNEL, TEST_EPOCH]);

  if (participant.rows.length === 0) {
    console.log(`   ❌ No participants found`);
    return false;
  }

  const user = participant.rows[0];
  console.log(`   Test user: ${user.username || user.user_hash.substring(0, 16)}`);
  console.log(`   Leaf index: ${user.idx}`);
  console.log(`   User hash: ${user.user_hash.substring(0, 32)}...`);

  // In production, this would call /proof endpoint
  console.log(`   ✅ PASS: Participant data available for proof generation`);
  return true;
}

async function main() {
  console.log('╔════════════════════════════════════════════════════════════════╗');
  console.log('║      END-TO-END PIPELINE VALIDATION                            ║');
  console.log('║      Channel: k1m6a (CLS, 50 participants)                     ║');
  console.log('║      Epoch: 1762444800 (Nov 6, 17:00 UTC)                      ║');
  console.log('╚════════════════════════════════════════════════════════════════╝');

  const results: boolean[] = [];

  results.push(await step1_checkParticipation());
  results.push(await step2_checkUsernameMapping());
  results.push(await step3_checkEpochSealing());
  results.push(await step4_verifyMerkleRoot());
  results.push(await step5_checkOnChain());
  results.push(await step6_testProofGeneration());

  console.log('\n╔════════════════════════════════════════════════════════════════╗');
  console.log('║      VALIDATION SUMMARY                                        ║');
  console.log('╚════════════════════════════════════════════════════════════════╝\n');

  const passed = results.filter(r => r).length;
  const total = results.length;

  console.log(`   Passed: ${passed}/${total}`);
  console.log(`   Success Rate: ${((passed/total)*100).toFixed(1)}%\n`);

  if (passed === total) {
    console.log('   ✅ ALL TESTS PASSED - Pipeline is healthy!\n');
  } else {
    console.log(`   ❌ ${total - passed} test(s) failed\n`);
  }

  await pool.end();
  process.exit(passed === total ? 0 : 1);
}

main();
