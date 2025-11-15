#!/usr/bin/env tsx
import { Connection, PublicKey } from '@solana/web3.js'
import dotenv from 'dotenv'

dotenv.config({ path: './.env' })

type Stat = { name: string; ok: number; fail: number; avg: number; p95: number }

async function timeit<T>(fn: () => Promise<T>): Promise<[number, T | null, any | null]> {
  const t0 = performance.now()
  try {
    const r = await fn()
    return [performance.now() - t0, r, null]
  } catch (e) {
    return [performance.now() - t0, null, e]
  }
}

async function runOne(name: string, url: string) {
  const conn = new Connection(url, 'confirmed')
  const latencies: Record<string, number[]> = {
    getHealth: [], getSlot: [], getLatestBlockhash: [], getVersion: [], getBlockTime: [], getMultipleAccounts: []
  }
  const errors: string[] = []
  // Warmup
  await conn.getLatestBlockhash().catch(()=>{})

  // Prepare a handful of random public keys (well-known)
  const keys = [
    new PublicKey('11111111111111111111111111111111'), // System
    new PublicKey('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb'), // Token-2022 program
    new PublicKey('BPFLoaderUpgradeab1e11111111111111111111111'),
  ]

  for (let i = 0; i < 10; i++) {
    let d; let err
    ;[d,,err] = await timeit(()=>conn.getHealth()); latencies.getHealth.push(d); if (err) errors.push('getHealth:'+err)
    ;[d,,err] = await timeit(()=>conn.getSlot()); latencies.getSlot.push(d); if (err) errors.push('getSlot:'+err)
    ;[d,,err] = await timeit(()=>conn.getLatestBlockhash('finalized')); latencies.getLatestBlockhash.push(d); if (err) errors.push('getLatestBlockhash:'+err)
    ;[d,,err] = await timeit(()=>conn.getVersion()); latencies.getVersion.push(d); if (err) errors.push('getVersion:'+err)
    const slot = await conn.getSlot('confirmed').catch(()=>null)
    if (slot) { ;[d,,err] = await timeit(()=>conn.getBlockTime(slot)); latencies.getBlockTime.push(d); if (err) errors.push('getBlockTime:'+err) }
    ;[d,,err] = await timeit(()=>conn.getMultipleAccountsInfo(keys)); latencies.getMultipleAccounts.push(d); if (err) errors.push('getMultipleAccounts:'+err)
  }

  const stats: Record<string, Stat> = {}
  for (const [k, arr] of Object.entries(latencies)) {
    const sorted = [...arr].sort((a,b)=>a-b)
    const sum = arr.reduce((a,b)=>a+b,0)
    const avg = sum / Math.max(1, arr.length)
    const p95 = sorted[Math.max(0, Math.floor(0.95*(sorted.length-1)))] || 0
    stats[k] = { name: k, ok: arr.length, fail: 0, avg, p95 }
  }
  return { name, url, stats, errors }
}

async function main() {
  const endpoints = [
    { name: 'Primary (RPC_URL)', url: process.env.RPC_URL! },
    { name: 'Helius Fallback', url: process.env.RPC_URL_HELIUS_FALLBACK! },
  ].filter(e => !!e.url)

  const results = [] as any[]
  for (const ep of endpoints) {
    const r = await runOne(ep.name, ep.url)
    results.push(r)
  }

  console.log(JSON.stringify({ when: new Date().toISOString(), results }, null, 2))
}

main().catch(e => { console.error(e); process.exit(1) })

