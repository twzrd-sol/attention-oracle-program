#!/usr/bin/env node
/**
 * Recovery Dashboard - Protocol Resilience Monitoring
 *
 * Research Purpose: Measure natural recovery rate of orphaned participation records
 *
 * Key Metrics:
 * 1. Recovery Rate: % of 201,184 orphaned user_hash values that have been recovered
 * 2. New Growth: New unique users added to the system post-fix
 * 3. Time-Series Analysis: Track recovery velocity over time
 *
 * Research Questions:
 * - What is the natural decay rate of user participation?
 * - What percentage of users return within 7/30/90 days?
 * - What is the economic cost of orphaned records?
 *
 * Usage:
 *   npm run recovery-dashboard           # One-time snapshot
 *   npm run recovery-dashboard --record  # Save snapshot to metrics table
 */

import { Pool } from 'pg';
import dotenv from 'dotenv';

dotenv.config();

const DATABASE_URL = process.env.DATABASE_URL;

if (!DATABASE_URL) {
  console.error('❌ Missing DATABASE_URL environment variable');
  process.exit(1);
}

const pool = new Pool({
  connectionString: DATABASE_URL,
});

interface RecoveryMetrics {
  timestamp: Date;
  totalOrphanedRecords: number;
  totalUniqueOrphanedHashes: number;
  recoveredHashes: number;
  recoveryRate: number;
  totalMappedUsers: number;
  newUsersToday: number;
  newUsersWeek: number;
  totalSealedParticipants: number;
  participantsWithUsername: number;
  participantsWithoutUsername: number;
}

interface ChannelMetrics {
  channel: string;
  totalParticipants: number;
  orphanedParticipants: number;
  recoveryRate: number;
}

interface EpochMetrics {
  epoch: number;
  epochDate: Date;
  totalParticipants: number;
  mappedParticipants: number;
  mappingRate: number;
}

async function ensureMetricsTable() {
  await pool.query(`
    CREATE TABLE IF NOT EXISTS recovery_metrics (
      id SERIAL PRIMARY KEY,
      recorded_at TIMESTAMP NOT NULL DEFAULT NOW(),
      total_orphaned_records BIGINT NOT NULL,
      total_unique_orphaned_hashes BIGINT NOT NULL,
      recovered_hashes BIGINT NOT NULL,
      recovery_rate DECIMAL(10, 6) NOT NULL,
      total_mapped_users BIGINT NOT NULL,
      new_users_today BIGINT NOT NULL,
      new_users_week BIGINT NOT NULL,
      total_sealed_participants BIGINT NOT NULL,
      participants_with_username BIGINT NOT NULL,
      participants_without_username BIGINT NOT NULL
    );
  `);
}

async function getRecoveryMetrics(): Promise<RecoveryMetrics> {
  const now = new Date();
  const oneDayAgo = new Date(now.getTime() - 24 * 60 * 60 * 1000);
  const oneWeekAgo = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);

  // Total sealed participants
  const totalResult = await pool.query(`
    SELECT COUNT(*) as count FROM sealed_participants
  `);
  const totalSealedParticipants = parseInt(totalResult.rows[0].count);

  // Participants with username
  const withUsernameResult = await pool.query(`
    SELECT COUNT(*) as count FROM sealed_participants WHERE username IS NOT NULL
  `);
  const participantsWithUsername = parseInt(withUsernameResult.rows[0].count);

  // Participants without username (orphaned)
  const withoutUsernameResult = await pool.query(`
    SELECT COUNT(*) as count FROM sealed_participants WHERE username IS NULL
  `);
  const participantsWithoutUsername = parseInt(withoutUsernameResult.rows[0].count);

  // Unique orphaned hashes
  const orphanedHashesResult = await pool.query(`
    SELECT COUNT(DISTINCT user_hash) as count
    FROM sealed_participants
    WHERE username IS NULL
  `);
  const totalUniqueOrphanedHashes = parseInt(orphanedHashesResult.rows[0].count);

  // Recovered hashes (previously orphaned, now mapped)
  // This requires checking if a user_hash exists in user_mapping but has NULL username in at least one sealed_participants record
  const recoveredResult = await pool.query(`
    SELECT COUNT(DISTINCT sp.user_hash) as count
    FROM sealed_participants sp
    INNER JOIN user_mapping um ON sp.user_hash = um.user_hash
    WHERE EXISTS (
      SELECT 1 FROM sealed_participants sp2
      WHERE sp2.user_hash = sp.user_hash
      AND sp2.username IS NULL
    )
  `);
  const recoveredHashes = parseInt(recoveredResult.rows[0].count);

  // Total mapped users
  const totalMappedResult = await pool.query(`
    SELECT COUNT(*) as count FROM user_mapping
  `);
  const totalMappedUsers = parseInt(totalMappedResult.rows[0].count);

  // New users today (first_seen in last 24 hours)
  const newTodayResult = await pool.query(`
    SELECT COUNT(*) as count
    FROM user_mapping
    WHERE first_seen >= $1
  `, [Math.floor(oneDayAgo.getTime() / 1000)]);
  const newUsersToday = parseInt(newTodayResult.rows[0].count);

  // New users this week
  const newWeekResult = await pool.query(`
    SELECT COUNT(*) as count
    FROM user_mapping
    WHERE first_seen >= $1
  `, [Math.floor(oneWeekAgo.getTime() / 1000)]);
  const newUsersWeek = parseInt(newWeekResult.rows[0].count);

  const recoveryRate = totalUniqueOrphanedHashes > 0
    ? (recoveredHashes / totalUniqueOrphanedHashes) * 100
    : 0;

  return {
    timestamp: now,
    totalOrphanedRecords: participantsWithoutUsername,
    totalUniqueOrphanedHashes,
    recoveredHashes,
    recoveryRate,
    totalMappedUsers,
    newUsersToday,
    newUsersWeek,
    totalSealedParticipants,
    participantsWithUsername,
    participantsWithoutUsername,
  };
}

async function getChannelMetrics(): Promise<ChannelMetrics[]> {
  const result = await pool.query(`
    SELECT
      channel,
      COUNT(*) as total_participants,
      COUNT(CASE WHEN username IS NULL THEN 1 END) as orphaned_participants,
      (COUNT(CASE WHEN username IS NOT NULL THEN 1 END) * 100.0 / COUNT(*)) as recovery_rate
    FROM sealed_participants
    GROUP BY channel
    ORDER BY total_participants DESC
    LIMIT 20
  `);

  return result.rows.map(row => ({
    channel: row.channel,
    totalParticipants: parseInt(row.total_participants),
    orphanedParticipants: parseInt(row.orphaned_participants),
    recoveryRate: parseFloat(row.recovery_rate) || 0,
  }));
}

async function getEpochMetrics(): Promise<EpochMetrics[]> {
  const result = await pool.query(`
    SELECT
      epoch,
      COUNT(*) as total_participants,
      COUNT(CASE WHEN username IS NOT NULL THEN 1 END) as mapped_participants,
      (COUNT(CASE WHEN username IS NOT NULL THEN 1 END) * 100.0 / COUNT(*)) as mapping_rate
    FROM sealed_participants
    GROUP BY epoch
    ORDER BY epoch DESC
    LIMIT 20
  `);

  return result.rows.map(row => ({
    epoch: parseInt(row.epoch),
    epochDate: new Date(parseInt(row.epoch) * 1000),
    totalParticipants: parseInt(row.total_participants),
    mappedParticipants: parseInt(row.mapped_participants),
    mappingRate: parseFloat(row.mapping_rate) || 0,
  }));
}

async function saveMetricsSnapshot(metrics: RecoveryMetrics) {
  await pool.query(`
    INSERT INTO recovery_metrics (
      recorded_at,
      total_orphaned_records,
      total_unique_orphaned_hashes,
      recovered_hashes,
      recovery_rate,
      total_mapped_users,
      new_users_today,
      new_users_week,
      total_sealed_participants,
      participants_with_username,
      participants_without_username
    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
  `, [
    metrics.timestamp,
    metrics.totalOrphanedRecords,
    metrics.totalUniqueOrphanedHashes,
    metrics.recoveredHashes,
    metrics.recoveryRate,
    metrics.totalMappedUsers,
    metrics.newUsersToday,
    metrics.newUsersWeek,
    metrics.totalSealedParticipants,
    metrics.participantsWithUsername,
    metrics.participantsWithoutUsername,
  ]);
}

async function getHistoricalTrend(): Promise<any[]> {
  const result = await pool.query(`
    SELECT
      recorded_at,
      recovery_rate,
      total_mapped_users,
      new_users_today
    FROM recovery_metrics
    ORDER BY recorded_at DESC
    LIMIT 30
  `);

  return result.rows;
}

function formatNumber(num: number): string {
  return num.toLocaleString();
}

function formatPercent(num: number): string {
  return `${num.toFixed(2)}%`;
}

function formatDate(date: Date): string {
  return date.toISOString().split('T')[0];
}

async function displayDashboard() {
  console.log('\n╔════════════════════════════════════════════════════════════════╗');
  console.log('║      MILO PROTOCOL - RECOVERY DASHBOARD v1.0                   ║');
  console.log('║      Data Loss Incident: Oct 26-30, 2025                       ║');
  console.log('╚════════════════════════════════════════════════════════════════╝\n');

  // Main metrics
  console.log('📊 CORE METRICS\n');
  const metrics = await getRecoveryMetrics();

  console.log('┌─ Orphaned Records (Historical Data Loss) ─────────────────────┐');
  console.log(`│  Total Sealed Participants:        ${formatNumber(metrics.totalSealedParticipants).padStart(12)} │`);
  console.log(`│  With Username (Claimable):        ${formatNumber(metrics.participantsWithUsername).padStart(12)} │`);
  console.log(`│  Without Username (Orphaned):      ${formatNumber(metrics.participantsWithoutUsername).padStart(12)} │`);
  console.log(`│  Unique Orphaned user_hash values: ${formatNumber(metrics.totalUniqueOrphanedHashes).padStart(12)} │`);
  console.log('└────────────────────────────────────────────────────────────────┘\n');

  console.log('┌─ Recovery Progress ────────────────────────────────────────────┐');
  console.log(`│  Recovered user_hash values:       ${formatNumber(metrics.recoveredHashes).padStart(12)} │`);
  console.log(`│  Recovery Rate:                    ${formatPercent(metrics.recoveryRate).padStart(12)} │`);
  console.log('└────────────────────────────────────────────────────────────────┘\n');

  console.log('┌─ User Growth (Post-Fix) ───────────────────────────────────────┐');
  console.log(`│  Total Mapped Users:               ${formatNumber(metrics.totalMappedUsers).padStart(12)} │`);
  console.log(`│  New Users (Last 24h):             ${formatNumber(metrics.newUsersToday).padStart(12)} │`);
  console.log(`│  New Users (Last 7d):              ${formatNumber(metrics.newUsersWeek).padStart(12)} │`);
  console.log('└────────────────────────────────────────────────────────────────┘\n');

  // Channel breakdown
  console.log('📺 RECOVERY BY CHANNEL\n');
  const channels = await getChannelMetrics();
  console.log('Channel              │ Total Participants │ Orphaned │ Recovery Rate');
  console.log('─────────────────────┼────────────────────┼──────────┼──────────────');
  channels.forEach(ch => {
    const channelName = ch.channel.padEnd(20);
    const total = formatNumber(ch.totalParticipants).padStart(18);
    const orphaned = formatNumber(ch.orphanedParticipants).padStart(8);
    const rate = formatPercent(100 - ch.recoveryRate).padStart(12);
    console.log(`${channelName} │ ${total} │ ${orphaned} │ ${rate}`);
  });
  console.log('');

  // Recent epochs
  console.log('⏰ RECENT EPOCHS (Mapping Rate)\n');
  const epochs = await getEpochMetrics();
  console.log('Date       │ Epoch      │ Participants │ Mapped │ Mapping Rate');
  console.log('───────────┼────────────┼──────────────┼────────┼──────────────');
  epochs.forEach(ep => {
    const date = formatDate(ep.epochDate);
    const epoch = ep.epoch.toString().padStart(10);
    const total = formatNumber(ep.totalParticipants).padStart(12);
    const mapped = formatNumber(ep.mappedParticipants).padStart(6);
    const rate = formatPercent(ep.mappingRate).padStart(12);
    console.log(`${date} │ ${epoch} │ ${total} │ ${mapped} │ ${rate}`);
  });
  console.log('');

  // Historical trend
  const trend = await getHistoricalTrend();
  if (trend.length > 0) {
    console.log('📈 HISTORICAL TREND (Last 30 Snapshots)\n');
    console.log('Date                │ Recovery Rate │ Total Users │ New Today');
    console.log('────────────────────┼───────────────┼─────────────┼──────────');
    trend.forEach(t => {
      const date = new Date(t.recorded_at).toISOString().substring(0, 19).replace('T', ' ');
      const rate = formatPercent(parseFloat(t.recovery_rate)).padStart(13);
      const total = formatNumber(parseInt(t.total_mapped_users)).padStart(11);
      const newToday = formatNumber(parseInt(t.new_users_today)).padStart(8);
      console.log(`${date} │ ${rate} │ ${total} │ ${newToday}`);
    });
    console.log('');
  }

  // Economic analysis
  console.log('💰 ECONOMIC IMPACT ANALYSIS\n');
  const orphanedValue = metrics.totalOrphanedRecords;
  const recoveredValue = metrics.participantsWithUsername;
  const lostValue = orphanedValue;

  console.log('┌─ Participation Value ──────────────────────────────────────────┐');
  console.log(`│  Total Participation Records:      ${formatNumber(metrics.totalSealedParticipants).padStart(12)} │`);
  console.log(`│  Claimable (Recovered + New):      ${formatNumber(recoveredValue).padStart(12)} │`);
  console.log(`│  Unclaimable (Orphaned):           ${formatNumber(lostValue).padStart(12)} │`);
  console.log(`│  Claimable Rate:                   ${formatPercent((recoveredValue / metrics.totalSealedParticipants) * 100).padStart(12)} │`);
  console.log('└────────────────────────────────────────────────────────────────┘\n');

  console.log('📝 RESEARCH NOTES\n');
  console.log('  • Fix deployed: username mapping now working for all new participation');
  console.log('  • Recovery strategy: Passive (natural re-engagement)');
  console.log('  • Expected recovery timeline: 30-90 days for 70-85% of active users');
  console.log('  • Unrecoverable: Users who stopped watching after Oct 30\n');

  console.log('🔬 NEXT STEPS\n');
  console.log('  1. Run this dashboard daily to track recovery velocity');
  console.log('  2. Analyze user retention patterns by channel');
  console.log('  3. Calculate economic cost per orphaned record');
  console.log('  4. Design protocol v2 with delayed hashing for resilience\n');

  return metrics;
}

async function main() {
  try {
    const shouldRecord = process.argv.includes('--record');

    // Ensure metrics table exists
    await ensureMetricsTable();

    // Display dashboard
    const metrics = await displayDashboard();

    // Save snapshot if requested
    if (shouldRecord) {
      await saveMetricsSnapshot(metrics);
      console.log('✅ Metrics snapshot saved to recovery_metrics table\n');
    } else {
      console.log('💡 Tip: Run with --record to save snapshot to database\n');
    }

    await pool.end();
    process.exit(0);
  } catch (err) {
    console.error('\n❌ Dashboard failed:', err);
    await pool.end();
    process.exit(1);
  }
}

main();
