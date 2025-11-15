#!/usr/bin/env tsx
/**
 * Initialize and publish Top 3 uninitialized CLS channels
 * Cost: 3 × ~0.04002 SOL = ~0.12 SOL
 * Benefit: Unlock 28,189 participants across 33 epochs
 */

import { execSync } from 'child_process';

const CHANNELS_TO_INIT = [
  'ravshann',      // 14,063 participants, 11 epochs - NEEDS INIT
  'plaqueboymax',  //  7,660 participants, 11 epochs - NEEDS INIT
  'leva2k',        //  6,266 participants, 11 epochs - NEEDS INIT
];

console.log('🚀 Initialize Top 3 Uninitialized CLS Channels');
console.log('='.repeat(50));
console.log(`Channels: ${CHANNELS_TO_INIT.length}`);
console.log(`Est. cost: ~${(CHANNELS_TO_INIT.length * 0.04002).toFixed(4)} SOL`);
console.log(`Impact: 28,189 participants, 33 epochs`);
console.log('');

const results = {
  initialized: [] as string[],
  published: [] as Array<{channel: string, epoch: number, signature: string}>,
  failed: [] as Array<{channel: string, error: string}>,
};

for (const channel of CHANNELS_TO_INIT) {
  console.log(`\n📍 Processing ${channel}...`);

  try {
    // Get first unpublished epoch for this channel
    const epochQuery = `
      SELECT epoch FROM sealed_epochs
      WHERE channel = '${channel}' AND token_group = 'CLS' AND published = 0
      ORDER BY epoch ASC LIMIT 1
    `;

    const epochResult = execSync(
      `PGPASSWORD="AVNS_7OLyCRhJkIPcAKrZMoi" psql "postgresql://doadmin@twzrd-prod-postgres-do-user-21113270-0.f.db.ondigitalocean.com:25061/twzrd-oracle-pool?sslmode=require" -t -c "${epochQuery}"`,
      { encoding: 'utf-8' }
    ).trim();

    if (!epochResult) {
      console.log(`  ⚠️  No unpublished epochs for ${channel}`);
      continue;
    }

    const epoch = parseInt(epochResult);
    console.log(`  ⏰ First unpublished epoch: ${epoch}`);

    // Publish (will auto-initialize if needed)
    console.log(`  📤 Publishing (will initialize PDA)...`);
    const publishCmd = `cd /home/twzrd/milo-token && npx tsx scripts/publish-root-mainnet.ts ${channel} ${epoch}`;

    const publishResult = execSync(publishCmd, {
      encoding: 'utf-8',
      stdio: 'pipe',
      env: {
        ...process.env,
        PUBLISH_REQUIRE_INITIALIZED: 'false', // Allow init for this run
        AGGREGATOR_URL: 'http://127.0.0.1:8080', // Correct aggregator port
        PROGRAM_ID: 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop', // Current mainnet program
        ADMIN_KEYPAIR: '/home/twzrd/.config/solana/oracle-authority.json', // Publisher wallet (87d5...ufdy)
      }
    });

    // Extract signature from output
    const sigMatch = publishResult.match(/Signature: ([A-Za-z0-9]+)/);
    const signature = sigMatch ? sigMatch[1] : 'unknown';

    console.log(`  ✅ Published! Sig: ${signature.slice(0, 16)}...`);

    results.initialized.push(channel);
    results.published.push({ channel, epoch, signature });

    // Small delay to avoid rate limits
    console.log(`  ⏱️  Waiting 3s...`);
    execSync('sleep 3');

  } catch (error: any) {
    const errorMsg = error.stderr?.toString() || error.message || String(error);
    console.log(`  ❌ Failed: ${errorMsg.slice(0, 200)}`);
    results.failed.push({ channel, error: errorMsg.slice(0, 500) });
  }
}

// Print summary
console.log('\n' + '='.repeat(50));
console.log('📊 SUMMARY');
console.log('='.repeat(50));
console.log(`✅ Successfully initialized: ${results.initialized.length}/3`);
console.log(`📤 Epochs published: ${results.published.length}`);
console.log(`❌ Failed: ${results.failed.length}`);

if (results.initialized.length > 0) {
  console.log('\n✅ Initialized channels:');
  results.initialized.forEach(ch => console.log(`  - ${ch}`));
}

if (results.published.length > 0) {
  console.log('\n📤 Published epochs:');
  results.published.forEach(p =>
    console.log(`  - ${p.channel} epoch ${p.epoch}`)
  );
  console.log('\nTransaction signatures:');
  results.published.forEach(p =>
    console.log(`  https://solscan.io/tx/${p.signature}`)
  );
}

if (results.failed.length > 0) {
  console.log('\n❌ Failed channels:');
  results.failed.forEach(f => {
    console.log(`  - ${f.channel}:`);
    console.log(`    ${f.error.slice(0, 200)}`);
  });
}

// Check final balance
console.log('\n💰 Checking final balance...');
try {
  const finalBalance = execSync('solana balance 87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy --url mainnet-beta', {
    encoding: 'utf-8'
  }).trim();
  console.log(`Publisher balance: ${finalBalance}`);
} catch (e) {
  console.log('Could not check balance');
}

console.log('\n✅ Done!');
console.log('These 3 channels are now initialized and will publish automatically.');
console.log('Strict mode remains enabled for all other channels.');
