#!/usr/bin/env tsx
/**
 * cls-epoch-report.ts
 *
 * Operational reporting tool for CLS epochs.
 * Provides quick summaries of allocation, claim, and confirmation status.
 *
 * Env: DATABASE_URL
 * Usage:
 *   npx tsx scripts/cls-epoch-report.ts --epoch 424245
 *   npx tsx scripts/cls-epoch-report.ts --epoch 424245 --channel test-cls
 *
 * Output: Summary table + key metrics
 */

import { Pool } from 'pg';

interface EpochSummary {
  epoch: number;
  channel: string | null;
  total_allocated: string;
  total_wallets: number;
  confirmed: number;
  pending: number;
  failed: number;
  unclaimed: number;
  confirmed_amount: string;
  merkle_root: string | null;
  sealed_at: string | null;
  last_claim_at: string | null;
}

async function main() {
  const DATABASE_URL = process.env.DATABASE_URL;
  if (!DATABASE_URL) {
    throw new Error('DATABASE_URL environment variable is required');
  }

  const pool = new Pool({ connectionString: DATABASE_URL });

  try {
    const args = process.argv.slice(2);
    const epochIdx = args.findIndex(a => a === '--epoch' || a === '-e');
    const channelIdx = args.findIndex(a => a === '--channel' || a === '-c');

    if (epochIdx === -1 || !args[epochIdx + 1]) {
      throw new Error('Require --epoch <id>');
    }

    const epoch = Number(args[epochIdx + 1]);
    const channel = channelIdx !== -1 ? args[channelIdx + 1] : null;

    if (!Number.isInteger(epoch)) {
      throw new Error(`Invalid epoch: ${args[epochIdx + 1]}`);
    }

    console.log(`\n📊 CLS Epoch Report: Epoch ${epoch}${channel ? ` / Channel ${channel}` : ''}\n`);

    // Query allocations
    const allocRes = await pool.query(
      `
      SELECT
        COUNT(*) as total_wallets,
        SUM(CAST(amount AS NUMERIC)) as total_allocated
      FROM allocations
      WHERE epoch_id = $1
      `,
      [epoch]
    );

    const allocData = allocRes.rows[0];
    if (!allocData || allocData.total_wallets === 0) {
      console.log('  ⚠️  No allocations found for this epoch.');
      await pool.end();
      return;
    }

    // Query claims by status
    const claimsRes = await pool.query(
      `
      SELECT
        COALESCE(tx_status, 'unclaimed') as status,
        COUNT(*) as count,
        MAX(confirmed_at) as last_confirmed,
        SUM(CASE WHEN amount IS NOT NULL THEN CAST(amount AS NUMERIC) ELSE 0 END) as amount
      FROM cls_claims
      WHERE epoch_id = $1
      GROUP BY tx_status
      `,
      [epoch]
    );

    // Query epoch metadata
    const epochRes = await pool.query(
      `
      SELECT
        root as merkle_root,
        to_timestamp(sealed_at) as sealed_at
      FROM sealed_epochs
      WHERE epoch = $1
      `,
      [epoch]
    );

    const epochData = epochRes.rows[0];

    // Parse claim statuses
    const claimsByStatus = new Map<string, { count: number; amount: string; lastAt: string | null }>();
    for (const row of claimsRes.rows) {
      const status = row.status || 'unclaimed';
      const countNum = Number(row.count ?? 0);
      const amtStr = row.amount ? BigInt(Math.floor(Number(row.amount))).toString() : '0';
      claimsByStatus.set(status, { count: countNum, amount: amtStr, lastAt: row.last_confirmed });
    }

    const confirmed = claimsByStatus.get('confirmed') || { count: 0, amount: '0', lastAt: null };
    const pending = claimsByStatus.get('pending') || { count: 0, amount: '0', lastAt: null };
    const failed = claimsByStatus.get('failed') || { count: 0, amount: '0', lastAt: null };
    const totalWallets = Number(allocData.total_wallets ?? 0);
    const unclaimedRaw = totalWallets - (confirmed.count + pending.count + failed.count);
    const unclaimed = unclaimedRaw < 0 ? 0 : unclaimedRaw;

    // Format output
    const totalAllocated = allocData.total_allocated
      ? BigInt(Math.floor(Number(allocData.total_allocated))).toString()
      : '0';
    const confirmedAmount = confirmed.amount;
    const percentClaimed = totalWallets > 0
      ? ((confirmed.count / totalWallets) * 100).toFixed(1)
      : '0.0';

    console.log('📈 Allocation Summary');
    console.log('─'.repeat(60));
    console.log(`  Epoch:              ${epoch}`);
    if (channel) console.log(`  Channel:            ${channel}`);
    if (epochData) {
      console.log(`  Merkle Root:        ${epochData.merkle_root}`);
      console.log(`  Sealed At:          ${epochData.sealed_at}`);
    }
    console.log('');

    console.log('💰 Claim Status');
    console.log('─'.repeat(60));
    console.log(`  Total Allocated:    ${totalAllocated} tokens`);
    console.log(`  Total Wallets:      ${totalWallets}`);
    console.log('');
    console.log(`  ✅ Confirmed:       ${confirmed.count} / ${totalWallets} (${percentClaimed}%)`);
    if (confirmed.count > 0) {
      console.log(`     Amount:         ${confirmedAmount} tokens`);
      console.log(`     Last Claim:     ${confirmed.lastAt || 'N/A'}`);
    }
    console.log(`  ⏳ Pending:         ${pending.count} / ${totalWallets}`);
    console.log(`  ❌ Failed:          ${failed.count} / ${totalWallets}`);
    console.log(`  📋 Unclaimed:       ${unclaimed} / ${totalWallets}`);
    console.log('');

    // Percentages
    console.log('📊 Percentages');
    console.log('─'.repeat(60));
    console.log(`  Claimed:            ${percentClaimed}%`);
    console.log(`  Pending:            ${(totalWallets > 0 ? ((pending.count / totalWallets) * 100) : 0).toFixed(1)}%`);
    console.log(`  Failed:             ${(totalWallets > 0 ? ((failed.count / totalWallets) * 100) : 0).toFixed(1)}%`);
    console.log(`  Unclaimed:          ${(totalWallets > 0 ? ((unclaimed / totalWallets) * 100) : 0).toFixed(1)}%`);
    console.log('');

    // Action items
    if (unclaimed > 0) {
      console.log('⚠️  Action Items');
      console.log('─'.repeat(60));
      console.log(`  ${unclaimed} wallets have not claimed yet`);
      console.log(`  Run: npx tsx scripts/generate-claims-csv.ts --epoch ${epoch}`);
      console.log(`  Then: npx tsx scripts/allocate-and-claim.ts --csv claims.csv`);
      console.log('');
    }

    if (failed.count > 0) {
      console.log('⚠️  Failed Claims');
      console.log('─'.repeat(60));
      console.log(`  ${failed.count} claims failed. Check gateway logs and retry.`);
      console.log('');
    }

    // Optional one-line summary
    if (args.includes('--summary')) {
      const total = totalWallets;
      const conf = confirmed.count;
      const pend = pending.count;
      const fail = failed.count;
      console.log(`\nsummary: epoch=${epoch} wallets=${total} confirmed=${conf} pending=${pend} failed=${fail} claimed_pct=${percentClaimed}% allocated=${totalAllocated}`);
    }

    console.log('✅ Report Complete\n');

    await pool.end();
  } catch (error) {
    console.error('❌ Error:', error instanceof Error ? error.message : error);
    await pool.end();
    process.exit(1);
  }
}

main();
