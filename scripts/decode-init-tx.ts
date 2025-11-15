#!/usr/bin/env tsx
import { Connection, PublicKey } from '@solana/web3.js'

const RPC_URL = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com'
const TX_SIG = process.argv[2]

async function main() {
  if (!TX_SIG) {
    console.error('Usage: npx tsx scripts/decode-init-tx.ts <signature>')
    process.exit(1)
  }

  const connection = new Connection(RPC_URL, 'confirmed')

  console.log(`\n🔍 Decoding Transaction: ${TX_SIG}\n`)

  const tx = await connection.getTransaction(TX_SIG, {
    maxSupportedTransactionVersion: 0
  })

  if (!tx) {
    console.error('Transaction not found')
    process.exit(1)
  }

  // Extract instruction data
  const ix = tx.transaction.message.compiledInstructions[0]
  const data = Buffer.from(ix.data)

  console.log('Instruction Data (hex):', data.toString('hex'))
  console.log('\nParsing SetMerkleRootRing instruction:')

  // Discriminator is first 8 bytes
  const discriminator = data.slice(0, 8).toString('hex')
  console.log('  Discriminator:', discriminator)

  // Channel length (u32)
  const channelLen = data.readUInt32LE(8)
  console.log('  Channel length:', channelLen)

  // Channel name
  const channel = data.slice(12, 12 + channelLen).toString('utf8')
  console.log('  Channel:', channel)

  // Epoch (u64)
  const epochOffset = 12 + channelLen
  const epoch = data.readBigUInt64LE(epochOffset)
  console.log('  Epoch:', epoch.toString())

  // Root (32 bytes)
  const rootOffset = epochOffset + 8
  const root = '0x' + data.slice(rootOffset, rootOffset + 32).toString('hex')
  console.log('  Root:', root)

  // Claim count (u16)
  const claimCountOffset = rootOffset + 32
  const claimCount = data.readUInt16LE(claimCountOffset)
  console.log('  Claim Count:', claimCount)

  console.log('\n✅ This epoch and root are now in the ring buffer')
}

main().catch(console.error)
