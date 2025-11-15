#!/usr/bin/env tsx
/**
 * Create a synthetic test epoch with our wallet for end-to-end testing
 */

import { Keypair, PublicKey } from '@solana/web3.js'
import { keccak_256 } from '@noble/hashes/sha3.js'
import fs from 'fs'
import pg from 'pg'

const { Client } = pg

const TEST_EPOCH = 9999999999
const TEST_CHANNEL = 'test'
const TEST_AMOUNT = 1024000000000 // 1024 MILO

// Load our keypair
const keypairPath = process.env.KEYPAIR_PATH || `${process.env.HOME}/.config/solana/id.json`
const keypair = Keypair.fromSecretKey(
  new Uint8Array(JSON.parse(fs.readFileSync(keypairPath, 'utf8')))
)

const claimer = keypair.publicKey
const claimerHex = Buffer.from(claimer.toBytes()).toString('hex')

console.log('🧪 Creating Test Epoch')
console.log('  Epoch:', TEST_EPOCH)
console.log('  Channel:', TEST_CHANNEL)
console.log('  Claimer:', claimer.toBase58())
console.log('  Amount:', TEST_AMOUNT)

// Compute leaf: keccak(claimer, index, amount, id)
const index = 0
const indexBytes = Buffer.alloc(4)
indexBytes.writeUInt32LE(index, 0)

const amountBytes = Buffer.alloc(8)
amountBytes.writeBigUInt64LE(BigInt(TEST_AMOUNT), 0)

const id = `twitch:${TEST_CHANNEL}:${claimerHex}`
const idBytes = Buffer.from(id, 'utf8')

const leaf = Buffer.from(keccak_256(Buffer.concat([
  claimer.toBytes(),
  indexBytes,
  amountBytes,
  idBytes
])))

console.log('\n📋 Merkle Tree (single leaf)')
console.log('  Leaf:', leaf.toString('hex'))
console.log('  Root:', leaf.toString('hex'), '(same as leaf for single participant)')

const root = leaf.toString('hex')
const proof: string[] = [] // Empty proof for single-leaf tree

async function main() {
  const dbUrl = process.env.DATABASE_URL
  if (!dbUrl) {
    throw new Error('DATABASE_URL not set')
  }

  const client = new Client({
    connectionString: dbUrl,
    ssl: { rejectUnauthorized: false }
  })
  await client.connect()

  try {
    console.log('\n💾 Inserting into database...')

    // Insert participant
    await client.query(`
      INSERT INTO sealed_participants (epoch, channel, idx, user_hash, username)
      VALUES ($1, $2, $3, $4, $5)
      ON CONFLICT (epoch, channel, idx) DO UPDATE SET user_hash = EXCLUDED.user_hash
    `, [TEST_EPOCH, TEST_CHANNEL, index, claimerHex, 'test_user'])
    console.log('  ✅ Participant inserted')

    // Seal epoch
    await client.query(`
      INSERT INTO sealed_epochs (epoch, channel, root, sealed_at, published)
      VALUES ($1, $2, $3, $4, $5)
      ON CONFLICT (epoch, channel) DO UPDATE SET root = EXCLUDED.root
    `, [TEST_EPOCH, TEST_CHANNEL, root, Math.floor(Date.now() / 1000), 0])
    console.log('  ✅ Epoch sealed')

    // Cache L2 tree
    const levelsJson = JSON.stringify([[leaf.toString('hex')]])
    await client.query(`
      INSERT INTO l2_tree_cache (epoch, channel, root, levels_json, participant_count, built_at)
      VALUES ($1, $2, $3, $4, $5, $6)
      ON CONFLICT (epoch, channel) DO UPDATE SET
        root = EXCLUDED.root,
        levels_json = EXCLUDED.levels_json,
        participant_count = EXCLUDED.participant_count,
        built_at = EXCLUDED.built_at
    `, [TEST_EPOCH, TEST_CHANNEL, root, levelsJson, 1, Math.floor(Date.now() / 1000)])
    console.log('  ✅ L2 tree cached')

    console.log('\n✅ Test epoch created!')
    console.log('\n📝 Proof file: /tmp/test-epoch-proof.json')

    // Write proof to file
    const proofData = {
      channel: TEST_CHANNEL,
      epoch: TEST_EPOCH,
      index,
      amount: TEST_AMOUNT,
      id,
      proof,
      root: '0x' + root,
      participantCount: 1,
      cached: true
    }

    fs.writeFileSync('/tmp/test-epoch-proof.json', JSON.stringify(proofData, null, 2))

    console.log('\n🎯 Next steps:')
    console.log('  1. Verify aggregator can serve this epoch:')
    console.log(`     curl "http://127.0.0.1:8080/claim-root?channel=${TEST_CHANNEL}&epoch=${TEST_EPOCH}"`)
    console.log('')
    console.log('  2. Publish root to on-chain:')
    console.log(`     npx tsx scripts/publish-channel-root.ts ${TEST_CHANNEL} ${TEST_EPOCH} 0x${root} mainnet`)
    console.log('')
    console.log('  3. Claim tokens:')
    console.log(`     npx tsx scripts/claims/claim-direct.ts /tmp/test-epoch-proof.json`)

  } finally {
    await client.end()
  }
}

main().catch(console.error)
