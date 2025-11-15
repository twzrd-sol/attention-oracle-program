#!/usr/bin/env tsx
/**
 * SOL Spend Tracker - Monitor all Solana transaction costs
 *
 * Tracks:
 * 1. Publisher transaction fees (merkle root publishing)
 * 2. Initialize channel state transactions
 * 3. Total SOL spend per channel/epoch/day
 * 4. Compute unit usage
 *
 * Usage:
 *   npm run sol-tracker                # Display current spend
 *   npm run sol-tracker --today        # Today's spend only
 *   npm run sol-tracker --week         # Last 7 days
 */

import { Connection, PublicKey } from '@solana/web3.js';
import { Pool } from 'pg';
import dotenv from 'dotenv';

dotenv.config();

const DATABASE_URL = process.env.DATABASE_URL;
const RPC_URL = process.env.PUBLISHER_RPC_URLS || process.env.RPC_URL || 'https://api.mainnet-beta.solana.com';

if (!DATABASE_URL) {
  console.error('❌ Missing DATABASE_URL environment variable');
  process.exit(1);
}

const pool = new Pool({ connectionString: DATABASE_URL });

interface TransactionRecord {
  signature: string;
  timestamp: Date;
  channel: string | null;
  epoch: number | null;
  fee: number; // lamports
  computeUnitsUsed: number;
  success: boolean;
  transactionType: 'initialize_channel' | 'publish_root' | 'other';
  errorMessage: string | null;
}

interface SpendSummary {
  totalTransactions: number;
  successfulTransactions: number;
  failedTransactions: number;
  totalFeeLamports: number;
  totalFeeSol: number;
  totalComputeUnits: number;
  avgFeePerTx: number;
  avgComputePerTx: number;
  byChannel: Map<string, { count: number; fee: number }>;
  byType: Map<string, { count: number; fee: number }>;
}

async function ensureTransactionTable() {
  await pool.query(`
    CREATE TABLE IF NOT EXISTS sol_transactions (
      id SERIAL PRIMARY KEY,
      signature TEXT UNIQUE NOT NULL,
      timestamp TIMESTAMP NOT NULL,
      channel TEXT,
      epoch BIGINT,
      fee_lamports BIGINT NOT NULL,
      compute_units_used BIGINT NOT NULL,
      success BOOLEAN NOT NULL DEFAULT true,
      transaction_type TEXT NOT NULL,
      error_message TEXT,
      recorded_at TIMESTAMP NOT NULL DEFAULT NOW()
    );

    CREATE INDEX IF NOT EXISTS idx_sol_tx_timestamp ON sol_transactions (timestamp DESC);
    CREATE INDEX IF NOT EXISTS idx_sol_tx_channel ON sol_transactions (channel);
    CREATE INDEX IF NOT EXISTS idx_sol_tx_type ON sol_transactions (transaction_type);
  `);
}

async function recordTransaction(tx: TransactionRecord): Promise<void> {
  await pool.query(`
    INSERT INTO sol_transactions (
      signature,
      timestamp,
      channel,
      epoch,
      fee_lamports,
      compute_units_used,
      success,
      transaction_type,
      error_message
    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
    ON CONFLICT (signature) DO NOTHING
  `, [
    tx.signature,
    tx.timestamp,
    tx.channel,
    tx.epoch,
    tx.fee,
    tx.computeUnitsUsed,
    tx.success,
    tx.transactionType,
    tx.errorMessage
  ]);
}

async function fetchTransactionDetails(signature: string, connection: Connection): Promise<Partial<TransactionRecord>> {
  try {
    const tx = await connection.getTransaction(signature, {
      maxSupportedTransactionVersion: 0
    });

    if (!tx) {
      return { fee: 0, computeUnitsUsed: 0, success: false, errorMessage: 'Transaction not found' };
    }

    return {
      fee: tx.meta?.fee || 0,
      computeUnitsUsed: tx.meta?.computeUnitsConsumed || 0,
      success: tx.meta?.err === null,
      errorMessage: tx.meta?.err ? JSON.stringify(tx.meta.err) : null,
      timestamp: tx.blockTime ? new Date(tx.blockTime * 1000) : new Date()
    };
  } catch (err: any) {
    return { fee: 0, computeUnitsUsed: 0, success: false, errorMessage: err.message };
  }
}

async function backfillRecentTransactions(signerPubkey: string, limit: number = 100): Promise<number> {
  const connection = new Connection(RPC_URL, 'confirmed');
  const pubkey = new PublicKey(signerPubkey);

  console.log(`\n[SOL TRACKER] Backfilling recent transactions for ${signerPubkey}...\n`);

  const signatures = await connection.getSignaturesForAddress(pubkey, { limit });

  let backfilled = 0;
  for (const sig of signatures) {
    const details = await fetchTransactionDetails(sig.signature, connection);

    await recordTransaction({
      signature: sig.signature,
      timestamp: details.timestamp || new Date(sig.blockTime! * 1000),
      channel: null, // Unknown for backfill
      epoch: null,
      fee: details.fee || 0,
      computeUnitsUsed: details.computeUnitsUsed || 0,
      success: details.success ?? false,
      transactionType: 'other',
      errorMessage: details.errorMessage || null
    });

    backfilled++;
  }

  return backfilled;
}

async function getSpendSummary(startTime?: Date, endTime?: Date): Promise<SpendSummary> {
  const whereClause = startTime && endTime
    ? `WHERE timestamp >= $1 AND timestamp < $2`
    : startTime
    ? `WHERE timestamp >= $1`
    : '';

  const params = startTime && endTime ? [startTime, endTime] : startTime ? [startTime] : [];

  // Total stats
  const totalQuery = await pool.query(`
    SELECT
      COUNT(*) as total_transactions,
      COUNT(CASE WHEN success THEN 1 END) as successful_transactions,
      COUNT(CASE WHEN NOT success THEN 1 END) as failed_transactions,
      SUM(fee_lamports) as total_fee_lamports,
      SUM(compute_units_used) as total_compute_units,
      AVG(fee_lamports) as avg_fee_per_tx,
      AVG(compute_units_used) as avg_compute_per_tx
    FROM sol_transactions
    ${whereClause}
  `, params);

  const totals = totalQuery.rows[0];

  // By channel
  const channelQuery = await pool.query(`
    SELECT
      channel,
      COUNT(*) as count,
      SUM(fee_lamports) as total_fee
    FROM sol_transactions
    ${whereClause}
    GROUP BY channel
    ORDER BY total_fee DESC
  `, params);

  const byChannel = new Map(
    channelQuery.rows.map(row => [
      row.channel || 'unknown',
      { count: parseInt(row.count), fee: parseInt(row.total_fee || '0') }
    ])
  );

  // By type
  const typeQuery = await pool.query(`
    SELECT
      transaction_type,
      COUNT(*) as count,
      SUM(fee_lamports) as total_fee
    FROM sol_transactions
    ${whereClause}
    GROUP BY transaction_type
    ORDER BY total_fee DESC
  `, params);

  const byType = new Map(
    typeQuery.rows.map(row => [
      row.transaction_type,
      { count: parseInt(row.count), fee: parseInt(row.total_fee || '0') }
    ])
  );

  return {
    totalTransactions: parseInt(totals.total_transactions),
    successfulTransactions: parseInt(totals.successful_transactions),
    failedTransactions: parseInt(totals.failed_transactions),
    totalFeeLamports: parseInt(totals.total_fee_lamports || '0'),
    totalFeeSol: parseInt(totals.total_fee_lamports || '0') / 1_000_000_000,
    totalComputeUnits: parseInt(totals.total_compute_units || '0'),
    avgFeePerTx: parseFloat(totals.avg_fee_per_tx || '0'),
    avgComputePerTx: parseFloat(totals.avg_compute_per_tx || '0'),
    byChannel,
    byType
  };
}

function formatSol(lamports: number): string {
  return (lamports / 1_000_000_000).toFixed(9) + ' SOL';
}

function formatNumber(n: number): string {
  return n.toLocaleString('en-US');
}

async function displayDashboard(period: 'all' | 'today' | 'week' = 'all') {
  const now = new Date();
  let startTime: Date | undefined;
  let endTime: Date | undefined;

  if (period === 'today') {
    startTime = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    endTime = new Date(startTime.getTime() + 24 * 60 * 60 * 1000);
  } else if (period === 'week') {
    startTime = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);
    endTime = now;
  }

  const summary = await getSpendSummary(startTime, endTime);

  console.log('\n╔══════════════════════════════════════════════════════════════════╗');
  console.log('║               SOL SPEND TRACKER - TRANSACTION COSTS              ║');
  console.log('╚══════════════════════════════════════════════════════════════════╝\n');

  console.log(`📅 Period: ${period === 'all' ? 'All Time' : period === 'today' ? 'Today' : 'Last 7 Days'}\n`);

  console.log('💰 TOTAL SPEND');
  console.log('─────────────────────────────────────────────────────────────────');
  console.log(`  Total Transactions:         ${formatNumber(summary.totalTransactions)}`);
  console.log(`  Successful:                 ${formatNumber(summary.successfulTransactions)}`);
  console.log(`  Failed:                     ${formatNumber(summary.failedTransactions)}`);
  console.log(`  Success Rate:               ${((summary.successfulTransactions / summary.totalTransactions) * 100).toFixed(2)}%`);
  console.log('');
  console.log(`  Total Fees (lamports):      ${formatNumber(summary.totalFeeLamports)}`);
  console.log(`  Total Fees (SOL):           ${formatSol(summary.totalFeeLamports)}`);
  console.log(`  Avg Fee per TX:             ${formatSol(summary.avgFeePerTx)}`);
  console.log('');
  console.log(`  Total Compute Units:        ${formatNumber(summary.totalComputeUnits)}`);
  console.log(`  Avg Compute per TX:         ${formatNumber(Math.round(summary.avgComputePerTx))}`);
  console.log('');

  if (summary.byType.size > 0) {
    console.log('📊 SPEND BY TYPE');
    console.log('─────────────────────────────────────────────────────────────────');
    for (const [type, stats] of summary.byType) {
      console.log(`  ${type.padEnd(25)} │ ${formatNumber(stats.count).padStart(8)} TXs │ ${formatSol(stats.fee)}`);
    }
    console.log('');
  }

  if (summary.byChannel.size > 0 && summary.byChannel.size <= 20) {
    console.log('📺 SPEND BY CHANNEL (Top 20)');
    console.log('─────────────────────────────────────────────────────────────────');
    const sorted = Array.from(summary.byChannel.entries())
      .sort((a, b) => b[1].fee - a[1].fee)
      .slice(0, 20);

    for (const [channel, stats] of sorted) {
      const channelName = (channel || 'unknown').padEnd(25);
      console.log(`  ${channelName} │ ${formatNumber(stats.count).padStart(8)} TXs │ ${formatSol(stats.fee)}`);
    }
    console.log('');
  }

  // Economic Analysis
  console.log('💵 ECONOMIC ANALYSIS');
  console.log('─────────────────────────────────────────────────────────────────');

  // Assuming SOL price (hardcoded for now, could be fetched from API)
  const SOL_PRICE_USD = 150; // Update this with real-time price if needed
  const totalCostUSD = summary.totalFeeSol * SOL_PRICE_USD;

  console.log(`  SOL Price (assumed):        $${SOL_PRICE_USD.toFixed(2)}`);
  console.log(`  Total Cost (USD):           $${totalCostUSD.toFixed(2)}`);
  console.log(`  Cost per TX (USD):          $${(totalCostUSD / summary.totalTransactions).toFixed(6)}`);
  console.log('');

  if (period === 'week') {
    const dailyAvg = summary.totalFeeSol / 7;
    const monthlyProjection = dailyAvg * 30;
    console.log(`  Daily Average:              ${formatSol(dailyAvg * 1_000_000_000)}`);
    console.log(`  Monthly Projection:         ${formatSol(monthlyProjection * 1_000_000_000)} ($${(monthlyProjection * SOL_PRICE_USD).toFixed(2)})`);
    console.log('');
  }

  console.log('╔══════════════════════════════════════════════════════════════════╗');
  console.log('║  Dashboard generated at: ' + new Date().toISOString().padEnd(43) + '║');
  console.log('╚══════════════════════════════════════════════════════════════════╝\n');
}

async function main() {
  try {
    // Ensure table exists
    await ensureTransactionTable();

    // Check for backfill flag
    if (process.argv.includes('--backfill')) {
      const signerPubkey = process.env.PUBLISHER_WALLET || process.env.PAYER_PUBKEY;
      if (!signerPubkey) {
        console.error('❌ Missing PUBLISHER_WALLET or PAYER_PUBKEY for backfill');
        process.exit(1);
      }
      const count = await backfillRecentTransactions(signerPubkey);
      console.log(`✅ Backfilled ${count} transactions\n`);
    }

    // Determine period
    let period: 'all' | 'today' | 'week' = 'all';
    if (process.argv.includes('--today')) period = 'today';
    else if (process.argv.includes('--week')) period = 'week';

    // Display dashboard
    await displayDashboard(period);

    await pool.end();
    process.exit(0);
  } catch (err) {
    console.error('\n❌ SOL Tracker failed:', err);
    await pool.end();
    process.exit(1);
  }
}

main();

export { recordTransaction, fetchTransactionDetails, TransactionRecord };
