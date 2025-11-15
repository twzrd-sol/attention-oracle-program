#!/usr/bin/env tsx
import { keccak_256 } from '@noble/hashes/sha3.js'
import { PublicKey } from '@solana/web3.js'

const channel = 'jasontheween'
const preimage = Buffer.concat([
  Buffer.from('channel:'),
  Buffer.from(channel.toLowerCase())
])
const hash = keccak_256(preimage)
const streamerKey = new PublicKey(hash)

console.log('Channel:', channel)
console.log('Streamer Key (KECCAK-256):', streamerKey.toBase58())

// Derive channel_state PDA
const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop')
const MINT = new PublicKey('AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5')
const [channelState, bump] = PublicKey.findProgramAddressSync(
  [Buffer.from('channel_state'), MINT.toBuffer(), streamerKey.toBuffer()],
  PROGRAM_ID
)

console.log('Channel State PDA (expected by program):', channelState.toBase58())
console.log('\nThis should match the address the program is looking for: HkSBNMT6...')
