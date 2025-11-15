#!/usr/bin/env tsx
/**
 * Set Publisher on Singleton Protocol PDA (no mint in seeds)
 * Requires: admin keypair (AmMftc4zHgR4yYfv29awV9Q46emo2aGPFW8utP81CsBv)
 */
import {
  Connection,
  Keypair,
  PublicKey,
  TransactionInstruction,
  Transaction,
  sendAndConfirmTransaction,
} from '@solana/web3.js';
import fs from 'fs';
import crypto from 'crypto';

const RPC_URL = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com';
const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const ADMIN_KEYPAIR_PATH = process.env.ADMIN_KEYPAIR || `${process.env.HOME}/milo-token/keys/admin-keypair.json`;
const NEW_PUBLISHER = new PublicKey('87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy');

// Discriminator for update_publisher (singleton version)
const DISCRIMINATOR = crypto
  .createHash('sha256')
  .update('global:update_publisher')
  .digest()
  .subarray(0, 8);

async function main() {
  console.log('🔐 Update Publisher on Singleton Protocol');
  console.log('Program:', PROGRAM_ID.toBase58());
  console.log('New Publisher:', NEW_PUBLISHER.toBase58());

  const admin = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(ADMIN_KEYPAIR_PATH, 'utf8')))
  );
  console.log('Admin (signer):', admin.publicKey.toBase58());

  const connection = new Connection(RPC_URL, 'confirmed');

  // Singleton protocol PDA (no mint)
  const [protocolState] = PublicKey.findProgramAddressSync(
    [Buffer.from('protocol')],
    PROGRAM_ID
  );
  console.log('Singleton Protocol PDA:', protocolState.toBase58());

  const data = Buffer.alloc(8 + 32);
  DISCRIMINATOR.copy(data, 0);
  NEW_PUBLISHER.toBuffer().copy(data, 8);

  const ix = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: admin.publicKey, isSigner: true, isWritable: false },
      { pubkey: protocolState, isSigner: false, isWritable: true },
    ],
    data,
  });

  try {
    const tx = new Transaction().add(ix);
    const sig = await sendAndConfirmTransaction(connection, tx, [admin], { commitment: 'confirmed' });
    console.log('✅ Publisher updated!');
    console.log('TX:', `https://solscan.io/tx/${sig}`);
  } catch (e: any) {
    console.error('❌ Failed:', e.message);
    if (e.logs) console.error('Logs:', e.logs);
    process.exit(1);
  }
}

main().catch((e) => { console.error(e); process.exit(1); });
