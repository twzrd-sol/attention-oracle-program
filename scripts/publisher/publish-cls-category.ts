#!/usr/bin/env tsx
/**
 * Publish CLS root for "crypto" category (aggregated across all channels)
 * Fetches category root from aggregator, publishes to Solana
 */
import { publishRootRing } from '../../apps/twzrd-aggregator/src/lib/publish.js'
import fetch from 'node-fetch'

async function publishCategoryRoot() {
  const clsMint = process.env.CLS_MINT
  const epoch = Number(process.env.EPOCH)
  const rpcUrl = process.env.PUBLISHER_RPC_URLS || process.env.RPC_URL || 'https://api.mainnet-beta.solana.com'
  const programId = process.env.PROGRAM_ID
  const walletPath = process.env.WALLET_PATH
  const aggregatorUrl = process.env.AGGREGATOR_URL || 'http://127.0.0.1:3000'

  if (!clsMint || !epoch || !programId || !walletPath) {
    throw new Error(
      'Missing env vars: CLS_MINT, EPOCH, PROGRAM_ID, WALLET_PATH'
    )
  }

  console.log(`Fetching category root for epoch ${epoch}...`)
  const res = await fetch(
    `${aggregatorUrl}/category/status?epoch=${epoch}`
  )

  if (!res.ok) {
    throw new Error(
      `Failed to fetch category status: ${res.status} ${res.statusText}`
    )
  }

  const data = (await res.json()) as any
  if (data.status !== 'ready') {
    throw new Error(
      `Category tree not ready. Status: ${data.status}. Message: ${data.message}`
    )
  }

  const root = data.root.replace(/^0x/, '')
  const participantCount = data.participantCount

  console.log(
    `Root for crypto category: ${root} (count: ${participantCount})`
  )

  console.log(`Publishing to Solana...`)
  const tx = await publishRootRing({
    rpcUrl,
    programId,
    mintPubkey: clsMint,
    payerKeypairPath: walletPath,
    channel: 'crypto',
    epoch,
    l2RootHex: root,
    claimCount: participantCount,
  })

  console.log(
    `✅ CLS category root published: epoch=${epoch} tx=${tx}`
  )
}

publishCategoryRoot().catch((e) => {
  console.error('✖ publish-cls-category failed:', e.message)
  process.exit(1)
})
