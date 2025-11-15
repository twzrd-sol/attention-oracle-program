#!/usr/bin/env tsx
/**
 * Transfer Protocol Admin Authority to Hardware Wallet (Ledger)
 *
 * Usage:
 *   npx tsx scripts/transfer-admin-to-ledger.ts \
 *     --ledger-pubkey <LEDGER_ADDRESS> \
 *     --current-admin ~/.config/solana/oracle-authority.json \
 *     --rpc-url https://api.mainnet-beta.solana.com
 *
 * Prerequisites:
 *   - Current admin keypair (oracle-authority.json)
 *   - Ledger public key (get via: solana-keygen pubkey usb://ledger?key=0/0)
 *   - 0.01 SOL in current admin for transaction fee
 *
 * Security Note:
 *   - Uses the update_admin_open instruction added in program update
 *   - Only transfers admin authority (publisher remains unchanged)
 *   - Test with --dry-run first to validate before executing
 */

import { Connection, Keypair, PublicKey, Transaction, TransactionInstruction } from '@solana/web3.js'
import crypto from 'node:crypto'
import fs from 'node:fs'
import { program } from 'commander'

const PROTOCOL_SEED = Buffer.from('protocol')
const PROGRAM_ID = new PublicKey('4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5')
const MINT = new PublicKey('AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5')

program
  .requiredOption('--ledger-pubkey <pubkey>', 'Ledger hardware wallet public key')
  .requiredOption('--current-admin <path>', 'Path to current admin keypair')
  .option('--rpc-url <url>', 'Solana RPC URL', 'https://api.mainnet-beta.solana.com')
  .option('--dry-run', 'Simulate transaction without sending', false)
  .parse()

const opts = program.opts()

async function main() {
  console.log('=== Transfer Admin to Hardware Wallet ===\n')

  // Load current admin
  const currentAdmin = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(opts.currentAdmin, 'utf-8')))
  )

  // Parse Ledger pubkey
  const ledgerPubkey = new PublicKey(opts.ledgerPubkey)

  console.log('Current admin:', currentAdmin.publicKey.toBase58())
  console.log('New admin (Ledger):', ledgerPubkey.toBase58())
  console.log('RPC:', opts.rpcUrl)
  console.log('Dry run:', opts.dryRun)
  console.log('')

  const connection = new Connection(opts.rpcUrl, 'confirmed')

  // Get protocol state PDA
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, MINT.toBuffer()],
    PROGRAM_ID
  )

  console.log('Protocol state PDA:', protocolState.toBase58())

  // Verify current admin
  const accountInfo = await connection.getAccountInfo(protocolState)
  if (!accountInfo) {
    throw new Error('Protocol state account not found!')
  }

  // Account data layout: [discriminator (8 bytes), Borsh 0101 (2 bytes), admin (32 bytes), ...]
  const currentAdminOnChain = new PublicKey(accountInfo.data.slice(10, 42))
  console.log('Current admin (on-chain):', currentAdminOnChain.toBase58())

  if (currentAdmin.publicKey.toBase58() !== currentAdminOnChain.toBase58()) {
    throw new Error(`Admin mismatch! Your keypair: ${currentAdmin.publicKey.toBase58()}, On-chain: ${currentAdminOnChain.toBase58()}`)
  }

  console.log('✅ Admin verified\n')

  // Build update_admin_open instruction
  // Discriminator for update_admin_open
  const disc = crypto.createHash('sha256').update('global:update_admin_open').digest().slice(0, 8)

  // Instruction data: [discriminator, new_admin (32 bytes)]
  const data = Buffer.concat([
    disc,
    ledgerPubkey.toBuffer(), // new admin
  ])

  const ix = new TransactionInstruction({
    keys: [
      { pubkey: currentAdmin.publicKey, isSigner: true, isWritable: true },
      { pubkey: protocolState, isSigner: false, isWritable: true },
    ],
    programId: PROGRAM_ID,
    data,
  })

  const tx = new Transaction().add(ix)
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash
  tx.feePayer = currentAdmin.publicKey

  if (opts.dryRun) {
    console.log('🔍 Simulating transaction...')
    const simulation = await connection.simulateTransaction(tx)

    if (simulation.value.err) {
      console.error('❌ Simulation failed:')
      console.error(JSON.stringify(simulation.value, null, 2))
      process.exit(1)
    }

    console.log('✅ Simulation successful!')
    console.log('\nLogs:')
    simulation.value.logs?.forEach(log => console.log('  ', log))
    console.log('\n⚠️  Dry run complete. Run without --dry-run to execute.')
    return
  }

  // Sign and send
  tx.sign(currentAdmin)

  console.log('📤 Sending transaction...')
  const sig = await connection.sendRawTransaction(tx.serialize(), {
    skipPreflight: false,
    preflightCommitment: 'confirmed',
  })

  console.log('Transaction sent:', sig)
  console.log(`https://solscan.io/tx/${sig}`)

  console.log('\n⏳ Confirming...')
  await connection.confirmTransaction(sig, 'confirmed')

  console.log('✅ Transaction confirmed!\n')

  // Verify new admin
  const updatedAccountInfo = await connection.getAccountInfo(protocolState)
  if (!updatedAccountInfo) {
    throw new Error('Protocol state account disappeared?!')
  }

  // Account data layout: [discriminator (8 bytes), Borsh 0101 (2 bytes), admin (32 bytes), ...]
  const newAdminOnChain = new PublicKey(updatedAccountInfo.data.slice(10, 42))
  console.log('New admin (on-chain):', newAdminOnChain.toBase58())

  if (newAdminOnChain.toBase58() === ledgerPubkey.toBase58()) {
    console.log('✅ Admin successfully transferred to Ledger!\n')
    console.log('⚠️  IMPORTANT: Test admin operations with Ledger before securing old keypair.')
  } else {
    console.log('❌ Admin transfer may have failed. On-chain admin:', newAdminOnChain.toBase58())
  }
}

main().catch((err) => {
  console.error('\n❌ Error:', err.message)
  process.exit(1)
})
