#!/usr/bin/env tsx
/**
 * Generate a claim proof for a specific epoch
 */
import { Pool } from 'pg'
import { keccak_256 } from '@noble/hashes/sha3.js'
import { generateProofFromLevels, buildTreeWithLevels, makeClaimLeaf } from '../apps/twzrd-aggregator/src/merkle.js'
import fs from 'fs'

const DATABASE_URL = process.env.DATABASE_URL!
const channel = process.argv[2] || 'jasontheween'
const epoch = Number(process.argv[3]) || 1762362000

async function main() {
  console.log(`\n🎯 Generating Proof for Epoch ${epoch}, Channel ${channel}\n`)

  const pool = new Pool({
    connectionString: DATABASE_URL,
    ssl: { rejectUnauthorized: false }
  })

  // Get sealed participants for this epoch
  const participantsResult = await pool.query(`
    SELECT user_hash
    FROM sealed_participants
    WHERE channel = $1 AND epoch = $2
    ORDER BY user_hash ASC
  `, [channel, epoch])

  if (participantsResult.rows.length === 0) {
    console.error('❌ No participants found for this epoch')
    process.exit(1)
  }

  console.log(`Found ${participantsResult.rows.length} participants`)

  // Get L2 tree from cache
  const treeResult = await pool.query(`
    SELECT root, levels_json, participant_count
    FROM l2_trees
    WHERE channel = $1 AND epoch = $2
  `, [channel, epoch])

  let root: string
  let levels: Buffer[][]
  let participantCount: number

  if (treeResult.rows.length > 0) {
    console.log('✅ Using cached L2 tree')
    root = treeResult.rows[0].root
    levels = treeResult.rows[0].levels_json.map((lvl: any[]) => lvl.map((hex: string) => Buffer.from(hex, 'hex')))
    participantCount = treeResult.rows[0].participant_count
  } else {
    console.log('⚠️  No cached tree, building from scratch...')

    // Get weighted participants
    const weightedResult = await pool.query(`
      SELECT user_hash, weight
      FROM weighted_participants
      WHERE channel = $1 AND epoch = $2
      ORDER BY user_hash ASC
    `, [channel, epoch])

    const weightByHash = new Map(weightedResult.rows.map(r => [r.user_hash, Number(r.weight)]))

    // Build leaves (limited to 1024 to match CHANNEL_MAX_CLAIMS)
    const MAX_CLAIMS = 1024
    const limited = participantsResult.rows.slice(0, MAX_CLAIMS)
    const leaves: Buffer[] = []
    const zeroClaimer = new Uint8Array(32)
    const BASE_PER_WEIGHT = 80
    const DECIMALS = 9

    for (let i = 0; i < limited.length; i++) {
      const userHash = limited[i].user_hash

      // Get username for id
      const usernameResult = await pool.query(`
        SELECT username FROM user_mapping WHERE user_hash = $1
      `, [userHash])
      const username = usernameResult.rows[0]?.username || userHash.slice(0, 16)

      const id = `twitch:${channel}:${username.toLowerCase()}`
      const weight = weightByHash.get(userHash) || 1
      const amount = BigInt(Math.round(weight * BASE_PER_WEIGHT * Math.pow(10, DECIMALS)))

      leaves.push(Buffer.from(makeClaimLeaf({
        claimer: zeroClaimer,
        index: i,
        amount,
        id
      })))
    }

    const tree = buildTreeWithLevels(leaves)
    root = '0x' + Buffer.from(tree.root).toString('hex')
    levels = tree.levels.map(lvl => lvl.map(u8 => Buffer.from(u8)))
    participantCount = limited.length

    console.log(`Built tree with ${participantCount} leaves`)
  }

  console.log(`Root: ${root}`)
  console.log(`Total participants: ${participantCount}`)

  // Pick first participant (index 0, guaranteed < 1024)
  const targetHash = participantsResult.rows[0].user_hash

  // Get username
  const usernameResult = await pool.query(`
    SELECT username FROM user_mapping WHERE user_hash = $1
  `, [targetHash])
  const username = usernameResult.rows[0]?.username || 'unknown'

  console.log(`\nTarget user: ${username} (index 0)`)
  console.log(`User hash: ${targetHash}`)

  // Generate proof
  const proof = generateProofFromLevels(levels, 0)
  const proofHex = proof.map(p => '0x' + Buffer.from(p).toString('hex'))

  // Get amount
  const weightedResult = await pool.query(`
    SELECT weight FROM weighted_participants
    WHERE channel = $1 AND epoch = $2 AND user_hash = $3
  `, [channel, epoch, targetHash])

  const weight = weightedResult.rows[0]?.weight || 1
  const amount = Math.round(weight * 80 * Math.pow(10, 9))

  const proofData = {
    channel,
    epoch,
    root,
    index: 0,
    amount: amount.toString(),
    id: `twitch:${channel}:${username.toLowerCase()}`,
    proof: proofHex,
    participant: username,
    participantCount
  }

  const filename = `/tmp/claim-${username}-${epoch}.json`
  fs.writeFileSync(filename, JSON.stringify(proofData, null, 2))

  console.log(`\n✅ Proof generated and saved to: ${filename}`)
  console.log(`\n🚀 Next step: Submit claim`)
  console.log(`   npx tsx scripts/claims/claim-direct.ts ${filename}`)

  await pool.end()
}

main().catch(console.error)
