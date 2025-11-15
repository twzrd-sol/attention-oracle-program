#!/usr/bin/env tsx
import { Connection, Keypair, PublicKey, Transaction } from '@solana/web3.js'
import { TOKEN_2022_PROGRAM_ID, createMintToInstruction, getAssociatedTokenAddressSync } from '@solana/spl-token'
import dotenv from 'dotenv'
import fs from 'fs'
import path from 'path'

dotenv.config({ path: './.env' })

const RPC_URL = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com'
const PROGRAM_ID = new PublicKey(process.env.PROGRAM_ID!)
const MINT = new PublicKey(process.env.MINT_PUBKEY!)
const MINT_AUTHORITY = process.env.MINT_AUTHORITY_KEYPAIR || ''

async function main() {
  if (!MINT_AUTHORITY) throw new Error('Set MINT_AUTHORITY_KEYPAIR to mint authority keypair path')
  const payerPath = process.env.PAYER_KEYPAIR || MINT_AUTHORITY
  const payer = Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(payerPath,'utf8'))))
  const mintAuth = Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(MINT_AUTHORITY,'utf8'))))

  const conn = new Connection(RPC_URL, 'confirmed')

  const [protocolPda] = PublicKey.findProgramAddressSync([
    Buffer.from('protocol'),
    MINT.toBuffer(),
  ], PROGRAM_ID)
  const treasuryAta = getAssociatedTokenAddressSync(MINT, protocolPda, true, TOKEN_2022_PROGRAM_ID)
  const amountUi = Number(process.env.MINT_AMOUNT || '100000') // default 100k tokens UI
  const decimals = 9
  const amount = BigInt(amountUi) * BigInt(10 ** decimals)

  const ix = createMintToInstruction(MINT, treasuryAta, mintAuth.publicKey, amount, [], TOKEN_2022_PROGRAM_ID)

  const { blockhash, lastValidBlockHeight } = await conn.getLatestBlockhash('finalized')
  const tx = new Transaction({ feePayer: payer.publicKey, blockhash, lastValidBlockHeight })
  tx.add(ix)
  tx.sign(payer, mintAuth)

  const sig = await conn.sendRawTransaction(tx.serialize(), { skipPreflight: false, preflightCommitment: 'confirmed' })
  console.log('MINT_TX', sig)
  await conn.confirmTransaction({ signature: sig, blockhash, lastValidBlockHeight }, 'confirmed')
}

main().catch((e)=>{ console.error(e); process.exit(1) })

