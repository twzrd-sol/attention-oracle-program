#!/usr/bin/env tsx
import { Connection, PublicKey } from '@solana/web3.js'
import { BorshCoder } from '@coral-xyz/anchor'
import fs from 'fs'
import path from 'path'
import dotenv from 'dotenv'

dotenv.config({ path: path.resolve(process.cwd(), '.env') })

const PROGRAM_ID = new PublicKey(process.env.PROGRAM_ID || 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop')
const MINT_PUBKEY = new PublicKey(String(process.env.MINT_PUBKEY))
const RPC_URL = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com'

async function main() {
  const conn = new Connection(RPC_URL, 'confirmed')
  const [protocolPda] = PublicKey.findProgramAddressSync([
    Buffer.from('protocol'),
    MINT_PUBKEY.toBuffer(),
  ], PROGRAM_ID)

  const info = await conn.getAccountInfo(protocolPda)
  if (!info) {
    console.log(JSON.stringify({ exists: false, protocolPda: protocolPda.toBase58() }, null, 2))
    return
  }

  const idlPath = path.join(process.cwd(), 'clean-hackathon/target/idl/token_2022.json')
  const idl = JSON.parse(fs.readFileSync(idlPath, 'utf8'))
  const coder = new BorshCoder(idl)
  const decoded: any = coder.accounts.decode('ProtocolState', info.data)
  console.log(JSON.stringify({
    exists: true,
    protocolPda: protocolPda.toBase58(),
    admin: decoded.admin.toBase58(),
    publisher: decoded.publisher.toBase58(),
    mint: decoded.mint.toBase58(),
    paused: decoded.paused,
    require_receipt: decoded.require_receipt,
  }, null, 2))
}

main().catch((e) => { console.error(e); process.exit(1) })

