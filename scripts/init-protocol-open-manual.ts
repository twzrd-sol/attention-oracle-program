#!/usr/bin/env tsx
import { Connection, Keypair, PublicKey, SystemProgram, Transaction, TransactionInstruction } from '@solana/web3.js'
import crypto from 'crypto'
import fs from 'fs'

function u16le(n: number) { const b = Buffer.alloc(2); b.writeUInt16LE(n, 0); return b }
function u64le(n: bigint) { const b = Buffer.alloc(8); b.writeBigUInt64LE(n, 0); return b }

async function main(){
  const PROGRAM_ID = new PublicKey(process.env.PROGRAM_ID!)
  const MINT = new PublicKey(process.env.MINT_PUBKEY!)
  const RPC = process.env.SOLANA_RPC || process.env.RPC_URL || 'https://api.devnet.solana.com'
  const KEYPAIR_PATH = process.env.KEYPAIR_PATH || `${process.env.HOME}/.config/solana/id.json`
  const FEE_BPS = Number(process.env.INIT_FEE_BPS || '10') // 0.10%
  const MAX_FEE = BigInt(process.env.INIT_MAX_FEE || '1000000000') // 1 token (9 decimals)

  const payer = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(KEYPAIR_PATH,'utf8'))))
  const conn = new Connection(RPC, 'confirmed')

  const [protocolState] = PublicKey.findProgramAddressSync([Buffer.from('protocol'), MINT.toBuffer()], PROGRAM_ID)
  const [feeConfig] = PublicKey.findProgramAddressSync([Buffer.from('protocol'), MINT.toBuffer(), Buffer.from('fee_config')], PROGRAM_ID)

  const disc = crypto.createHash('sha256').update('global:initialize_mint_open').digest().subarray(0,8)
  const data = Buffer.concat([disc, u16le(FEE_BPS), u64le(MAX_FEE)])

  const ix = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: payer.publicKey, isSigner: true, isWritable: true }, // admin
      { pubkey: MINT, isSigner: false, isWritable: false },
      { pubkey: protocolState, isSigner: false, isWritable: true },
      { pubkey: feeConfig, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  })

  const { blockhash, lastValidBlockHeight } = await conn.getLatestBlockhash('finalized')
  const tx = new Transaction({ feePayer: payer.publicKey, blockhash, lastValidBlockHeight })
  tx.add(ix)
  tx.sign(payer)
  const sig = await conn.sendRawTransaction(tx.serialize(), { preflightCommitment: 'confirmed' })
  console.log('INIT_OPEN_TX', sig)
  await conn.confirmTransaction({ signature: sig, blockhash, lastValidBlockHeight }, 'confirmed')
}

main().catch((e)=>{ console.error(e); process.exit(1) })

