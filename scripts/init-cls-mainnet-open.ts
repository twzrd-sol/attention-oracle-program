#!/usr/bin/env tsx
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  TransactionInstruction,
  Transaction,
  sendAndConfirmTransaction
} from '@solana/web3.js';
import fs from 'fs';
import crypto from 'crypto';

const RPC_URL = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com';
const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const ADMIN_KEYPAIR_PATH = process.env.HOME + '/milo-token/keys/admin-keypair.json';
const CLS_MINT = new PublicKey('FZnUPK6eRWSQFEini3Go11JmVEqRNAQZgDP7q1DhyaKo');

// Discriminator for initialize_mint_open
const INIT_DISCRIMINATOR = crypto
  .createHash('sha256')
  .update('global:initialize_mint_open')
  .digest()
  .subarray(0, 8);

async function main() {
  console.log('🚀 Initializing CLS Protocol (OPEN Variant)');

  const adminKeypair = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(ADMIN_KEYPAIR_PATH, 'utf8')))
  );
  console.log('Admin:', adminKeypair.publicKey.toBase58());
  console.log('Mint:', CLS_MINT.toBase58());

  const connection = new Connection(RPC_URL, 'confirmed');

  // OPEN variant seeds: [b"protocol", mint]
  const [protocolState, protocolBump] = PublicKey.findProgramAddressSync(
    [Buffer.from('protocol'), CLS_MINT.toBuffer()],
    PROGRAM_ID
  );
  const [feeConfig, feeBump] = PublicKey.findProgramAddressSync(
    [Buffer.from('protocol'), CLS_MINT.toBuffer(), Buffer.from('fee_config')],
    PROGRAM_ID
  );

  console.log('Protocol State PDA:', protocolState.toBase58());
  console.log('Fee Config PDA:', feeConfig.toBase58());

  const accountInfo = await connection.getAccountInfo(protocolState);
  if (accountInfo) {
    console.log('✅ Already initialized!');
    return;
  }

  const feeBasisPoints = 10; // 0.1%
  const maxFee = 100000000000; // 100 CLS (9 decimals)

  const instructionData = Buffer.alloc(8 + 2 + 8);
  INIT_DISCRIMINATOR.copy(instructionData, 0);
  instructionData.writeUInt16LE(feeBasisPoints, 8);
  instructionData.writeBigUInt64LE(BigInt(maxFee), 10);

  const instruction = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: adminKeypair.publicKey, isSigner: true, isWritable: true },
      { pubkey: CLS_MINT, isSigner: false, isWritable: false },
      { pubkey: protocolState, isSigner: false, isWritable: true },
      { pubkey: feeConfig, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: instructionData,
  });

  try {
    console.log('\nSending transaction...');
    const transaction = new Transaction().add(instruction);
    const sig = await sendAndConfirmTransaction(connection, transaction, [adminKeypair], {
      commitment: 'confirmed',
      skipPreflight: false,
    });

    console.log('\n✅ CLS Protocol initialized (OPEN variant)!');
    console.log('Signature:', sig);
    console.log('Explorer:', `https://explorer.solana.com/tx/${sig}`);
    console.log('\nProtocol State:', protocolState.toBase58());
    console.log('Fee Config:', feeConfig.toBase58());
  } catch (e: any) {
    console.error('\n❌ Error:', e.message);
    if (e.logs) {
      console.log('\nProgram Logs:');
      e.logs.forEach((log: string) => console.log('  ', log));
    }
  }
}

main().catch(console.error);
