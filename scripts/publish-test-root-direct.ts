#!/usr/bin/env tsx
/**
 * Direct root publication (bypasses L2 verification for testing)
 */

import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from '@solana/web3.js';
import { keccak_256 } from '@noble/hashes/sha3.js';
import * as fs from 'fs';
import * as crypto from 'crypto';

const PROGRAM_ID = new PublicKey(process.env.PROGRAM_ID || 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop')
const MINT = new PublicKey(process.env.MINT_PUBKEY || 'AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5')
const RPC_URL = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com'

const PROTOCOL_SEED = Buffer.from('protocol');
const CHANNEL_STATE_SEED = Buffer.from('channel_state');

// CLI args
const [channel, epochStr, rootHex] = process.argv.slice(2);

if (!channel || !epochStr || !rootHex) {
  console.error('Usage: publish-test-root-direct.ts <channel> <epoch> <root_hex>');
  process.exit(1);
}

const epoch = parseInt(epochStr);

async function main() {
  const connection = new Connection(RPC_URL, 'confirmed');
  const payerPath = process.env.KEYPAIR_PATH || `${process.env.HOME}/.config/solana/id.json`;
  const payer = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(payerPath, 'utf-8')))
  );

  console.log(`🚀 Publishing Test Root (Direct)`)
  console.log(`   Channel: ${channel}`)
  console.log(`   Epoch: ${epoch}`)
  console.log(`   Root: ${rootHex}`)
  console.log(`   Payer: ${payer.publicKey.toBase58()}`)

  // Derive addresses
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, MINT.toBuffer()],
    PROGRAM_ID
  );

  // Derive streamer key from channel
  const channelBytes = Buffer.from(channel.toLowerCase());
  const keccakHash = keccak_256(Buffer.concat([Buffer.from('channel:'), channelBytes]));
  const streamerKey = new PublicKey(keccakHash);

  const [channelState] = PublicKey.findProgramAddressSync(
    [CHANNEL_STATE_SEED, MINT.toBuffer(), streamerKey.toBuffer()],
    PROGRAM_ID
  );

  console.log(`   Protocol State: ${protocolState.toBase58()}`);
  console.log(`   Channel State: ${channelState.toBase58()}`);
  console.log(`   Streamer Key: ${streamerKey.toBase58()}`);

  // Parse root
  const rootClean = rootHex.startsWith('0x') ? rootHex.slice(2) : rootHex;
  const root = Buffer.from(rootClean, 'hex');
  if (root.length !== 32) {
    throw new Error(`Invalid root length: ${root.length} (expected 32)`);
  }

  // Build instruction data
  const hash = crypto.createHash('sha256').update('global:set_channel_merkle_root').digest();
  const discriminator = hash.slice(0, 8);

  const channelLen = Buffer.alloc(4);
  channelLen.writeUInt32LE(channel.length, 0);
  const epochBuf = Buffer.alloc(8);
  epochBuf.writeBigUInt64LE(BigInt(epoch), 0);

  const data = Buffer.concat([discriminator, channelLen, Buffer.from(channel), epochBuf, root]);

  const ix = new TransactionInstruction({
    keys: [
      { pubkey: payer.publicKey, isSigner: true, isWritable: true },
      { pubkey: protocolState, isSigner: false, isWritable: true },
      { pubkey: channelState, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    programId: PROGRAM_ID,
    data,
  });

  const tx = new Transaction().add(ix);
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.feePayer = payer.publicKey;
  tx.sign(payer);

  console.log('\n📤 Sending transaction...');
  const sig = await connection.sendRawTransaction(tx.serialize());
  console.log(`✅ Signature: ${sig}`);
  console.log(`   Explorer: https://explorer.solana.com/tx/${sig}`);

  await connection.confirmTransaction(sig, 'confirmed');
  console.log('✅ Root published!');
}

main().catch(console.error);
