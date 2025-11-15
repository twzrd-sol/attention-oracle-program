#!/usr/bin/env tsx
/**
 * Stage 1: Dual-read verification
 * Compare SQLite vs PostgreSQL responses for critical queries
 * Run this AFTER migration completes to verify data integrity
 */

import { TwzrdDB } from '../../apps/twzrd-aggregator/src/db.js'
import { TwzrdDBPostgres } from '../../apps/twzrd-aggregator/src/db-pg.js'

const SQLITE_PATH = './apps/twzrd-aggregator/data/twzrd.db'
const PG_CONN = process.env.DATABASE_URL || 'postgresql://twzrd:twzrd_password_2025@localhost:5432/twzrd'

interface VerificationResult {
  test: string
  sqlite: any
  postgres: any
  match: boolean
  error?: string
}

async function verifyDualRead() {
  const sqliteDB = new TwzrdDB(SQLITE_PATH)
  const pgDB = new TwzrdDBPostgres(PG_CONN)

  const results: VerificationResult[] = []

  console.log('Starting dual-read verification...\n')

  // Test 1: Row counts
  try {
    const sqliteCount = sqliteDB.db.prepare('SELECT COUNT(*) as count FROM channel_participation').get() as { count: number }
    const pgResult = await pgDB['pool'].query('SELECT COUNT(*) FROM channel_participation')
    const pgCount = parseInt(pgResult.rows[0].count)

    const match = sqliteCount.count === pgCount
    results.push({
      test: 'channel_participation row count',
      sqlite: sqliteCount.count,
      postgres: pgCount,
      match,
    })

    console.log(`✓ channel_participation: SQLite=${sqliteCount.count}, PG=${pgCount} [${match ? 'MATCH' : 'MISMATCH'}]`)
  } catch (err: any) {
    results.push({
      test: 'channel_participation row count',
      sqlite: 'N/A',
      postgres: 'N/A',
      match: false,
      error: err.message,
    })
    console.error(`✗ channel_participation count failed: ${err.message}`)
  }

  // Test 2: Sealed participants for a known epoch
  try {
    const testEpoch = 1761789600
    const testChannel = 'jasontheween'

    const sqliteSealed = sqliteDB.getSealedParticipants(testEpoch, testChannel)
    const pgSealed = await pgDB.getSealedParticipants(testEpoch, testChannel)

    const match =
      sqliteSealed?.length === pgSealed?.length &&
      sqliteSealed?.every((hash, idx) => hash === pgSealed[idx])

    results.push({
      test: `sealed_participants (${testChannel}, epoch=${testEpoch})`,
      sqlite: sqliteSealed?.length || 0,
      postgres: pgSealed?.length || 0,
      match,
    })

    console.log(`✓ sealed_participants: SQLite=${sqliteSealed?.length}, PG=${pgSealed?.length} [${match ? 'MATCH' : 'MISMATCH'}]`)
  } catch (err: any) {
    results.push({
      test: 'sealed_participants',
      sqlite: 'N/A',
      postgres: 'N/A',
      match: false,
      error: err.message,
    })
    console.error(`✗ sealed_participants failed: ${err.message}`)
  }

  // Test 3: L2 cache for known tree
  try {
    const testEpoch = 1761789600
    const testChannel = 'jasontheween'

    const sqliteCache = sqliteDB.getCachedL2Tree(testEpoch, testChannel)
    const pgCache = await pgDB.getCachedL2Tree(testEpoch, testChannel)

    const match = sqliteCache?.root === pgCache?.root && sqliteCache?.participantCount === pgCache?.participantCount

    results.push({
      test: `l2_tree_cache (${testChannel}, epoch=${testEpoch})`,
      sqlite: { root: sqliteCache?.root?.slice(0, 16), count: sqliteCache?.participantCount },
      postgres: { root: pgCache?.root?.slice(0, 16), count: pgCache?.participantCount },
      match,
    })

    console.log(`✓ l2_tree_cache: SQLite root=${sqliteCache?.root.slice(0, 16)}..., PG root=${pgCache?.root.slice(0, 16)}... [${match ? 'MATCH' : 'MISMATCH'}]`)
  } catch (err: any) {
    results.push({
      test: 'l2_tree_cache',
      sqlite: 'N/A',
      postgres: 'N/A',
      match: false,
      error: err.message,
    })
    console.error(`✗ l2_tree_cache failed: ${err.message}`)
  }

  // Test 4: User mapping
  try {
    const sqliteUserCount = sqliteDB.db.prepare('SELECT COUNT(*) as count FROM user_mapping').get() as { count: number }
    const pgUserResult = await pgDB['pool'].query('SELECT COUNT(*) FROM user_mapping')
    const pgUserCount = parseInt(pgUserResult.rows[0].count)

    const match = sqliteUserCount.count === pgUserCount
    results.push({
      test: 'user_mapping row count',
      sqlite: sqliteUserCount.count,
      postgres: pgUserCount,
      match,
    })

    console.log(`✓ user_mapping: SQLite=${sqliteUserCount.count}, PG=${pgUserCount} [${match ? 'MATCH' : 'MISMATCH'}]`)
  } catch (err: any) {
    results.push({
      test: 'user_mapping row count',
      sqlite: 'N/A',
      postgres: 'N/A',
      match: false,
      error: err.message,
    })
    console.error(`✗ user_mapping count failed: ${err.message}`)
  }

  await pgDB.close()

  // Summary
  console.log('\n' + '='.repeat(60))
  const passed = results.filter(r => r.match).length
  const failed = results.filter(r => !r.match).length
  console.log(`Verification complete: ${passed} passed, ${failed} failed`)
  console.log('='.repeat(60))

  if (failed > 0) {
    console.error('\n⚠️  Data mismatch detected. Do NOT cutover to PostgreSQL yet.')
    process.exit(1)
  } else {
    console.log('\n✅ All verifications passed. Ready for Stage 2 (writer cutover).')
  }
}

verifyDualRead().catch(err => {
  console.error('Verification script failed:', err.message)
  process.exit(1)
})
