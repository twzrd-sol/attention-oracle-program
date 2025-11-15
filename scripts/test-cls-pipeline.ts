#!/usr/bin/env node
/**
 * CLS Pipeline End-to-End Test Suite
 *
 * Purpose: Validate every angle of CLS (Crypto category) data collection
 *
 * Test Coverage:
 * 1. Discovery: Verify crypto channels are being discovered
 * 2. Classification: Confirm token_group=CLS, category=crypto assignment
 * 3. Collection: Validate worker → aggregator data flow
 * 4. Username Mapping: Ensure user_mapping is populated (the fix)
 * 5. Epoch Sealing: Check sealed_participants has token_group/category
 * 6. Merkle Trees: Verify CLS trees are being built
 * 7. On-Chain: Confirm roots are published to Solana (optional)
 *
 * Exit Codes:
 * 0 = All tests passed
 * 1 = One or more tests failed
 */

import { Pool } from 'pg';
import { keccak_256 } from 'js-sha3';
import dotenv from 'dotenv';

dotenv.config();

const DATABASE_URL = process.env.DATABASE_URL;
const AGGREGATOR_URL = process.env.AGGREGATOR_URL || 'http://127.0.0.1:8080';

if (!DATABASE_URL) {
  console.error('❌ Missing DATABASE_URL');
  process.exit(1);
}

const pool = new Pool({
  connectionString: DATABASE_URL,
});

interface TestResult {
  name: string;
  passed: boolean;
  message: string;
  details?: any;
}

const results: TestResult[] = [];

function pass(name: string, message: string, details?: any) {
  results.push({ name, passed: true, message, details });
  console.log(`✅ ${name}: ${message}`);
  if (details) console.log(`   ${JSON.stringify(details, null, 2)}`);
}

function fail(name: string, message: string, details?: any) {
  results.push({ name, passed: false, message, details });
  console.log(`❌ ${name}: ${message}`);
  if (details) console.log(`   ${JSON.stringify(details, null, 2)}`);
}

// Test 1: CLS Discovery System
async function testClsDiscovery() {
  console.log('\n📡 TEST 1: CLS Discovery System\n');

  try {
    // Check if cls_discovered_channels table exists
    const tableCheck = await pool.query(`
      SELECT COUNT(*) as count
      FROM information_schema.tables
      WHERE table_name = 'cls_discovered_channels'
    `);

    if (parseInt(tableCheck.rows[0].count) === 0) {
      fail('CLS Discovery Table', 'cls_discovered_channels table does not exist');
      return;
    }

    pass('CLS Discovery Table', 'cls_discovered_channels table exists');

    // Check active CLS channels
    const channelsResult = await pool.query(`
      SELECT DISTINCT channel_name, category, discovered_at
      FROM cls_discovered_channels
      WHERE discovered_at >= NOW() - INTERVAL '7 days'
      ORDER BY discovered_at DESC
      LIMIT 20
    `);

    if (channelsResult.rows.length === 0) {
      fail('CLS Active Channels', 'No CLS channels discovered in last 7 days', {
        hint: 'Run: npm run discover-cls-categories'
      });
      return;
    }

    pass('CLS Active Channels', `Found ${channelsResult.rows.length} active CLS channels`, {
      sample: channelsResult.rows.slice(0, 5).map(r => ({
        channel: r.channel_name,
        category: r.category,
        discovered: new Date(r.discovered_at).toISOString()
      }))
    });

    // Check crypto category specifically
    const cryptoChannels = await pool.query(`
      SELECT COUNT(DISTINCT channel_name) as count
      FROM cls_discovered_channels
      WHERE category = 'crypto'
        AND discovered_at >= NOW() - INTERVAL '7 days'
    `);

    const cryptoCount = parseInt(cryptoChannels.rows[0].count);
    if (cryptoCount === 0) {
      fail('Crypto Channels', 'No crypto category channels found');
    } else {
      pass('Crypto Channels', `${cryptoCount} crypto channels discovered`);
    }

  } catch (err) {
    fail('CLS Discovery', `Discovery test failed: ${err}`);
  }
}

// Test 2: Channel Classification
async function testChannelClassification() {
  console.log('\n🏷️  TEST 2: Channel Classification\n');

  try {
    // Get a sample of recent participation with classification
    const result = await pool.query(`
      SELECT
        channel,
        token_group,
        category,
        COUNT(*) as participant_count,
        MAX(epoch) as latest_epoch
      FROM channel_participation
      WHERE epoch >= $1
      GROUP BY channel, token_group, category
      ORDER BY participant_count DESC
      LIMIT 20
    `, [Math.floor(Date.now() / 1000) - 86400]); // Last 24h

    if (result.rows.length === 0) {
      fail('Classification Data', 'No recent channel_participation records found');
      return;
    }

    pass('Classification Data', `Found ${result.rows.length} classified channels in last 24h`);

    // Check for CLS token group
    const clsChannels = result.rows.filter(r => r.token_group === 'CLS');
    if (clsChannels.length === 0) {
      fail('CLS Token Group', 'No channels classified as token_group=CLS', {
        hint: 'Check if CLS channels are being tracked by worker'
      });
    } else {
      pass('CLS Token Group', `${clsChannels.length} channels classified as CLS`, {
        sample: clsChannels.slice(0, 3).map(r => ({
          channel: r.channel,
          category: r.category,
          participants: r.participant_count
        }))
      });
    }

    // Check for MILO token group
    const miloChannels = result.rows.filter(r => r.token_group === 'MILO');
    if (miloChannels.length === 0) {
      fail('MILO Token Group', 'No channels classified as token_group=MILO');
    } else {
      pass('MILO Token Group', `${miloChannels.length} channels classified as MILO`);
    }

  } catch (err) {
    fail('Channel Classification', `Classification test failed: ${err}`);
  }
}

// Test 3: Username Mapping (The Fix)
async function testUsernameMapping() {
  console.log('\n👤 TEST 3: Username Mapping (Data Loss Fix)\n');

  try {
    // Check recent user_mapping entries
    const recentMappings = await pool.query(`
      SELECT COUNT(*) as count
      FROM user_mapping
      WHERE first_seen >= $1
    `, [Math.floor(Date.now() / 1000) - 3600]); // Last hour

    const recentCount = parseInt(recentMappings.rows[0].count);
    if (recentCount === 0) {
      fail('Recent Mappings', 'No new user mappings in last hour', {
        hint: 'Check if aggregator ingest is calling upsertUsernameMapping()'
      });
    } else {
      pass('Recent Mappings', `${recentCount} new users mapped in last hour`);
    }

    // Check mapping coverage for recent participation
    const coverageResult = await pool.query(`
      SELECT
        COUNT(DISTINCT cp.user_hash) as total_users,
        COUNT(DISTINCT um.user_hash) as mapped_users
      FROM channel_participation cp
      LEFT JOIN user_mapping um ON cp.user_hash = um.user_hash
      WHERE cp.epoch >= $1
    `, [Math.floor(Date.now() / 1000) - 3600]);

    const totalUsers = parseInt(coverageResult.rows[0].total_users);
    const mappedUsers = parseInt(coverageResult.rows[0].mapped_users);
    const coverageRate = totalUsers > 0 ? (mappedUsers / totalUsers) * 100 : 0;

    if (coverageRate < 95) {
      fail('Mapping Coverage', `Only ${coverageRate.toFixed(2)}% coverage (expected >95%)`, {
        total: totalUsers,
        mapped: mappedUsers
      });
    } else {
      pass('Mapping Coverage', `${coverageRate.toFixed(2)}% of recent users mapped`, {
        total: totalUsers,
        mapped: mappedUsers
      });
    }

  } catch (err) {
    fail('Username Mapping', `Mapping test failed: ${err}`);
  }
}

// Test 4: Epoch Sealing with Categories
async function testEpochSealing() {
  console.log('\n🔒 TEST 4: Epoch Sealing with Token Groups\n');

  try {
    // Check recent sealed epochs
    const sealedResult = await pool.query(`
      SELECT
        epoch,
        token_group,
        category,
        COUNT(DISTINCT channel) as channel_count
      FROM sealed_epochs
      WHERE epoch >= $1
      GROUP BY epoch, token_group, category
      ORDER BY epoch DESC
      LIMIT 20
    `, [Math.floor(Date.now() / 1000) - 86400]);

    if (sealedResult.rows.length === 0) {
      fail('Sealed Epochs', 'No epochs sealed in last 24h');
      return;
    }

    pass('Sealed Epochs', `${sealedResult.rows.length} epoch/category combinations sealed`);

    // Check for CLS sealed epochs
    const clsSealed = sealedResult.rows.filter(r => r.token_group === 'CLS');
    if (clsSealed.length === 0) {
      fail('CLS Sealed Epochs', 'No CLS epochs sealed', {
        hint: 'Check if CLS channels have participants to seal'
      });
    } else {
      pass('CLS Sealed Epochs', `${clsSealed.length} CLS epoch/category sealed`, {
        sample: clsSealed.slice(0, 3)
      });
    }

    // Check sealed_participants with usernames
    const participantsResult = await pool.query(`
      SELECT
        token_group,
        category,
        COUNT(*) as total,
        COUNT(CASE WHEN username IS NOT NULL THEN 1 END) as with_username
      FROM sealed_participants
      WHERE epoch >= $1
      GROUP BY token_group, category
    `, [Math.floor(Date.now() / 1000) - 3600]);

    for (const row of participantsResult.rows) {
      const total = parseInt(row.total);
      const withUsername = parseInt(row.with_username);
      const rate = total > 0 ? (withUsername / total) * 100 : 0;

      if (rate < 95) {
        fail('Sealed Username Rate', `${row.token_group}/${row.category}: ${rate.toFixed(2)}% (expected >95%)`, {
          total,
          with_username: withUsername
        });
      } else {
        pass('Sealed Username Rate', `${row.token_group}/${row.category}: ${rate.toFixed(2)}%`, {
          total,
          with_username: withUsername
        });
      }
    }

  } catch (err) {
    fail('Epoch Sealing', `Sealing test failed: ${err}`);
  }
}

// Test 5: Merkle Tree Building
async function testMerkleTreeBuilding() {
  console.log('\n🌳 TEST 5: Merkle Tree Building\n');

  try {
    // Check if merkle_roots table exists
    const tableCheck = await pool.query(`
      SELECT COUNT(*) as count
      FROM information_schema.tables
      WHERE table_name = 'merkle_roots'
    `);

    if (parseInt(tableCheck.rows[0].count) === 0) {
      fail('Merkle Roots Table', 'merkle_roots table does not exist');
      return;
    }

    pass('Merkle Roots Table', 'merkle_roots table exists');

    // Check recent merkle roots
    const rootsResult = await pool.query(`
      SELECT
        epoch,
        channel,
        root,
        participant_count
      FROM merkle_roots
      WHERE epoch >= $1
      ORDER BY epoch DESC
      LIMIT 20
    `, [Math.floor(Date.now() / 1000) - 86400]);

    if (rootsResult.rows.length === 0) {
      fail('Recent Merkle Roots', 'No merkle roots built in last 24h');
      return;
    }

    pass('Recent Merkle Roots', `${rootsResult.rows.length} merkle trees built`, {
      sample: rootsResult.rows.slice(0, 3).map(r => ({
        epoch: r.epoch,
        channel: r.channel,
        participants: r.participant_count,
        root: r.root.substring(0, 16) + '...'
      }))
    });

    // Check for CLS channel merkle roots
    const clsRoots = await pool.query(`
      SELECT COUNT(*) as count
      FROM merkle_roots mr
      INNER JOIN cls_discovered_channels cdc ON mr.channel = cdc.channel_name
      WHERE mr.epoch >= $1
    `, [Math.floor(Date.now() / 1000) - 86400]);

    const clsRootCount = parseInt(clsRoots.rows[0].count);
    if (clsRootCount === 0) {
      fail('CLS Merkle Roots', 'No merkle roots built for CLS channels');
    } else {
      pass('CLS Merkle Roots', `${clsRootCount} CLS channel roots built`);
    }

  } catch (err) {
    fail('Merkle Tree Building', `Tree building test failed: ${err}`);
  }
}

// Test 6: Aggregator Health
async function testAggregatorHealth() {
  console.log('\n💓 TEST 6: Aggregator Health\n');

  try {
    const response = await fetch(`${AGGREGATOR_URL}/health`);

    if (!response.ok) {
      fail('Aggregator Health', `Health check failed: ${response.status}`);
      return;
    }

    const health = await response.json();
    pass('Aggregator Health', 'Aggregator is running', health);

  } catch (err) {
    fail('Aggregator Health', `Cannot connect to aggregator: ${err}`);
  }
}

// Test 7: End-to-End Flow Test
async function testEndToEndFlow() {
  console.log('\n🔄 TEST 7: End-to-End Flow Validation\n');

  try {
    // Pick a recent CLS channel with participation
    const clsChannel = await pool.query(`
      SELECT
        cp.channel,
        cp.epoch,
        cp.token_group,
        cp.category,
        COUNT(DISTINCT cp.user_hash) as participant_count
      FROM channel_participation cp
      INNER JOIN cls_discovered_channels cdc ON cp.channel = cdc.channel_name
      WHERE cp.epoch >= $1
        AND cp.token_group = 'CLS'
      GROUP BY cp.channel, cp.epoch, cp.token_group, cp.category
      ORDER BY cp.epoch DESC
      LIMIT 1
    `, [Math.floor(Date.now() / 1000) - 3600]);

    if (clsChannel.rows.length === 0) {
      fail('E2E Flow', 'No recent CLS participation found for end-to-end test');
      return;
    }

    const testChannel = clsChannel.rows[0];
    pass('E2E Test Data', `Testing with ${testChannel.channel} (epoch ${testChannel.epoch})`);

    // Step 1: Verify participation was collected
    pass('E2E Step 1', `✓ Participation collected (${testChannel.participant_count} users)`);

    // Step 2: Check if epoch was sealed
    const sealedCheck = await pool.query(`
      SELECT COUNT(*) as count
      FROM sealed_epochs
      WHERE epoch = $1 AND channel = $2 AND token_group = $3 AND category = $4
    `, [testChannel.epoch, testChannel.channel, testChannel.token_group, testChannel.category]);

    if (parseInt(sealedCheck.rows[0].count) === 0) {
      fail('E2E Step 2', 'Epoch not sealed yet (may still be active)');
    } else {
      pass('E2E Step 2', '✓ Epoch sealed');

      // Step 3: Check merkle root
      const rootCheck = await pool.query(`
        SELECT root
        FROM merkle_roots
        WHERE epoch = $1 AND channel = $2
      `, [testChannel.epoch, testChannel.channel]);

      if (rootCheck.rows.length === 0) {
        fail('E2E Step 3', 'Merkle root not built');
      } else {
        pass('E2E Step 3', `✓ Merkle root built: ${rootCheck.rows[0].root.substring(0, 16)}...`);
      }
    }

  } catch (err) {
    fail('End-to-End Flow', `E2E test failed: ${err}`);
  }
}

// Main test runner
async function runTests() {
  console.log('╔════════════════════════════════════════════════════════════════╗');
  console.log('║      CLS PIPELINE END-TO-END TEST SUITE                        ║');
  console.log('╚════════════════════════════════════════════════════════════════╝');

  await testClsDiscovery();
  await testChannelClassification();
  await testUsernameMapping();
  await testEpochSealing();
  await testMerkleTreeBuilding();
  await testAggregatorHealth();
  await testEndToEndFlow();

  // Summary
  console.log('\n╔════════════════════════════════════════════════════════════════╗');
  console.log('║      TEST SUMMARY                                              ║');
  console.log('╚════════════════════════════════════════════════════════════════╝\n');

  const passed = results.filter(r => r.passed).length;
  const failed = results.filter(r => !r.passed).length;
  const total = results.length;

  console.log(`Total Tests:  ${total}`);
  console.log(`Passed:       ${passed} ✅`);
  console.log(`Failed:       ${failed} ${failed > 0 ? '❌' : ''}`);
  console.log(`Success Rate: ${((passed / total) * 100).toFixed(1)}%\n`);

  if (failed > 0) {
    console.log('❌ FAILED TESTS:\n');
    results.filter(r => !r.passed).forEach(r => {
      console.log(`  • ${r.name}: ${r.message}`);
    });
    console.log('');
  }

  await pool.end();
  process.exit(failed > 0 ? 1 : 0);
}

runTests();
