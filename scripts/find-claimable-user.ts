#!/usr/bin/env tsx
import { Pool } from 'pg'

const DATABASE_URL = process.env.DATABASE_URL!

async function main() {
  const pool = new Pool({
    connectionString: DATABASE_URL,
    ssl: { rejectUnauthorized: false }
  })

  console.log('\n🔍 Finding users with claimable index < 1024\n')

  // Get participants from jasontheween, epoch 1761944400
  const result = await pool.query(`
    SELECT user_hash, username
    FROM sealed_participants sp
    LEFT JOIN user_mapping um ON sp.user_hash = um.user_hash
    WHERE sp.channel = 'jasontheween'
      AND sp.epoch = 1761944400
    ORDER BY sp.user_hash ASC
    LIMIT 10
  `)

  console.log('First 10 participants (these will have index 0-9):')
  result.rows.forEach((row, i) => {
    console.log(`  ${i}. ${row.username || 'unknown'} (${row.user_hash.slice(0, 16)}...)`)
  })

  await pool.end()

  console.log('\n✅ Any of these users should have a valid claimable proof.')
  console.log('   Use the first one to test the claim.')
}

main().catch(console.error)
