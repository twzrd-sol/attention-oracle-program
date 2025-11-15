#!/usr/bin/env tsx
import { Connection, PublicKey } from '@solana/web3.js'

const RPC_URL = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com'
const CHANNEL_STATE = 'HkSBNMT6FyZCJnYeTgfSgPBiqAAG79UGULhUoWH2Zxei'

async function main() {
  const connection = new Connection(RPC_URL, 'confirmed')
  const pubkey = new PublicKey(CHANNEL_STATE)

  console.log('\n🔍 Reading ChannelState Account')
  console.log(`Address: ${CHANNEL_STATE}\n`)

  const accountInfo = await connection.getAccountInfo(pubkey)
  if (!accountInfo) {
    console.error('❌ Account not found')
    process.exit(1)
  }

  const data = accountInfo.data

  console.log(`Account size: ${data.length} bytes`)
  console.log(`Owner: ${accountInfo.owner.toBase58()}\n`)

  // Parse ChannelState structure
  // Discriminator: 8 bytes
  // version: u8 (1 byte)
  // mint: Pubkey (32 bytes)
  // streamer: Pubkey (32 bytes)
  // latest_epoch: u64 (8 bytes)
  // slots: [ChannelSlot; 10]

  const discriminator = data.slice(0, 8).toString('hex')
  console.log(`Discriminator: ${discriminator}`)

  const version = data.readUInt8(8)
  console.log(`Version: ${version}`)

  const bump = data.readUInt8(9)
  console.log(`Bump: ${bump}`)

  const mint = new PublicKey(data.slice(10, 42))
  console.log(`Mint: ${mint.toBase58()}`)

  const streamer = new PublicKey(data.slice(42, 74))
  console.log(`Streamer: ${streamer.toBase58()}`)

  const latestEpoch = data.readBigUInt64LE(74)
  console.log(`Latest Epoch: ${latestEpoch}`)

  console.log(`\n📦 Ring Buffer Slots (10 total):\n`)

  // Each ChannelSlot is: epoch (u64) + root ([u8;32]) + claimed_bitmap ([u8;128])
  const SLOT_SIZE = 8 + 32 + 128
  const SLOTS_OFFSET = 82

  for (let i = 0; i < 10; i++) {
    const slotOffset = SLOTS_OFFSET + (i * SLOT_SIZE)
    const epoch = data.readBigUInt64LE(slotOffset)
    const root = '0x' + data.slice(slotOffset + 8, slotOffset + 40).toString('hex')

    if (epoch === 0n) {
      console.log(`  Slot ${i}: [EMPTY]`)
    } else {
      console.log(`  Slot ${i}: Epoch ${epoch}`)
      console.log(`           Root: ${root}`)

      // Check what slot this epoch SHOULD be in
      const expectedSlot = Number(epoch) % 10
      if (expectedSlot !== i) {
        console.log(`           ⚠️  WARNING: Epoch ${epoch} should be in slot ${expectedSlot}, not ${i}!`)
      }
    }
  }

  console.log(`\n✅ Done`)
}

main().catch(console.error)
