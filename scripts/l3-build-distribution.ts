#!/usr/bin/env tsx
import fs from 'node:fs'
import path from 'node:path'
import { Pool } from 'pg'
import { createHash } from 'crypto'

type Params = {
  epoch: number
  b_ccm_per_epoch: number
  creators_split: number
  channelsFile: string
}

type Row = {
  channel: string
  participants: number
}

function h(s: string) { return createHash('sha256').update(s).digest('hex') }

function merkleRoot(leaves: string[]): { root: string; levels: string[][] } {
  if (leaves.length === 0) return { root: h(''), levels: [[]] }
  let level = leaves.map((x) => h(x))
  const levels: string[][] = [level]
  while (level.length > 1) {
    const next: string[] = []
    for (let i = 0; i < level.length; i += 2) {
      const a = level[i]
      const b = level[i + 1] ?? level[i]
      next.push(h(a + b))
    }
    level = next
    levels.push(level)
  }
  return { root: level[0], levels }
}

async function main() {
  const epochArg = process.argv.indexOf('--epoch')
  if (epochArg === -1 || !process.argv[epochArg + 1]) {
    console.error('Usage: npx tsx scripts/l3-build-distribution.ts --epoch <E> --channels-file <path> [--b 600] [--dry-run]')
    process.exit(2)
  }
  const epoch = parseInt(process.argv[epochArg + 1], 10)
  const chArg = process.argv.indexOf('--channels-file')
  const channelsFile = chArg !== -1 ? process.argv[chArg + 1] : 'clean-hackathon/exports/core33_allowlist.txt'
  const bArg = process.argv.indexOf('--b')
  const B = bArg !== -1 ? parseFloat(process.argv[bArg + 1]) : 600
  const DRY = process.argv.includes('--dry-run')

  const allowlist = fs.readFileSync(channelsFile, 'utf8').split(/\r?\n/).map(s=>s.trim()).filter(Boolean)
  if (allowlist.length === 0) throw new Error('Allowlist is empty')

  const dbUrl = process.env.DATABASE_URL || 'postgresql://doadmin:***@localhost:5432/placeholder'
  const pool = new Pool({ connectionString: dbUrl, ssl: { rejectUnauthorized: false } })

  // Latest-epoch participants per channel for the specified epoch
  const sql = `
    SELECT sp.channel, COUNT(*)::bigint AS participants
    FROM sealed_participants sp
    WHERE sp.channel = ANY($1) AND sp.epoch = $2
    GROUP BY sp.channel
  `
  const res = await pool.query(sql, [allowlist, epoch])
  const rows: Row[] = res.rows.map(r => ({ channel: String(r.channel), participants: Number(r.participants) }))

  const totalParticipants = rows.reduce((s, r) => s + r.participants, 0)
  const creatorsSplit = 0.70 // creators share inside alpha (voucher launch is creator-only, conservative)
  const alpha = totalParticipants > 0 ? (creatorsSplit * B) / totalParticipants : 0

  const distribution = rows.map(r => ({ channel: r.channel, participants: r.participants, ccm: +(alpha * r.participants).toFixed(6) }))
  const totalCcm = distribution.reduce((s, r) => s + r.ccm, 0)

  // Merkle over JSON leaf: {epoch,channel,amount}
  const leaves = distribution.map(d => JSON.stringify({ epoch, channel: d.channel, amount: d.ccm }))
  const { root } = merkleRoot(leaves)

  const outDir = path.resolve('clean-hackathon/exports')
  fs.mkdirSync(outDir, { recursive: true })
  const base = `l3_epoch_${epoch}`
  const payload = { params: { epoch, b_ccm_per_epoch: B, creators_split: creatorsSplit, alpha_cls: alpha }, totals: { participants: totalParticipants, ccm: +totalCcm.toFixed(6) }, root, leaves: distribution }
  const jsonPath = path.join(outDir, `${base}_distribution.json`)
  fs.writeFileSync(jsonPath, JSON.stringify(payload, null, 2))
  fs.writeFileSync(path.join(outDir, `${base}_root.txt`), root + '\n')

  console.log(`Epoch ${epoch} L3 distribution built.`)
  console.log(`  Participants total: ${totalParticipants}`)
  console.log(`  B_ccm_per_epoch:   ${B}`)
  console.log(`  alpha_CLS:         ${alpha.toFixed(6)}`)
  console.log(`  Total CCM (creators): ${totalCcm.toFixed(6)}`)
  console.log(`  Merkle root:       ${root}`)
  console.log(`  Wrote: ${jsonPath}`)

  await pool.end()
}

main().catch((e) => { console.error(e); process.exit(1) })

