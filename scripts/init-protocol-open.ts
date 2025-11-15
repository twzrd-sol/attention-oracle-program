#!/usr/bin/env tsx
/**
 * Initialize ProtocolState + FeeConfig PDAs for the public program (Token-2022, open variant).
 *
 * SAFE-GUARD: Requires CONFIRM_INIT=YES in env to actually send the tx.
 */
import { Connection, Keypair, PublicKey, SystemProgram } from '@solana/web3.js'
import * as anchor from '@coral-xyz/anchor'
import fs from 'fs'
import path from 'path'
import dotenv from 'dotenv'

dotenv.config({ path: path.resolve(process.cwd(), '.env') })

const PROGRAM_ID = new PublicKey(process.env.PROGRAM_ID || 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop')
const MINT_PUBKEY = new PublicKey(String(process.env.MINT_PUBKEY))
const RPC_URL = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com'

// Choose admin keypair path (prefer aggregator payer)
const ADMIN_KEYPAIR = process.env.ADMIN_KEYPAIR || process.env.PAYER_KEYPAIR || path.join(process.env.HOME || '', '.config/solana/id.json')
const FEE_BPS = Number(process.env.INIT_FEE_BPS || 10) // 0.10%
const MAX_FEE = BigInt(process.env.INIT_MAX_FEE || '1000000000') // 1 token (9 decimals)

async function main() {
  if (String(process.env.CONFIRM_INIT).toUpperCase() !== 'YES') {
    console.error('Refusing to run: set CONFIRM_INIT=YES to proceed (mainnet write).')
    process.exit(2)
  }

  const secret = JSON.parse(fs.readFileSync(ADMIN_KEYPAIR, 'utf8'))
  const admin = Keypair.fromSecretKey(new Uint8Array(secret))
  console.log('Admin:', admin.publicKey.toBase58())

  const connection = new Connection(RPC_URL, 'confirmed')
  const wallet = new anchor.Wallet(admin)
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: 'confirmed' })

  // Load IDL
  const idlPath = path.join(process.cwd(), 'clean-hackathon/target/idl/token_2022.json')
  const idl = JSON.parse(fs.readFileSync(idlPath, 'utf8'))
  const program = new anchor.Program(idl as anchor.Idl, PROGRAM_ID, provider)

  const [protocolPda] = PublicKey.findProgramAddressSync([
    Buffer.from('protocol'),
    MINT_PUBKEY.toBuffer(),
  ], PROGRAM_ID)
  const [feeConfigPda] = PublicKey.findProgramAddressSync([
    Buffer.from('protocol'),
    MINT_PUBKEY.toBuffer(),
    Buffer.from('fee_config'),
  ], PROGRAM_ID)

  const info = await connection.getAccountInfo(protocolPda)
  if (info) {
    console.log('Protocol already initialized at', protocolPda.toBase58())
    return
  }

  console.log('Initializing ProtocolState…')
  await program.methods
    .initializeMintOpen(FEE_BPS, new anchor.BN(MAX_FEE.toString()))
    .accounts({
      admin: admin.publicKey,
      mint: MINT_PUBKEY,
      protocolState: protocolPda,
      feeConfig: feeConfigPda,
      systemProgram: SystemProgram.programId,
    })
    .rpc()
    .then((sig) => console.log('Tx:', sig))
}

main().catch((e) => { console.error(e); process.exit(1) })

