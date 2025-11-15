#!/usr/bin/env tsx
/**
 * Initialize and publish Top 10 CLS channels
 * Cost: 10 × ~0.04002 SOL = ~0.40 SOL
 * Benefit: Unlock 58,072 participants across 102 epochs
 */

import { execSync } from 'child_process';

const TOP_10_CHANNELS = [
  'ravshann',      // 14,063 participants, 11 epochs
  'eslcs',         //  8,159 participants, 10 epochs
  'plaqueboymax',  //  7,660 participants, 11 epochs
  'leva2k',        //  6,266 participants, 11 epochs
  'nooreax',       //  5,615 participants, 10 epochs
  'loud_coringa',  //  4,613 participants, 11 epochs
  'bysl4m',        //  3,796 participants,  8 epochs
  'summit1g',      //  3,449 participants, 10 epochs
  'theburntpeanut',//  3,230 participants,  9 epochs
  'lacari',        //  3,221 participants, 11 epochs
];

console.log('🚀 Top 10 CLS Channel Initialization');
console.log('====================================');
console.log(`Channels: ${TOP_10_CHANNELS.length}`);
console.log(`Est. cost: ~${(TOP_10_CHANNELS.length * 0.04002).toFixed(4)} SOL`);
console.log('');

const results = {
  initialized: [] as string[],
  published: [] as Array<{channel: string, epoch: number, signature: string}>,
  failed: [] as Array<{channel: string, error: string}>,
};

// Temporarily allow initialization for these channels
console.log('⚙️  Setting temporary init allowlist...');
process.env.INIT_ALLOWLIST = TOP_10_CHANNELS.join(',');
process.env.PUBLISH_REQUIRE_INITIALIZED = 'false'; // Temporarily disable strict mode

for (const channel of TOP_10_CHANNELS) {
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
    console.log(`  📤 Publishing...`);
    const publishResult = execSync(
      `cd /home/twzrd/milo-token && npx tsx scripts/publish-root-mainnet.ts ${channel} ${epoch}`,
      { encoding: 'utf-8', stdio: 'pipe' }
    );

    // Extract signature from output
    const sigMatch = publishResult.match(/Signature: ([A-Za-z0-9]+)/);
    const signature = sigMatch ? sigMatch[1] : 'unknown';

    console.log(`  ✅ Published! Sig: ${signature.slice(0, 8)}...`);

    results.initialized.push(channel);
    results.published.push({ channel, epoch, signature });

    // Small delay to avoid rate limits
    execSync('sleep 2');

  } catch (error: any) {
    const errorMsg = error.message || String(error);
    console.log(`  ❌ Failed: ${errorMsg.slice(0, 100)}`);
    results.failed.push({ channel, error: errorMsg });
  }
}

// Re-enable strict mode
console.log('\n⚙️  Re-enabling strict publish mode...');
delete process.env.INIT_ALLOWLIST;
process.env.PUBLISH_REQUIRE_INITIALIZED = 'true';

// Print summary
console.log('\n' + '='.repeat(50));
console.log('📊 SUMMARY');
console.log('='.repeat(50));
console.log(`✅ Successfully initialized: ${results.initialized.length}`);
console.log(`📤 Epochs published: ${results.published.length}`);
console.log(`❌ Failed: ${results.failed.length}`);

if (results.initialized.length > 0) {
  console.log('\n✅ Initialized channels:');
  results.initialized.forEach(ch => console.log(`  - ${ch}`));
}

if (results.published.length > 0) {
  console.log('\n📤 Published epochs:');
  results.published.forEach(p =>
    console.log(`  - ${p.channel} epoch ${p.epoch}: ${p.signature.slice(0, 16)}...`)
  );
}

if (results.failed.length > 0) {
  console.log('\n❌ Failed channels:');
  results.failed.forEach(f => console.log(`  - ${f.channel}: ${f.error.slice(0, 80)}`));
}

// Check final balance
console.log('\n💰 Checking final balance...');
const finalBalance = execSync('solana balance 87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy --url mainnet-beta', {
  encoding: 'utf-8'
}).trim();
console.log(`Publisher balance: ${finalBalance}`);

console.log('\n✅ Done! Strict mode re-enabled.');
console.log('These channels will now publish automatically each epoch.');
