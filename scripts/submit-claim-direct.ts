#!/usr/bin/env tsx
/**
 * Direct claim submission for Claim #0001
 * Builds and submits the claim transaction directly without using the gateway
 */

import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
  SystemProgram,
} from '@solana/web3.js';
import * as fs from 'fs';
import * as crypto from 'crypto';

const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const MINT = new PublicKey('AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5');
const RPC_URL = 'https://api.mainnet-beta.solana.com';

const PROTOCOL_SEED = Buffer.from('protocol');
const EPOCH_STATE_SEED = Buffer.from('epoch_state');

async function main() {
  const connection = new Connection(RPC_URL, 'confirmed');

  // Test wallet (claimer)
  const claimerKeypair = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync('/home/twzrd/.config/solana/cls-claim-0001.json', 'utf-8')))
  );

  // Payer (can be different)
  const payerPath = `${process.env.HOME}/.config/solana/id.json`;
  const payer = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(payerPath, 'utf-8')))
  );

  console.log('🚀 Submitting Claim #0001 (Direct)');
  console.log('===================================\n');

  // Claim parameters
  const index = 0;
  const amount = BigInt('100000000000'); // 100 CCM
  const id = 'claim-0001';
  const epoch = 424243;
  const proof: Buffer[] = []; // Empty for single-entry tree

  // Derive PDAs
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, MINT.toBuffer()],
    PROGRAM_ID
  );

  const epochBuf = Buffer.alloc(8);
  epochBuf.writeBigUInt64LE(BigInt(epoch), 0);

  // Derive streamer key (for epoch state)
  const channel = 'claim-0001-test';
  const channelBytes = Buffer.from(channel.toLowerCase());
  const { keccak_256 } = require('@noble/hashes/sha3');
  const streamerKeyHash = keccak_256(Buffer.concat([Buffer.from('twitch:'), channelBytes]));
  const streamerKey = new PublicKey(Buffer.from(streamerKeyHash));

  const [epochState] = PublicKey.findProgramAddressSync(
    [EPOCH_STATE_SEED, epochBuf, streamerKey.toBuffer(), MINT.toBuffer()],
    PROGRAM_ID
  );

  // Token-2022 constants
  const TOKEN_2022_PROGRAM_ID = new PublicKey('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPvZeS');
  const ASSOCIATED_TOKEN_PROGRAM_ID = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');

  // Derive claimer ATA - PDA from [mint, claimer, token_program_id]
  const [claimerAta] = PublicKey.findProgramAddressSync(
    [MINT.toBuffer(), claimerKeypair.publicKey.toBuffer(), TOKEN_2022_PROGRAM_ID.toBuffer()],
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  // Derive treasury ATA - PDA from [mint, treasury_pda, token_program_id]
  const [treasuryAta] = PublicKey.findProgramAddressSync(
    [MINT.toBuffer(), protocolState.toBuffer(), TOKEN_2022_PROGRAM_ID.toBuffer()],
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  console.log(`Claimer: ${claimerKeypair.publicKey.toBase58()}`);
  console.log(`Payer: ${payer.publicKey.toBase58()}`);
  console.log(`Protocol State: ${protocolState.toBase58()}`);
  console.log(`Epoch State: ${epochState.toBase58()}`);
  console.log(`Streamer Key: ${streamerKey.toBase58()}`);
  console.log(`Mint: ${MINT.toBase58()}`);
  console.log(`Claimer ATA: ${claimerAta.toBase58()}`);
  console.log(`Treasury ATA: ${treasuryAta.toBase58()}`);
  console.log(`\nClaim Parameters:`);
  console.log(`  Index: ${index}`);
  console.log(`  Amount: ${amount.toString()} (100 CCM)`);
  console.log(`  ID: ${id}`);
  console.log(`  Epoch: ${epoch}`);
  console.log(`  Proof: [] (empty - single entry)\n`);

  // Build claim instruction
  // Discriminator for claim_open
  const hash = crypto.createHash('sha256').update('global:claim_open').digest();
  const discriminator = hash.slice(0, 8);

  // Instruction data: discriminator + _streamer_index (u8) + index (u32) + amount (u64) + id_len (4) + id_bytes + proof_count (4) + proof_items
  const streamerIndexBuf = Buffer.alloc(1);
  streamerIndexBuf.writeUInt8(0, 0); // _streamer_index (unused but required)

  const indexBuf = Buffer.alloc(4);
  indexBuf.writeUInt32LE(index, 0);

  const amountBuf = Buffer.alloc(8);
  amountBuf.writeBigUInt64LE(amount, 0);

  const idBuf = Buffer.from(id, 'utf-8');
  const idLenBuf = Buffer.alloc(4);
  idLenBuf.writeUInt32LE(idBuf.length, 0);

  const proofCountBuf = Buffer.alloc(4);
  proofCountBuf.writeUInt32LE(proof.length, 0);

  // Combine all data
  let data = Buffer.concat([
    discriminator,
    streamerIndexBuf,
    indexBuf,
    amountBuf,
    idLenBuf,
    idBuf,
    proofCountBuf,
  ]);

  // Add proof items (each 32 bytes)
  for (const proofItem of proof) {
    data = Buffer.concat([data, Buffer.from(proofItem)]);
  }

  // Build instruction
  const ix = new TransactionInstruction({
    keys: [
      { pubkey: claimerKeypair.publicKey, isSigner: true, isWritable: true },
      { pubkey: protocolState, isSigner: false, isWritable: true },
      { pubkey: epochState, isSigner: false, isWritable: true },
      { pubkey: MINT, isSigner: false, isWritable: false },
      { pubkey: claimerAta, isSigner: false, isWritable: true },
      { pubkey: treasuryAta, isSigner: false, isWritable: true },
      { pubkey: TOKEN_2022_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    programId: PROGRAM_ID,
    data,
  });

  // Build and sign transaction
  const tx = new Transaction().add(ix);
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.feePayer = payer.publicKey;
  tx.sign(payer, claimerKeypair);

  console.log('📤 Submitting claim transaction...');
  const sig = await connection.sendRawTransaction(tx.serialize());
  console.log(`✅ Signature: ${sig}`);
  console.log(`   Explorer: https://explorer.solana.com/tx/${sig}`);

  console.log('\n⏳ Confirming transaction...');
  const confirmation = await connection.confirmTransaction(sig, 'confirmed');

  if (confirmation.value.err) {
    console.log('\n❌ Claim FAILED:');
    console.log(JSON.stringify(confirmation.value.err, null, 2));
  } else {
    console.log('\n✅ Claim CONFIRMED!');
    console.log('\nVerify:');
    console.log(`  1. Check CCM balance: https://solscan.io/token/${MINT.toBase58()}?owner=${claimerKeypair.publicKey.toBase58()}`);
    console.log(`  2. Check transaction: https://explorer.solana.com/tx/${sig}`);
  }
}

main().catch(console.error);
