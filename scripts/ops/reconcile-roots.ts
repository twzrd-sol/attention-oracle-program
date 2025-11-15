#!/usr/bin/env tsx
/**
 * Reconcile sealed roots in Postgres with on-chain ChannelState ring slots.
 * - Reads recent sealed epochs from `sealed_epochs`
 * - Derives ChannelState PDA for (mint, streamerKey(channel))
 * - Loads on-chain slot for epoch and compares root + claim_count
 *
 * Env:
 *   DATABASE_URL=postgresql://...
 *   PROGRAM_ID=GnGzNdsQ...
 *   MINT_PUBKEY=<mint pubkey>
 *   RPC_URLS="https://rpc1,...,https://rpcN" (optional; comma‑sep)
 *   LIMIT=50 (optional)
 *   SLACK_WEBHOOK=... (optional)
 */
import 'dotenv/config'
import { Client as PgClient } from 'pg'
import { Connection, PublicKey } from '@solana/web3.js'
import { sha3_256 } from 'js-sha3'

const CHANNEL_RING_SLOTS = 10
const CHANNEL_MAX_CLAIMS = 1024
const CHANNEL_BITMAP_BYTES = Math.ceil(CHANNEL_MAX_CLAIMS / 8)
const CHANNEL_SLOT_LEN = 8 + 32 + 2 + CHANNEL_BITMAP_BYTES
const HEADER_LEN = 8 + 1 + 1 + 32 + 32 + 8 // disc + version + bump + mint + streamer + latest_epoch

function expandUrlList(raw?: string): string[] {
  if (!raw) return []
  return raw
    .split(',')
    .map((s) => s.trim())
    .filter((s) => s.length > 0)
}

function getConnections(): Connection[] {
  const urls = Array.from(new Set([
    ...expandUrlList(process.env.RECONCILE_RPC_URLS),
    ...expandUrlList(process.env.AGGREGATOR_RPC_URLS),
    ...expandUrlList(process.env.PUBLISHER_RPC_URLS),
    ...expandUrlList(process.env.RPC_URLS),
    process.env.RPC_URL,
  ].filter(Boolean)))

  const final = urls.length > 0 ? urls : ['https://api.mainnet-beta.solana.com']
  return final.map((u) => new Connection(u, 'confirmed'))
}

async function tryRpc<T>(f: (c: Connection) => Promise<T>): Promise<T> {
  let last: any
  for (const c of getConnections()) {
    try { return await f(c) } catch (e) { last = e }
  }
  throw last || new Error('All RPC endpoints failed')
}

function deriveStreamerKey(channel: string): PublicKey {
  const lower = channel.trim().toLowerCase()
  const hash = Buffer.from(sha3_256.arrayBuffer(Buffer.concat([Buffer.from('twitch:'), Buffer.from(lower)])))
  return new PublicKey(hash)
}

function slotIndex(epoch: number): number { return epoch % CHANNEL_RING_SLOTS }

function readSlot(buf: Buffer, epoch: number) {
  const idx = slotIndex(epoch)
  const base = HEADER_LEN + idx * CHANNEL_SLOT_LEN
  const epochLE = buf.readBigUInt64LE(base)
  const slotEpoch = Number(epochLE)
  const root = buf.subarray(base + 8, base + 8 + 32)
  const claimCount = buf.readUInt16LE(base + 8 + 32)
  return { slotEpoch, rootHex: Buffer.from(root).toString('hex'), claimCount }
}

async function slack(text: string) {
  const url = process.env.SLACK_WEBHOOK
  if (!url) return
  try {
    await fetch(url, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ text }) })
  } catch {}
}

async function main() {
  if (process.argv.includes('--test-alert')) {
    await slack(`[TWZRD Reconcile] Test alert at ${new Date().toISOString()}`)
    console.log('Sent test Slack alert.')
    return
  }

  const programIdRaw = process.env.PROGRAM_ID
  const mintRaw = process.env.MINT_PUBKEY
  const limit = Number(process.env.LIMIT || 50)
  if (!process.env.DATABASE_URL || !programIdRaw || !mintRaw) {
    throw new Error('Missing env: DATABASE_URL, PROGRAM_ID, MINT_PUBKEY')
  }

  const programId = new PublicKey(programIdRaw)
  const mint = new PublicKey(mintRaw)

  const pg = new PgClient({ connectionString: process.env.DATABASE_URL })
  await pg.connect()

  const columnInfo = await pg.query(
    `select column_name from information_schema.columns
     where table_schema = 'public'
       and table_name = 'sealed_epochs'
       and column_name in ('token_group','category','claim_count')`
  )
  const hasTokenGroup = columnInfo.rows.some((row) => row.column_name === 'token_group')
  const hasCategory = columnInfo.rows.some((row) => row.column_name === 'category')
  const hasClaimCount = columnInfo.rows.some((row) => row.column_name === 'claim_count')

  const selectParts = [
    'e.epoch',
    'e.channel',
    'e.root',
    hasClaimCount
      ? 'e.claim_count'
      : '(select count(*) from sealed_participants sp where sp.epoch = e.epoch and sp.channel = e.channel) as claim_count',
    'coalesce(e.published,0) as published',
    hasTokenGroup ? "coalesce(e.token_group, 'default') as token_group" : null,
    hasCategory ? "coalesce(e.category, 'default') as category" : null,
  ].filter(Boolean)

  const rows = (
    await pg.query(
      `select ${selectParts.join(', ')}
       from sealed_epochs e
       order by e.sealed_at desc nulls last
       limit $1`,
      [limit]
    )
  ).rows as Array<{
    epoch: number
    channel: string
    root: string
    claim_count: number
    published: number
    token_group?: string
    category?: string
  }>

  let mismatches = 0
  for (const r of rows) {
    try {
      // Fetch L2 claim root from cache (this is what gets published on-chain)
      const l2Query = await pg.query(
        `SELECT root, participant_count FROM l2_tree_cache WHERE epoch = $1 AND channel = $2 LIMIT 1`,
        [r.epoch, r.channel]
      )

      if (l2Query.rows.length === 0) {
        console.log(`SKIP  ${formatScope(r, hasTokenGroup, hasCategory)} L2 tree not cached, sealed_root=${r.root.slice(0,8)}.. (not published yet)`)
        continue
      }

      const l2Root = l2Query.rows[0].root.replace(/^0x/, '').toLowerCase()
      const l2ClaimCount = parseInt(l2Query.rows[0].participant_count || '0')

      // Debug: Log that we found L2 cache
      if (process.env.DEBUG) {
        console.log(`DEBUG ${r.channel} epoch=${r.epoch} found_l2_cache root=${l2Root.slice(0,8)} claims=${l2ClaimCount}`)
      }

      const streamer = deriveStreamerKey(r.channel)
      const seeds = [Buffer.from('channel_state'), mint.toBuffer(), streamer.toBuffer()] as Buffer[]
      const [channelState] = PublicKey.findProgramAddressSync(seeds, programId)
      const info = await tryRpc((c) => c.getAccountInfo(channelState))
      if (!info) {
        console.log(`MISS  ${formatScope(r, hasTokenGroup, hasCategory)} on-chain account missing`)
        mismatches++
        continue
      }
      const slot = readSlot(info.data as Buffer, r.epoch)
      const have = slot.rootHex.toLowerCase()

      // Check if the on-chain slot epoch matches our target epoch (ring buffer validation)
      if (slot.slotEpoch !== r.epoch) {
        console.log(`STALE ${formatScope(r, hasTokenGroup, hasCategory)} on-chain slot contains epoch=${slot.slotEpoch} (ring buffer wrapped)`)
        continue
      }

      const ok = l2Root === have && slot.claimCount === Math.min(0xffff, l2ClaimCount)
      if (!ok) {
        console.log(
          `DIFF  ${formatScope(r, hasTokenGroup, hasCategory)} root_onchain=${have.slice(0,8)}.. root_l2cache=${l2Root.slice(0,8)}.. ` +
          `claims_onchain=${slot.claimCount} claims_l2=${l2ClaimCount}`
        )
        mismatches++
      } else {
        console.log(`OK    ${formatScope(r, hasTokenGroup, hasCategory)} root=${have.slice(0,8)}.. claims=${slot.claimCount}`)
      }
    } catch (e: any) {
      console.log(`ERR   ${formatScope(r, hasTokenGroup, hasCategory)} ${e?.message || e}`)
      mismatches++
    }
  }

  if (mismatches > 0) {
    await slack(`[TWZRD Reconcile] ${mismatches} issue(s) in last ${rows.length} records`)
    process.exitCode = 2
  } else {
    console.log(`All good. Checked ${rows.length} rows.`)
  }

  await pg.end()
}

main().catch((e) => {
  console.error('reconcile-roots failed:', e)
  process.exit(1)
})

function formatScope(
  row: { channel: string; epoch: number; token_group?: string; category?: string },
  hasTokenGroup: boolean,
  hasCategory: boolean
): string {
  const parts = [row.channel]
  if (hasTokenGroup) parts.push(`group=${row.token_group}`)
  if (hasCategory) parts.push(`category=${row.category}`)
  parts.push(`epoch=${row.epoch}`)
  return parts.join(' ')
}
