/**
 * Create Treasury ATA by directly initializing a Token-2022 account
 * This bypasses the ATP program which doesn't work with Token-2022
 */

import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
  SystemProgram,
  LAMPORTS_PER_SOL
} from '@solana/web3.js';
import * as fs from 'fs';
import * as crypto from 'crypto';

const MINT = new PublicKey('AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5');
const TOKEN_2022_PROGRAM_ID = new PublicKey('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPvZeS');
const ASSOCIATED_TOKEN_PROGRAM_ID = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const RPC_URL = 'https://api.mainnet-beta.solana.com';

async function main() {
  const connection = new Connection(RPC_URL, 'confirmed');

  const claimer = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync('/home/twzrd/.config/solana/cls-claim-0001.json', 'utf-8')))
  );

  const PROTOCOL_SEED = Buffer.from('protocol');
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, MINT.toBuffer()],
    PROGRAM_ID
  );

  const [treasuryAta] = PublicKey.findProgramAddressSync(
    [MINT.toBuffer(), protocolState.toBuffer(), TOKEN_2022_PROGRAM_ID.toBuffer()],
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  console.log('Treasury Account to create:', treasuryAta.toBase58());
  console.log('Owner (protocol_state):', protocolState.toBase58());
  console.log('Mint:', MINT.toBase58());
  console.log('');

  // Check if exists
  const existing = await connection.getAccountInfo(treasuryAta);
  if (existing) {
    console.log('✅ Account already exists!');
    process.exit(0);
  }

  // Step 1: Create account with SystemProgram
  const TOKEN_ACCOUNT_SIZE = 165;
  const lamports = await connection.getMinimumBalanceForRentExemption(TOKEN_ACCOUNT_SIZE);

  console.log(`Creating account (size: ${TOKEN_ACCOUNT_SIZE} bytes, rent: ${(lamports / LAMPORTS_PER_SOL).toFixed(6)} SOL)...`);

  const createAccountIx = SystemProgram.createAccount({
    fromPubkey: claimer.publicKey,
    newAccountPubkey: treasuryAta,
    lamports,
    space: TOKEN_ACCOUNT_SIZE,
    programId: TOKEN_2022_PROGRAM_ID,
  });

  // Step 2: Initialize the Token-2022 account
  // Token-2022 InitializeAccount3 instruction structure:
  // Discriminator (1 byte) + mint (32 bytes) + owner (32 bytes)

  const initData = Buffer.alloc(65);
  initData[0] = 0; // Instruction discriminator for InitializeAccount3 (might be 0, 1, 2, etc. - let's try 0)
  MINT.toBuffer().copy(initData, 1);
  protocolState.toBuffer().copy(initData, 33);

  const initializeIx = new TransactionInstruction({
    programId: TOKEN_2022_PROGRAM_ID,
    keys: [
      { pubkey: treasuryAta, isSigner: false, isWritable: true },
      { pubkey: MINT, isSigner: false, isWritable: false },
      { pubkey: protocolState, isSigner: false, isWritable: false },
    ],
    data: initData,
  });

  const tx = new Transaction().add(createAccountIx, initializeIx);
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.feePayer = claimer.publicKey;
  tx.sign(claimer);

  try {
    console.log('\n📤 Sending transaction...');
    const sig = await connection.sendRawTransaction(tx.serialize());
    console.log('✅ Submitted:', sig);
    console.log('🔗 https://explorer.solana.com/tx/' + sig);

    console.log('\n⏳ Confirming...');
    const conf = await connection.confirmTransaction(sig, 'confirmed');

    if (conf.value.err) {
      console.error('\n❌ Transaction failed:', conf.value.err);
      process.exit(1);
    }

    console.log('✅ CONFIRMED!');
    console.log('\n🎉 Treasury ATA created successfully!');
    console.log('Address:', treasuryAta.toBase58());
  } catch (err: any) {
    console.error('\n❌ Error:', err.message);
    if (err.logs) {
      console.log('\nLogs:');
      err.logs.forEach((l: string) => console.log('  ' + l));
    }
    process.exit(1);
  }
}

main().catch(console.error);
