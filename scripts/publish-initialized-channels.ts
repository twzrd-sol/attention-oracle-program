#!/usr/bin/env tsx
/**
 * Publish backlog for already-initialized CLS channels
 * Cost: ~0.00 SOL (no inits, fee-only publishes)
 * Channels: eslcs, nooreax, bysl4m, summit1g
 */

import { execSync } from 'child_process';

const INITIALIZED_CHANNELS = [
  'eslcs',    //  8,159 participants, 10 epochs - ALREADY INIT
  'nooreax',  //  5,615 participants, 10 epochs - ALREADY INIT
  'bysl4m',   //  3,796 participants,  8 epochs - ALREADY INIT
  'summit1g', //  3,449 participants, 10 epochs - ALREADY INIT
];

console.log('📤 Publish Backlog for Initialized CLS Channels');
console.log('='.repeat(50));
console.log(`Channels: ${INITIALIZED_CHANNELS.length} (already initialized)`);
console.log(`Cost: Fee-only (~0.00001 SOL × epochs published)`);
console.log('');

const results = {
  published: [] as Array<{channel: string, epoch: number, signature: string}>,
  failed: [] as Array<{channel: string, epoch: number, error: string}>,
};

for (const channel of INITIALIZED_CHANNELS) {
  console.log(`\n📍 Publishing ${channel} backlog...`);

  // Get all unpublished epochs for this channel
  const epochQuery = `
    SELECT epoch FROM sealed_epochs
    WHERE channel = '${channel}' AND token_group = 'CLS' AND published = 0
    ORDER BY epoch ASC
  `;

  try {
    const epochsResult = execSync(
      `PGPASSWORD="AVNS_7OLyCRhJkIPcAKrZMoi" psql "postgresql://doadmin@twzrd-prod-postgres-do-user-21113270-0.f.db.ondigitalocean.com:25061/twzrd-oracle-pool?sslmode=require" -t -c "${epochQuery}"`,
      { encoding: 'utf-8' }
    ).trim();

    if (!epochsResult) {
      console.log(`  ✅ No unpublished epochs for ${channel}`);
      continue;
    }

    const epochs = epochsResult.split('\n').map(e => parseInt(e.trim())).filter(e => e > 0);
    console.log(`  📊 Found ${epochs.length} unpublished epochs`);

    for (const epoch of epochs) {
      try {
        console.log(`  📤 Publishing epoch ${epoch}...`);

        const publishResult = execSync(
          `cd /home/twzrd/milo-token && env AGGREGATOR_URL=http://127.0.0.1:8080 PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop npx tsx scripts/publish-root-mainnet.ts ${channel} ${epoch}`,
          { encoding: 'utf-8', stdio: 'pipe' }
        );

        // Extract signature
        const sigMatch = publishResult.match(/Signature.*?([A-Za-z0-9]{87,88})/);
        const signature = sigMatch ? sigMatch[1] : 'unknown';

        console.log(`     ✅ ${signature.slice(0, 16)}...`);
        results.published.push({ channel, epoch, signature });

        // Small delay
        execSync('sleep 1');

      } catch (error: any) {
        const errorMsg = error.stderr?.toString() || error.message || String(error);
        // Check if already published (expected for some)
        if (errorMsg.includes('EpochAlreadyInitialized')) {
          console.log(`     ⏭️  Already published, skipping`);
        } else {
          console.log(`     ❌ Failed: ${errorMsg.slice(0, 100)}`);
          results.failed.push({ channel, epoch, error: errorMsg.slice(0, 300) });
        }
      }
    }

  } catch (error: any) {
    console.log(`  ❌ Query failed for ${channel}`);
    results.failed.push({ channel, epoch: 0, error: String(error).slice(0, 200) });
  }
}

// Print summary
console.log('\n' + '='.repeat(50));
console.log('📊 SUMMARY');
console.log('='.repeat(50));
console.log(`✅ Successfully published: ${results.published.length} epochs`);
console.log(`❌ Failed: ${results.failed.length}`);

if (results.published.length > 0) {
  console.log(`\n✅ Published ${results.published.length} epochs across ${INITIALIZED_CHANNELS.length} channels`);

  // Group by channel
  const byChannel: Record<string, number> = {};
  results.published.forEach(p => {
    byChannel[p.channel] = (byChannel[p.channel] || 0) + 1;
  });

  Object.entries(byChannel).forEach(([channel, count]) => {
    console.log(`  - ${channel}: ${count} epochs`);
  });
}

if (results.failed.length > 0 && results.failed.length < 10) {
  console.log('\n❌ Failed publishes:');
  results.failed.forEach(f =>
    console.log(`  - ${f.channel} epoch ${f.epoch}: ${f.error.slice(0, 80)}`)
  );
}

// Check final balance
console.log('\n💰 Checking balance...');
try {
  const adminBalance = execSync('solana balance AmMftc4zHgR4yYfv29awV9Q46emo2aGPFW8utP81CsBv --url mainnet-beta', {
    encoding: 'utf-8'
  }).trim();
  console.log(`Admin wallet: ${adminBalance}`);
} catch (e) {
  console.log('Could not check balance');
}

console.log('\n✅ Done! These channels will now publish automatically each epoch.');
