#!/usr/bin/env tsx
import { Connection, PublicKey } from '@solana/web3.js'
import { createHash } from 'crypto'
import * as fs from 'fs'

const RPC = process.env.RPC_URL || 'https://solana-mainnet.api.syndica.io/api-key/3RUSu4CASNgJUXfZCWMTk949UtkS4WVh1JzngExSKLcu89P7hMD39PLWdqBfA6uneHhaM64FqgteGUYPsdyVhpfJwQd8Mht48q4'
const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop')
const MINT = new PublicKey('AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5')
const CHANNELS = ['threadguy_live','thread_guytv']

function derivePda(ch: string): PublicKey {
  const lower = ch.toLowerCase()
  const hash = createHash('sha3-256').update(Buffer.concat([Buffer.from('channel:'), Buffer.from(lower)])).digest()
  const [pda] = PublicKey.findProgramAddressSync([Buffer.from('channel_state'), MINT.toBuffer(), hash.slice(0,32)], PROGRAM_ID)
  return pda
}

async function main(){
  const conn = new Connection(RPC, 'confirmed')
  const out:any = { accounts: [] as any[] }
  for(const ch of CHANNELS){
    const pda = derivePda(ch)
    const info = await conn.getAccountInfo(pda)
    const sol = info ? info.lamports/1e9 : 0
    out.accounts.push({ channel: ch, pda: pda.toBase58(), lamports: info?.lamports||0, sol })
  }
  const path = '/home/twzrd/milo-token/exports/ghost-threadguy.json'
  fs.writeFileSync(path, JSON.stringify(out, null, 2))
  console.log(path)
}

main().catch(e=>{console.error(e); process.exit(1)})
