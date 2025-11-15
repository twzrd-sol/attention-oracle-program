#!/usr/bin/env tsx
/**
 * Initialize 5 High-ROI CLS Channels
 *
 * Channels:
 * - loud_coringa (3,532 users)
 * - theburntpeanut (2,947 users)
 * - hanjoudesu (1,858 users)
 * - sheviiioficial (1,680 users)
 * - lacari (1,598 users)
 *
 * Total cost: ~0.2001 SOL (5 × 0.04002 SOL)
 * Total impact: 11,615 users
 *
 * Usage:
 *   npx tsx scripts/init-high-roi-5.ts
 */

import { execSync } from 'child_process';

// Channels to initialize (high user count, not yet initialized)
const CHANNELS_TO_INIT = [
  { name: 'loud_coringa', users: 3532, description: 'Brazilian streamer' },
  { name: 'theburntpeanut', users: 2947, description: 'Gaming content' },
  { name: 'hanjoudesu', users: 1858, description: 'Variety streamer' },
  { name: 'sheviiioficial', users: 1680, description: 'Latin American creator' },
  { name: 'lacari', users: 1598, description: 'Gaming/variety' },
];

const INIT_COST_PER_CHANNEL = 0.04002; // SOL
const TOTAL_COST = CHANNELS_TO_INIT.length * INIT_COST_PER_CHANNEL;
const TOTAL_USERS = CHANNELS_TO_INIT.reduce((sum, ch) => sum + ch.users, 0);

interface ChannelResult {
  channel: string;
  success: boolean;
  signature?: string;
  error?: string;
}

async function main() {
  console.log('🚀 Initialize 5 High-ROI CLS Channels');
  console.log('==================================================');
  console.log(`Channels: ${CHANNELS_TO_INIT.length}`);
  console.log(`Est. cost: ~${TOTAL_COST.toFixed(4)} SOL`);
  console.log(`Impact: ${TOTAL_USERS.toLocaleString()} users`);
  console.log('');

  // Check publisher balance first
  const balanceStr = execSync('solana balance 87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy --url mainnet-beta', {
    encoding: 'utf-8'
  }).trim();
  const balanceSol = parseFloat(balanceStr.split(' ')[0]);

  console.log(`💰 Publisher balance: ${balanceSol} SOL`);
  if (balanceSol < TOTAL_COST + 0.01) {
    console.error(`❌ Insufficient balance! Need ${TOTAL_COST + 0.01} SOL (cost + buffer)`);
    process.exit(1);
  }
  console.log('');

  const results: ChannelResult[] = [];
  const signatures: string[] = [];

  for (const channel of CHANNELS_TO_INIT) {
    console.log(`📍 Processing ${channel.name}...`);
    console.log(`  👥 Users: ${channel.users.toLocaleString()}`);
    console.log(`  📝 ${channel.description}`);

    try {
      // Use known first unpublished epoch (same for all channels from this batch)
      // This is the epoch when strict mode was enabled
      const firstEpoch = 1762495200;
      console.log(`  ⏰ First unpublished epoch: ${firstEpoch}`);

      // Publish first epoch (will initialize PDA)
      console.log(`  📤 Publishing (will initialize PDA)...`);

      const output = execSync(
        `cd /home/twzrd/milo-token && env AGGREGATOR_URL=http://127.0.0.1:8080 PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop ADMIN_KEYPAIR=/home/twzrd/.config/solana/oracle-authority.json PUBLISH_REQUIRE_INITIALIZED=false npx tsx scripts/publish-root-mainnet.ts ${channel.name} ${firstEpoch}`,
        { encoding: 'utf-8', stdio: 'pipe' }
      );

      // Extract signature from output
      const sigMatch = output.match(/Signature\s*:\s*([A-Za-z0-9]{87,88})/);
      const signature = sigMatch ? sigMatch[1] : 'unknown';

      console.log(`  ✅ Published! Sig: ${signature.slice(0, 16)}...`);
      signatures.push(`https://solscan.io/tx/${signature}`);
      results.push({ channel: channel.name, success: true, signature });

      // Wait between operations
      console.log('  ⏱️  Waiting 3s...');
      await new Promise(resolve => setTimeout(resolve, 3000));
      console.log('');

    } catch (err: any) {
      const errorMsg = err.message || String(err);
      console.log(`  ❌ Failed: ${errorMsg.slice(0, 100)}`);
      results.push({ channel: channel.name, success: false, error: errorMsg });
      console.log('');
    }
  }

  // Summary
  console.log('==================================================');
  console.log('📊 SUMMARY');
  console.log('==================================================');

  const successCount = results.filter(r => r.success).length;
  const failCount = results.filter(r => !r.success).length;

  console.log(`✅ Successfully initialized: ${successCount}/${CHANNELS_TO_INIT.length}`);
  console.log(`❌ Failed: ${failCount}`);
  console.log('');

  if (successCount > 0) {
    console.log('✅ Initialized channels:');
    results.filter(r => r.success).forEach(r => {
      console.log(`  - ${r.channel}`);
    });
    console.log('');
  }

  if (failCount > 0) {
    console.log('❌ Failed channels:');
    results.filter(r => !r.success).forEach(r => {
      console.log(`  - ${r.channel}:`, r.error);
    });
    console.log('');
  }

  if (signatures.length > 0) {
    console.log('Transaction signatures:');
    signatures.forEach(sig => console.log(`  ${sig}`));
    console.log('');
  }

  // Check final balance
  console.log('💰 Checking final balance...');
  const finalBalance = execSync('solana balance 87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy --url mainnet-beta', {
    encoding: 'utf-8'
  }).trim();
  console.log(`Publisher balance: ${finalBalance}`);
  console.log('');

  console.log('✅ Done!');
  console.log('These channels are now initialized and will publish automatically.');
  console.log('Strict mode remains enabled for all other channels.');
}

main().catch(err => {
  console.error('Fatal error:', err);
  process.exit(1);
});
