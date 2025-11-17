#!/usr/bin/env tsx
import { Connection, Keypair, PublicKey, Transaction } from '@solana/web3.js'
import pg from 'pg'
import fs from 'fs'

async function main(){
  const DATABASE_URL = process.env.DATABASE_URL!
  const SOLANA_RPC = process.env.SOLANA_RPC || 'https://api.devnet.solana.com'
  const PROGRAM_ID = new PublicKey(process.env.PROGRAM_ID!)
  const MINT_PUBKEY = new PublicKey(process.env.MINT_PUBKEY!)

  const [,, walletArg, epochArg, keypairPathArg] = process.argv
  if(!walletArg || !epochArg || !keypairPathArg){
    console.error('Usage: tsx scripts/direct-claim-devnet.ts <WALLET> <EPOCH> <KEYPAIR_PATH>')
    process.exit(2)
  }
  const wallet = walletArg
  const epochId = Number(epochArg)
  const keypair = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(keypairPathArg,'utf8'))))

  if(keypair.publicKey.toBase58() !== wallet){
    throw new Error('Keypair pubkey does not match wallet')
  }

  const conn = new Connection(SOLANA_RPC, 'confirmed')
  const pool = new pg.Pool({ connectionString: DATABASE_URL, ssl: { rejectUnauthorized: false } as any })

  // Fetch allocation and root
  const alloc = await pool.query(`SELECT index, amount, id, proof_json FROM allocations WHERE epoch_id=$1 AND wallet=$2`, [epochId, wallet])
  if(alloc.rows.length===0){ throw new Error('No allocation row') }
  const row = alloc.rows[0]
  const proof: string[] = Array.isArray(row.proof_json) ? row.proof_json.map((s:string)=>String(s)) : JSON.parse(row.proof_json)

  const rootRes = await pool.query(`SELECT REPLACE(root,'0x','') AS root FROM sealed_epochs WHERE epoch=$1 AND channel='test-cls' ORDER BY sealed_at DESC LIMIT 1`, [epochId])
  if(rootRes.rows.length===0){ throw new Error('No sealed root') }
  const merkleRoot = rootRes.rows[0].root

  // Import buildClaimTransaction from compiled gateway
  const { buildClaimTransaction } = await import('../gateway/dist/onchain/claim-transaction.js')

  const tx: Transaction = await buildClaimTransaction({
    wallet: new PublicKey(wallet),
    epochId,
    merkleRoot,
    index: Number(row.index),
    amount: BigInt(typeof row.amount==='number'? row.amount : String(row.amount)),
    id: String(row.id),
    proof,
  })

  tx.sign(keypair)
  const sig = await conn.sendRawTransaction(tx.serialize(), { preflightCommitment: 'confirmed' })
  console.log('CLAIM_TX', sig)
  await conn.confirmTransaction(sig, 'confirmed')

  await pool.query(`INSERT INTO cls_claims (wallet, epoch_id, amount, tx_signature, tx_status, confirmed_at) VALUES ($1,$2,$3,$4,'confirmed', NOW()) ON CONFLICT (wallet, epoch_id) DO UPDATE SET amount=$3, tx_signature=$4, tx_status='confirmed', confirmed_at=NOW()`, [wallet, epochId, String(row.amount), sig])
  await pool.end()
}

main().catch((e)=>{ console.error(e); process.exit(1) })

