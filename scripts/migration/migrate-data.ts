#!/usr/bin/env tsx
/**
 * Migrate data from SQLite to PostgreSQL
 * Preserves all data and relationships
 */
import Database from 'better-sqlite3'
import pg from 'pg'

const { Pool } = pg

const SQLITE_PATH = './apps/twzrd-aggregator/data/twzrd.db'
const PG_CONNECTION = 'postgresql://twzrd:twzrd_password_2025@localhost:5432/twzrd'

const TABLES = [
  'channel_participation',
  'user_signals',
  'sealed_epochs',
  'sealed_participants',
  'user_mapping',
  'l2_tree_cache',
  'attention_index',
]

async function migrateTable(
  sqlite: Database.Database,
  pool: pg.Pool,
  tableName: string
) {
  console.log(`\nMigrating ${tableName}...`)

  // Get row count
  const countRow = sqlite.prepare(`SELECT COUNT(*) as count FROM ${tableName}`).get() as { count: number }
  const rowCount = countRow.count

  if (rowCount === 0) {
    console.log(`  ✓ ${tableName}: 0 rows (skipped)`)
    return
  }

  // Get all rows
  const rows = sqlite.prepare(`SELECT * FROM ${tableName}`).all()

  if (rows.length === 0) {
    console.log(`  ✓ ${tableName}: 0 rows`)
    return
  }

  // Get column names from first row
  const columns = Object.keys(rows[0])
  const placeholders = columns.map((_, i) => `$${i + 1}`).join(', ')
  const columnList = columns.join(', ')

  const insertQuery = `
    INSERT INTO ${tableName} (${columnList})
    VALUES (${placeholders})
    ON CONFLICT DO NOTHING
  `

  // Insert in batches
  const BATCH_SIZE = 1000
  let inserted = 0

  for (let i = 0; i < rows.length; i += BATCH_SIZE) {
    const batch = rows.slice(i, i + BATCH_SIZE)
    const client = await pool.connect()

    try {
      await client.query('BEGIN')

      for (const row of batch) {
        const values = columns.map(col => row[col])
        await client.query(insertQuery, values)
      }

      await client.query('COMMIT')
      inserted += batch.length
      console.log(`  Progress: ${inserted}/${rows.length}`)
    } catch (err: any) {
      await client.query('ROLLBACK')
      console.error(`  ✖ Error in batch ${i}-${i + batch.length}:`, err.message)
    } finally {
      client.release()
    }
  }

  console.log(`  ✓ ${tableName}: ${inserted}/${rows.length} rows migrated`)
}

async function migrate() {
  console.log('Starting migration from SQLite to PostgreSQL...')
  console.log(`SQLite: ${SQLITE_PATH}`)
  console.log(`PostgreSQL: ${PG_CONNECTION}`)

  const sqlite = new Database(SQLITE_PATH, { readonly: true })
  const pool = new Pool({ connectionString: PG_CONNECTION })

  try {
    for (const table of TABLES) {
      await migrateTable(sqlite, pool, table)
    }

    console.log('\n✅ Migration complete!')

    // Verify counts
    console.log('\nVerifying row counts...')
    for (const table of TABLES) {
      const sqliteCount = (sqlite.prepare(`SELECT COUNT(*) as count FROM ${table}`).get() as { count: number }).count
      const pgResult = await pool.query(`SELECT COUNT(*) FROM ${table}`)
      const pgCount = parseInt(pgResult.rows[0].count)

      const match = sqliteCount === pgCount ? '✓' : '✗'
      console.log(`  ${match} ${table}: SQLite=${sqliteCount}, PostgreSQL=${pgCount}`)
    }
  } catch (err: any) {
    console.error('Migration failed:', err.message)
    process.exit(1)
  } finally {
    sqlite.close()
    await pool.end()
  }
}

migrate().catch(console.error)
