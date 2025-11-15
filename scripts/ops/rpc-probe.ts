#!/usr/bin/env tsx
import 'dotenv/config'
import { Connection, PublicKey } from '@solana/web3.js'
import { performance } from 'node:perf_hooks'

type ProbeResult = {
  method: string
  latencyMs: number
  status: 'OK' | 'FAIL'
  meta?: string
  error?: string
}

const formatName = (url: string, idx: number, names?: string[]): string => {
  if (names && names[idx]) return names[idx]
  try {
    const { hostname } = new URL(url)
    return hostname
  } catch {
    return `rpc-${idx + 1}`
  }
}

const ms = (start: number) => Math.round((performance.now() - start) * 1000) / 1000

async function probeEndpoint(opts: {
  url: string
  name: string
  account?: string
  program?: string
  filterLen?: number
  commitment: 'processed' | 'confirmed'
}): Promise<ProbeResult[]> {
  const { url, name, account, program, filterLen, commitment } = opts
  const connection = new Connection(url, commitment)
  const results: ProbeResult[] = []

  // getLatestBlockhash
  let start = performance.now()
  try {
    await connection.getLatestBlockhash(commitment)
    results.push({ method: 'getLatestBlockhash', latencyMs: ms(start), status: 'OK' })
  } catch (err: any) {
    results.push({
      method: 'getLatestBlockhash',
      latencyMs: ms(start),
      status: 'FAIL',
      error: err?.message ?? String(err),
    })
  }

  // getAccountInfo (if provided)
  if (account) {
    start = performance.now()
    try {
      const info = await connection.getAccountInfo(new PublicKey(account), commitment)
      results.push({
        method: 'getAccountInfo',
        latencyMs: ms(start),
        status: info ? 'OK' : 'FAIL',
        meta: info ? `lamports=${info.lamports}` : 'account not found',
      })
    } catch (err: any) {
      results.push({
        method: 'getAccountInfo',
        latencyMs: ms(start),
        status: 'FAIL',
        error: err?.message ?? String(err),
      })
    }
  }

  // getProgramAccounts (optional)
  if (program) {
    start = performance.now()
    try {
      const filters = filterLen
        ? [{ dataSize: filterLen }]
        : undefined
      const resp = await connection.getProgramAccounts(new PublicKey(program), {
        commitment,
        filters,
        dataSlice: { offset: 0, length: 0 },
      })
      results.push({
        method: 'getProgramAccounts',
        latencyMs: ms(start),
        status: 'OK',
        meta: `count=${resp.length}`,
      })
    } catch (err: any) {
      results.push({
        method: 'getProgramAccounts',
        latencyMs: ms(start),
        status: 'FAIL',
        error: err?.message ?? String(err),
      })
    }
  }

  return results.map((r) => ({ ...r, method: `[${name}] ${r.method}` }))
}

function parseList(src?: string): string[] {
  if (!src) return []
  return Array.from(new Set(src.split(',').map((s) => s.trim()).filter(Boolean)))
}

async function main() {
  const rpcList = parseList(process.env.RPC_URLS || process.env.RPC_URL)
  if (rpcList.length === 0) {
    console.error('No RPC_URLS or RPC_URL provided')
    process.exit(1)
  }
  const names = parseList(process.env.RPC_NAMES)
  const account = process.env.PROBE_ACCOUNT || process.env.PROTOCOL_TREASURY
  const program = process.env.PROBE_PROGRAM_ID || process.env.PROGRAM_ID
  const filterLen = process.env.PROBE_ACCOUNT_SIZE ? Number(process.env.PROBE_ACCOUNT_SIZE) : undefined
  const commitment = (process.env.PROBE_COMMITMENT as 'processed' | 'confirmed') || 'confirmed'

  for (let i = 0; i < rpcList.length; i++) {
    const url = rpcList[i]
    const name = formatName(url, i, names)
    const res = await probeEndpoint({ url, name, account, program, filterLen, commitment })
    res.forEach((r) => {
      const base = `${r.method} ${r.latencyMs.toFixed(3)}ms ${r.status}`
      const extra = r.status === 'OK' ? r.meta : r.error
      console.log(extra ? `${base} ${extra}` : base)
    })
  }
}

main().catch((err) => {
  console.error('rpc-probe failed:', err)
  process.exit(1)
})
