#!/usr/bin/env tsx
import { Connection, PublicKey, LAMPORTS_PER_SOL } from '@solana/web3.js'

const RPC_URL = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com'
const WALLET = process.argv[2] || '2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD'

async function main() {
  const connection = new Connection(RPC_URL, 'confirmed')
  const pubkey = new PublicKey(WALLET)

  console.log(`\n💰 Wallet Balance Check`)
  console.log(`Address: ${WALLET}`)
  console.log(`Network: ${RPC_URL.includes('devnet') ? 'Devnet' : 'Mainnet-Beta'}`)

  // Get balance
  const balance = await connection.getBalance(pubkey)
  console.log(`\nBalance: ${(balance / LAMPORTS_PER_SOL).toFixed(4)} SOL (${balance} lamports)`)

  // Get recent transactions
  console.log(`\n📜 Recent Transactions (last 10):\n`)
  const signatures = await connection.getSignaturesForAddress(pubkey, { limit: 10 })

  signatures.forEach((sig, i) => {
    const date = new Date(sig.blockTime! * 1000).toISOString().replace('T', ' ').slice(0, 19)
    const status = sig.err ? '❌ FAILED' : '✅ SUCCESS'
    console.log(`${i + 1}. ${status} - ${date}`)
    console.log(`   Signature: ${sig.signature}`)
    console.log(`   Explorer: https://explorer.solana.com/tx/${sig.signature}`)
    console.log()
  })
}

main().catch(console.error)
