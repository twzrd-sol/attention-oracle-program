#!/usr/bin/env tsx
/**
 * publish-backlog-postgres.ts
 *
 * Utility to drain unpublished MILO channel epochs backed by PostgreSQL.
 * For each (epoch, channel) pair with `published = 0`, it:
 *  1. Pulls the cached L2 root from the running aggregator (builds if missing)
 *  2. Publishes the root on-chain via `publishRootRing`
 *  3. Marks the epoch as published in the database
 *
 * Environment variables (defaults align with production):
 *   DATABASE_URL  – Postgres connection string
 *   PROGRAM_ID    – MILO program id
 *   MINT_PUBKEY   – MILO mint
 *   PAYER_KEYPAIR – Authority keypair path
 *   RPC_URL       – Solana RPC endpoint
 *   AGGREGATOR_URL – Base URL for aggregator (default http://127.0.0.1:8080)
 *   BATCH_LIMIT   – Max epochs to process this run (default 60)
 */

import { Pool } from 'pg'
import fs from 'node:fs'
import path from 'node:path'
import { publishRootRing } from '../apps/twzrd-aggregator/src/lib/publish.js'

const DATABASE_URL =
  process.env.DATABASE_URL || 'postgresql://twzrd:twzrd_password_2025@localhost:5432/twzrd_oracle'
const PROGRAM_ID =
  process.env.PROGRAM_ID || '4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5'
const MINT_PUBKEY =
  process.env.MINT_PUBKEY || 'AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5'
const PAYER_KEYPAIR =
  process.env.PAYER_KEYPAIR || '/home/twzrd/.config/solana/oracle-authority.json'
const RPC_URL =
  process.env.PUBLISHER_RPC_URLS || process.env.RPC_URL || 'https://api.mainnet-beta.solana.com'
const AGGREGATOR_URL = process.env.AGGREGATOR_URL || `http://127.0.0.1:${process.env.AGGREGATOR_PORT || 8080}`
const BATCH_LIMIT = Number(process.env.BATCH_LIMIT || 60)

const pool = new Pool({ connectionString: DATABASE_URL, ssl: { rejectUnauthorized: false } as any })

// Options / gates
const DRY_RUN = process.argv.includes('--dry-run')
const MIN_CLAIMS = Number(process.env.LIVE_GATE_MIN_CLAIMS || 5)
const ALLOWLIST_PATH = process.env.CORE_ALLOWLIST_PATH || 'clean-hackathon/exports/core33_allowlist.txt'
let ALLOWLIST: Set<string> | null = null
try {
  if (fs.existsSync(ALLOWLIST_PATH)) {
    const list = fs.readFileSync(ALLOWLIST_PATH, 'utf8').split(/\r?\n/).map(s=>s.trim().toLowerCase()).filter(Boolean)
    ALLOWLIST = new Set(list)
  }
} catch {}

type Observation = {
  channel: string
  epoch: number
  eligible: boolean
  reason?: string
  claimCount?: number
  root?: string
  tx?: string
}
const OBS: Observation[] = []

type BacklogRow = { epoch: number; channel: string }

async function fetchBacklog(): Promise<BacklogRow[]> {
  const client = await pool.connect()
  try {
    const miloChannels = (process.env.MILO_CHANNELS || '')
      .split(',')
      .map((s) => s.trim().toLowerCase())
      .filter(Boolean)

    const where: string[] = ["(published IS NULL OR published = 0)"]
    if (miloChannels.length > 0) where.push('LOWER(channel) = ANY($2)')
    where.push("COALESCE(token_group,'MILO')='MILO'")

    const params: any[] = [BATCH_LIMIT]
    if (miloChannels.length > 0) params.push(miloChannels)

    const sql = `
      SELECT epoch, channel
      FROM sealed_epochs
      WHERE ${where.join(' AND ')}
      ORDER BY epoch ASC
      LIMIT $1
    `
    const { rows } = await client.query<BacklogRow>(sql, params)
    return rows
  } finally {
    client.release()
  }
}

async function getL2Root(channel: string, epoch: number): Promise<{ root: string; count: number } | null> {
  // Fast path: read from l2_tree_cache directly
  const client = await pool.connect()
  try {
    const { rows } = await client.query<{ root: string; participant_count: string }>(
      `SELECT root, participant_count FROM l2_tree_cache WHERE epoch = $1 AND channel = $2 LIMIT 1`,
      [epoch, channel]
    )
    if (rows.length > 0 && rows[0].root) {
      return { root: rows[0].root.replace(/^0x/, ''), count: Math.max(1, parseInt(rows[0].participant_count || '1')) }
    }
  } finally {
    client.release()
  }

  // Slow path: ask aggregator to build, then try cache again
  try {
    const artifactUrl = `${AGGREGATOR_URL}/claim-artifact?channel=${encodeURIComponent(channel)}&epoch=${epoch}`
    const art = await fetch(artifactUrl)
    if (!art.ok) return null
  } catch {
    return null
  }

  const client2 = await pool.connect()
  try {
    const { rows } = await client2.query<{ root: string; participant_count: string }>(
      `SELECT root, participant_count FROM l2_tree_cache WHERE epoch = $1 AND channel = $2 LIMIT 1`,
      [epoch, channel]
    )
    if (rows.length > 0 && rows[0].root) {
      return { root: rows[0].root.replace(/^0x/, ''), count: Math.max(1, parseInt(rows[0].participant_count || '1')) }
    }
    return null
  } finally {
    client2.release()
  }
}

async function markPublished(channel: string, epoch: number) {
  const client = await pool.connect()
  try {
    await client.query(
      `UPDATE sealed_epochs SET published = 1 WHERE channel = $1 AND epoch = $2 AND COALESCE(token_group,'MILO')='MILO'`,
      [channel, epoch]
    )
  } finally {
    client.release()
  }
}

async function main() {
  console.log('🔄 Publish backlog (Postgres)')
  if (ALLOWLIST) console.log(`• Allowlist loaded (${ALLOWLIST.size}) from ${ALLOWLIST_PATH}`)
  console.log(`• Liveness gate MIN_CLAIMS=${MIN_CLAIMS}${DRY_RUN ? ' • DRY-RUN' : ''}`)
  const backlog = await fetchBacklog()
  if (!backlog.length) {
    console.log('✅ Nothing to publish – backlog already empty.')
    return
  }

  console.log(`Found ${backlog.length} unpublished epochs.`)

  let success = 0
  let skipped = 0

  for (const { channel, epoch } of backlog) {
    console.log(`\n→ ${channel}:${epoch}`)

    try {
      if (ALLOWLIST && !ALLOWLIST.has(channel.toLowerCase())) {
        console.log('  • Skipped (not in allowlist)')
        OBS.push({ channel, epoch, eligible: false, reason: 'not_allowlisted' })
        skipped++
        continue
      }
      const l2 = await getL2Root(channel, epoch)
      if (!l2) {
        console.warn('  ! Unable to fetch L2 root – skipping')
        OBS.push({ channel, epoch, eligible: false, reason: 'no_l2_root' })
        skipped++
        continue
      }

      console.log(`  Root: ${l2.root.slice(0, 12)}… count=${l2.count}`)

      if (l2.count < MIN_CLAIMS) {
        console.log(`  • Skipped (liveness < ${MIN_CLAIMS})`)
        OBS.push({ channel, epoch, eligible: false, reason: 'below_min_claims', claimCount: l2.count, root: l2.root })
        skipped++
        continue
      }

      try {
        if (DRY_RUN) {
          console.log('  • DRY-RUN: would publish root')
          OBS.push({ channel, epoch, eligible: true, claimCount: l2.count, root: l2.root })
          success++
          continue
        }
        const sig = await publishRootRing({
          rpcUrl: RPC_URL,
          programId: PROGRAM_ID,
          mintPubkey: MINT_PUBKEY,
          payerKeypairPath: PAYER_KEYPAIR,
          channel,
          epoch,
          l2RootHex: l2.root,
          claimCount: l2.count,
        })

        console.log(`  ✓ Published tx=${sig}`)
        OBS.push({ channel, epoch, eligible: true, claimCount: l2.count, root: l2.root, tx: sig })
      } catch (err: any) {
        const msg = String(err?.message || '')
        if (msg.includes('0x1100')) {
          console.log('  • Already published on-chain, marking complete.')
          OBS.push({ channel, epoch, eligible: true, claimCount: l2.count, root: l2.root, reason: 'already_published' })
        } else if (msg.includes('0x1787')) {
          console.log('  • Ring slot advanced (EpochNotIncreasing) – marking complete.')
          OBS.push({ channel, epoch, eligible: true, claimCount: l2.count, root: l2.root, reason: 'slot_advanced' })
        } else {
          throw err
        }
      }

      if (!DRY_RUN) await markPublished(channel, epoch)
      success++
    } catch (err: any) {
      console.error(`  ✖ ${err?.message || err}`)
      skipped++
      OBS.push({ channel, epoch, eligible: false, reason: 'error:'+String(err?.message||err) })
    }
  }

  console.log('\nDone.')
  console.log(`  Success: ${success}`)
  console.log(`  Skipped/Failed: ${skipped}`)

  try {
    const outDir = path.resolve('clean-hackathon/exports')
    fs.mkdirSync(outDir, { recursive: true })
    const ts = new Date().toISOString().replace(/[:.]/g,'-')
    const out = path.join(outDir, `l2_observation_report_${ts}.json`)
    fs.writeFileSync(out, JSON.stringify({ ts, params: { DRY_RUN, MIN_CLAIMS, allowlistCount: ALLOWLIST?.size || 0 }, items: OBS }, null, 2))
    console.log(`  Wrote observation report: ${out}`)
  } catch {}
}

main()
  .catch((err) => {
    console.error('Fatal error while publishing backlog:', err)
    process.exit(1)
  })
  .finally(() => pool.end())
