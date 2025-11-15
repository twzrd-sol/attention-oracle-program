#!/usr/bin/env tsx
/**
 * Update Protocol State to Authorize New Publisher
 * This script updates the protocol_state.publisher field to authorize oracle-authority.json
 */

import { Connection, Keypair, PublicKey, SystemProgram, Transaction, TransactionInstruction } from '@solana/web3.js'
import crypto from 'node:crypto'
import fs from 'node:fs'

const PROTOCOL_SEED = Buffer.from('protocol')

async function main() {
  const rpcUrl = process.env.RPC_URL || process.env.RPC_URL || 'https://api.mainnet.solana.com'
  const programId = new PublicKey('4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5')
  const mint = new PublicKey('AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5')

  // Load admin keypair (must be current admin to call update_protocol)
  const adminPath = process.env.ADMIN_KEY || '/home/twzrd/.config/solana/authority-keypair.json'
  let admin: Keypair
  try {
    admin = Keypair.fromSecretKey(
      Uint8Array.from(JSON.parse(fs.readFileSync(adminPath, 'utf-8')))
    )
  } catch (err) {
    console.error(`Failed to load admin keypair from ${adminPath}`)
    console.error('Set ADMIN_KEY environment variable to the correct admin keypair path')
    process.exit(1)
  }

  console.log('Admin keypair:', admin.publicKey.toBase58())

  // New publisher address (oracle-authority.json)
  const newPublisher = new PublicKey('87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy')
  console.log('New publisher:', newPublisher.toBase58())

  const connection = new Connection(rpcUrl, 'confirmed')

  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, mint.toBuffer()],
    programId
  )

  console.log('Protocol state PDA:', protocolState.toBase58())

  // Check current state
  const accountInfo = await connection.getAccountInfo(protocolState)
  if (!accountInfo) {
    console.error('Protocol state account not found!')
    process.exit(1)
  }

  const currentAdmin = new PublicKey(accountInfo.data.slice(8, 40))
  const currentPublisher = new PublicKey(accountInfo.data.slice(40, 72))

  console.log('\nCurrent state:')
  console.log('  Admin:', currentAdmin.toBase58())
  console.log('  Publisher:', currentPublisher.toBase58())

  if (admin.publicKey.toBase58() !== currentAdmin.toBase58()) {
    console.error(`\n❌ Admin keypair mismatch!`)
    console.error(`   Your keypair: ${admin.publicKey.toBase58()}`)
    console.error(`   Required:     ${currentAdmin.toBase58()}`)
    console.error(`\nYou need the admin keypair (${currentAdmin.toBase58()}) to update the protocol state.`)
    process.exit(1)
  }

  console.log('\n✅ Admin keypair matches!')
  console.log('\nUpdating publisher to:', newPublisher.toBase58())

  // Build update_protocol instruction
  // Instruction discriminator for update_protocol
  const disc = crypto.createHash('sha256').update('global:update_protocol').digest().slice(0, 8)

  // Build instruction data
  // For Anchor programs, we typically need to match the instruction format
  // This is a simplified version - you may need to adjust based on your program's update_protocol instruction
  const data = Buffer.concat([
    disc,
    newPublisher.toBuffer(), // new publisher
    Buffer.from([0]), // no admin change
    Buffer.from([0]), // no paused change
  ])

  const ix = new TransactionInstruction({
    keys: [
      { pubkey: admin.publicKey, isSigner: true, isWritable: true },
      { pubkey: protocolState, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    programId,
    data,
  })

  const tx = new Transaction().add(ix)
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash
  tx.feePayer = admin.publicKey
  tx.sign(admin)

  console.log('\nSending transaction...')

  try {
    const sig = await connection.sendRawTransaction(tx.serialize(), {
      skipPreflight: false,
      preflightCommitment: 'confirmed',
    })

    console.log('Transaction sent:', sig)
    console.log(`https://solscan.io/tx/${sig}`)

    await connection.confirmTransaction(sig, 'confirmed')
    console.log('\n✅ Transaction confirmed!')
    console.log('\nPublisher authority updated successfully!')
    console.log('You can now run the publisher script with oracle-authority.json')
  } catch (err: any) {
    console.error('\n❌ Transaction failed:', err.message)
    if (err.logs) {
      console.error('\nProgram logs:')
      err.logs.forEach((log: string) => console.error('  ', log))
    }
    process.exit(1)
  }
}

main().catch((err) => {
  console.error('Error:', err.message)
  process.exit(1)
})
