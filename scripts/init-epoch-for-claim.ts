#!/usr/bin/env tsx
/**
 * Initialize epoch state for Claim #0001
 * Calls set_merkle_root_open to create the epoch state PDA with correct merkle root
 */

import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from '@solana/web3.js';
import { keccak_256 } from '@noble/hashes/sha3';
import * as fs from 'fs';
import * as crypto from 'crypto';
import * as bs58 from 'bs58';

// Config via env/flags for devnet use
const PROGRAM_ID = new PublicKey(process.env.PROGRAM_ID || 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const MINT = new PublicKey(process.env.MINT_PUBKEY || 'AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5');
const RPC_URL = process.env.SOLANA_RPC || 'https://api.devnet.solana.com';

const PROTOCOL_SEED = Buffer.from('protocol');
const EPOCH_STATE_SEED = Buffer.from('epoch_state');

async function main() {
  const connection = new Connection(RPC_URL, 'confirmed');
  const payerPath = process.env.KEYPAIR_PATH || `${process.env.HOME}/.config/solana/id.json`;
  const payer = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(payerPath, 'utf-8')))
  );

  console.log('🚀 Initializing Epoch State for Claim #0001');
  console.log('=========================================\n');

  // Parameters
  // Flags: --channel <str> --epoch <num> --root <hex_no_0x> --claim-count <num>
  const args = process.argv.slice(2);
  const getFlag = (n: string) => {
    const i = args.findIndex(a => a === n);
    return i >= 0 ? args[i+1] : undefined;
  };
  const channel = getFlag('--channel') || getFlag('-c') || process.env.CLS_STREAMER_NAME || 'test-cls';
  const epoch = Number(getFlag('--epoch') || getFlag('-e') || process.env.EPOCH_ID || '0');
  if (!Number.isInteger(epoch) || epoch <= 0) throw new Error('Provide --epoch <id>');
  const rootHex = (getFlag('--root') || process.env.MERKLE_ROOT || '').replace(/^0x/i,'');
  if (!/^[0-9a-f]{64}$/i.test(rootHex)) throw new Error('Provide --root <64-hex> (no 0x)');
  const claimCount = Number(getFlag('--claim-count') || process.env.CLAIM_COUNT || '1');

  // Derive streamer key (must match the channel streamer key)
  const channelBytes = Buffer.from(channel.toLowerCase());
  const streamerKeyHash = keccak_256(Buffer.concat([Buffer.from('twitch:'), channelBytes]));
  const streamerKey = new PublicKey(streamerKeyHash);

  // Parse root
  const root = Buffer.from(rootHex, 'hex');
  if (root.length !== 32) throw new Error(`Invalid root length: ${root.length}`);

  // Derive protocol state
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, MINT.toBuffer()],
    PROGRAM_ID
  );

  // Derive epoch state (epoch as little-endian u64)
  const epochBuf = Buffer.alloc(8);
  epochBuf.writeBigUInt64LE(BigInt(epoch), 0);

  const [epochState] = PublicKey.findProgramAddressSync(
    [EPOCH_STATE_SEED, epochBuf, streamerKey.toBuffer(), MINT.toBuffer()],
    PROGRAM_ID
  );

  console.log(`Channel: ${channel}`);
  console.log(`Epoch: ${epoch}`);
  console.log(`Claim Count: ${claimCount}`);
  console.log(`Merkle Root: ${rootHex}`);
  console.log(`Streamer Key: ${streamerKey.toBase58()}`);
  console.log(`Protocol State: ${protocolState.toBase58()}`);
  console.log(`Epoch State: ${epochState.toBase58()}`);
  console.log(`Payer: ${payer.publicKey.toBase58()}\n`);

  // Build instruction data
  // Discriminator for set_merkle_root_open
  const hash = crypto.createHash('sha256').update('global:set_merkle_root_open').digest();
  const discriminator = hash.slice(0, 8);

  // Instruction format: discriminator + root (32) + epoch (8) + claim_count (4) + streamer_key (32)
  // (epochBuf already created above for PDA derivation, reuse it)
  const claimCountBuf = Buffer.alloc(4);
  claimCountBuf.writeUInt32LE(claimCount, 0);

  const data = Buffer.concat([
    discriminator,
    root,
    epochBuf,
    claimCountBuf,
    streamerKey.toBuffer(),
  ]);

  // Manually build instruction (Anchor macro is not available here)
  const ix = new TransactionInstruction({
    keys: [
      { pubkey: payer.publicKey, isSigner: true, isWritable: true },
      { pubkey: protocolState, isSigner: false, isWritable: true },
      { pubkey: epochState, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    programId: PROGRAM_ID,
    data,
  });

  const tx = new Transaction().add(ix);
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.feePayer = payer.publicKey;
  tx.sign(payer);

  console.log('📤 Sending epoch initialization...');
  const sig = await connection.sendRawTransaction(tx.serialize());
  console.log(`✅ Signature: ${sig}`);
  console.log(`   Explorer: https://explorer.solana.com/tx/${sig}`);

  await connection.confirmTransaction(sig, 'confirmed');
  console.log('✅ Epoch state initialized!\n');

  console.log('Claim is now ready. Parameters:');
  console.log(`  wallet: DV879FdRD3LAot5MTfzftrVmMv9WC9giUeYsDnmTSZh1`);
  console.log(`  channelName: claim-0001-test`);
  console.log(`  epochId: 424243`);
  console.log(`  amount: 100000000000`);
  console.log(`  index: 0`);
  console.log(`  proof: []`);
}

main().catch(console.error);
