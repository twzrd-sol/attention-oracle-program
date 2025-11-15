#!/usr/bin/env tsx
import { Connection, PublicKey, LAMPORTS_PER_SOL } from '@solana/web3.js'

const RPC_URL = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com'
const WALLET = '2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD'

async function main() {
  const connection = new Connection(RPC_URL, 'confirmed')
  const pubkey = new PublicKey(WALLET)

  console.log(`\n💸 Detailed Spending Analysis`)
  console.log(`Wallet: ${WALLET}\n`)

  // Get last 100 transactions
  const signatures = await connection.getSignaturesForAddress(pubkey, { limit: 100 })

  console.log(`Analyzing ${signatures.length} transactions...\n`)

  let totalFees = 0
  let successCount = 0
  let failCount = 0
  let rentSpent = 0

  for (const sig of signatures) {
    if (sig.err) {
      failCount++
    } else {
      successCount++
    }

    // Get transaction details to see fees
    try {
      const tx = await connection.getTransaction(sig.signature, {
        maxSupportedTransactionVersion: 0
      })

      if (tx && tx.meta) {
        const fee = tx.meta.fee
        totalFees += fee

        // Check if this was a channel initialization (creates account = rent)
        const postBalances = tx.meta.postBalances
        const preBalances = tx.meta.preBalances

        // Rent is typically the first account balance decrease beyond fees
        if (preBalances[0] && postBalances[0]) {
          const spent = preBalances[0] - postBalances[0]
          const rentForTx = spent - fee
          if (rentForTx > 0) {
            rentSpent += rentForTx
          }
        }
      }
    } catch (e) {
      // Skip if we can't fetch details
    }

    // Rate limit
    if (signatures.indexOf(sig) % 10 === 0) {
      await new Promise(resolve => setTimeout(resolve, 100))
    }
  }

  console.log(`\n📊 Summary:`)
  console.log(`  ✅ Successful: ${successCount}`)
  console.log(`  ❌ Failed: ${failCount}`)
  console.log(`  💰 Total Fees: ${(totalFees / LAMPORTS_PER_SOL).toFixed(4)} SOL`)
  console.log(`  🏠 Rent Spent: ${(rentSpent / LAMPORTS_PER_SOL).toFixed(4)} SOL`)
  console.log(`  📉 Total Spent: ${((totalFees + rentSpent) / LAMPORTS_PER_SOL).toFixed(4)} SOL`)
  console.log(`\n  💡 Average per tx: ${((totalFees + rentSpent) / signatures.length / LAMPORTS_PER_SOL).toFixed(6)} SOL`)
}

main().catch(console.error)
