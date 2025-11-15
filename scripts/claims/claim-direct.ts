#!/usr/bin/env tsx
/**
 * Direct claim using a local proof JSON file (bypasses aggregator)
 * Usage: npx tsx scripts/claims/claim-direct.ts <proof_file_path>
 */

import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js'
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from '@solana/spl-token'
import { keccak_256 } from '@noble/hashes/sha3.js'
import fs from 'fs'

const PROGRAM_ID = new PublicKey(process.env.PROGRAM_ID || 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop')
const MINT = new PublicKey(process.env.MINT_PUBKEY || 'AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5')
const RPC_URL = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com'

const PROTOCOL_SEED = Buffer.from('protocol')
const CHANNEL_STATE_SEED = Buffer.from('channel_state')

function deriveStreamerKey(channel: string): PublicKey {
  const lower = channel.toLowerCase()
  // MUST use KECCAK-256 with "channel:" prefix to match on-chain program:
  // clean-hackathon/programs/token-2022/src/instructions/channel.rs:25
  // let hash = keccak::hashv(&[b"channel:", &lower]);
  const preimage = Buffer.concat([
    Buffer.from('channel:'),
    Buffer.from(lower, 'utf8')
  ])
  const hash = keccak_256(preimage)
  return new PublicKey(hash)
}

function toHex32(h: string): Buffer {
  const x = h.startsWith('0x') ? h.slice(2) : h
  return Buffer.from(x, 'hex')
}

function buildClaimData(
  channel: string,
  epoch: number,
  index: number,
  amount: number | string | bigint,
  user_hash: string,
  proof: string[],
): Buffer {
  // Discriminator: sha256('global:claim_channel_open')[:8]
  const discriminator = Buffer.from([223, 171, 187, 41, 167, 71, 15, 184])
  const chanBuf = Buffer.from(channel, 'utf8')
  const chanLen = Buffer.alloc(4); chanLen.writeUInt32LE(chanBuf.length, 0)
  const epochBuf = Buffer.alloc(8); epochBuf.writeBigUInt64LE(BigInt(epoch), 0)
  const idxBuf = Buffer.alloc(4); idxBuf.writeUInt32LE(index, 0)
  const amtBuf = Buffer.alloc(8); amtBuf.writeBigUInt64LE(BigInt(amount), 0)
  // user_hash is a hex string (64 chars), passed as-is to on-chain program
  const userHashBuf = Buffer.from(user_hash, 'utf8')
  const userHashLen = Buffer.alloc(4); userHashLen.writeUInt32LE(userHashBuf.length, 0)
  const pLen = Buffer.alloc(4); pLen.writeUInt32LE(proof.length, 0)
  const pBuf = Buffer.concat(proof.map(toHex32))
  return Buffer.concat([discriminator, chanLen, chanBuf, epochBuf, idxBuf, amtBuf, userHashLen, userHashBuf, pLen, pBuf])
}

async function main() {
  const [proofFilePath] = process.argv.slice(2)
  if (!proofFilePath) {
    console.error('Usage: npx tsx scripts/claims/claim-direct.ts <proof_file_path>')
    process.exit(1)
  }

  // Load proof from file
  const proofData = JSON.parse(fs.readFileSync(proofFilePath, 'utf8'))
  const channel = proofData.channel
  const epoch = proofData.epoch
  const root = proofData.root
  const proof = proofData.proof

  // Extract user_hash (hex string, 32 bytes = 64 hex chars)
  const user_hash = proofData.user_hash
  if (!user_hash || user_hash.length !== 64) {
    console.error('Error: proof file must contain user_hash (64-char hex string)')
    process.exit(1)
  }

  const index = proofData.index !== undefined ? proofData.index : 0
  const amount = proofData.amount !== undefined ? proofData.amount : 1024000000000

  const keypairPath = process.env.KEYPAIR_PATH || `${process.env.HOME}/.config/solana/id.json`
  const payer = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(keypairPath, 'utf8'))))

  console.log(`\n🎯 Direct Claim`)
  console.log(`  Program:   ${PROGRAM_ID.toBase58()}`)
  console.log(`  Mint:      ${MINT.toBase58()}`)
  console.log(`  Channel:   ${channel}`)
  console.log(`  Epoch:     ${epoch}`)
  console.log(`  Index:     ${index}`)
  console.log(`  Amount:    ${amount}`)
  console.log(`  Payer:     ${payer.publicKey.toBase58()}`)

  const conn = new Connection(RPC_URL, 'confirmed')

  // Derive accounts
  const [protocolState] = PublicKey.findProgramAddressSync([PROTOCOL_SEED, MINT.toBuffer()], PROGRAM_ID)
  const streamerKey = deriveStreamerKey(channel)
  const [channelState] = PublicKey.findProgramAddressSync([CHANNEL_STATE_SEED, MINT.toBuffer(), streamerKey.toBuffer()], PROGRAM_ID)
  const treasuryAta = getAssociatedTokenAddressSync(MINT, protocolState, true, TOKEN_2022_PROGRAM_ID)
  const claimerAta = getAssociatedTokenAddressSync(MINT, payer.publicKey, false, TOKEN_2022_PROGRAM_ID)

  console.log(`\n📋 Accounts`)
  console.log(`  Protocol State:  ${protocolState.toBase58()}`)
  console.log(`  Channel State:   ${channelState.toBase58()}`)
  console.log(`  Treasury ATA:    ${treasuryAta.toBase58()}`)
  console.log(`  Claimer ATA:     ${claimerAta.toBase58()}`)

  // Build instruction
  const data = buildClaimData(channel, epoch, index, amount, user_hash, proof)
  const keys = [
    { pubkey: payer.publicKey, isSigner: true, isWritable: true },
    { pubkey: protocolState, isSigner: false, isWritable: true },
    { pubkey: channelState, isSigner: false, isWritable: true },
    { pubkey: MINT, isSigner: false, isWritable: false },
    { pubkey: treasuryAta, isSigner: false, isWritable: true },
    { pubkey: claimerAta, isSigner: false, isWritable: true },
    { pubkey: TOKEN_2022_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
  ]

  const { blockhash } = await conn.getLatestBlockhash('confirmed')
  const msg = new TransactionMessage({ payerKey: payer.publicKey, recentBlockhash: blockhash, instructions: [{ programId: PROGRAM_ID, keys, data }] }).compileToV0Message()
  const tx = new VersionedTransaction(msg)
  tx.sign([payer])

  console.log('\n📤 Submitting transaction...')
  try {
    const sig = await conn.sendTransaction(tx, { maxRetries: 3 })
    console.log('✅ Claim successful!')
    console.log('   Signature:', sig)
    const cluster = RPC_URL.includes('devnet') ? '?cluster=devnet' : ''
    console.log('   Explorer:', `https://explorer.solana.com/tx/${sig}${cluster}`)
  } catch (e: any) {
    console.error('\n❌ Claim failed:', e.message)
    if (e.logs) {
      console.log('\n📋 Program Logs:')
      e.logs.forEach((log: string) => console.log('   ', log))
    }
    process.exit(1)
  }
}

main().catch((e) => {
  console.error('❌ Error:', e)
  process.exit(1)
})
