/**
 * Resize ChannelState account to match the current CHANNEL_RING_SLOTS.
 *
 * Required after increasing the on-chain ring buffer (e.g., 10 → 2016).
 *
 * Usage:
 *   npx ts-node scripts/resize-channel-state.ts [channel_name]
 */

import {
  ComputeBudgetProgram,
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
} from '@solana/web3.js';
import * as fs from 'fs';
import { createHash } from 'crypto';
import jsSha3 from 'js-sha3';
import { requireScriptEnv } from './script-guard.js';

const { keccak256 } = jsSha3;

const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const CCM_MINT = new PublicKey('ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe');

const CHANNEL_STATE_SEED = Buffer.from('channel_state');
const PROTOCOL_SEED = Buffer.from('protocol');

// Keep in sync with programs/token_2022/src/constants.rs
const CHANNEL_RING_SLOTS = 2048; // ~7.1 days @ 5-min epochs
const CHANNEL_MAX_CLAIMS = 4096;
const CHANNEL_BITMAP_BYTES = (CHANNEL_MAX_CLAIMS + 7) / 8; // 512
const SLOT_SIZE = 8 + 32 + 2 + 6 + CHANNEL_BITMAP_BYTES; // 560
const HEADER_BYTES = 8 + 1 + 1 + 32 + 32 + 6 + 8; // 88
const TARGET_SIZE = HEADER_BYTES + CHANNEL_RING_SLOTS * SLOT_SIZE; // 1_146_968

function deriveSubjectId(channel: string): PublicKey {
  const lower = channel.toLowerCase();
  const hashHex = keccak256('channel:' + lower);
  return new PublicKey(Buffer.from(hashHex, 'hex'));
}

function discriminator(name: string): Buffer {
  // sha256("global:<name>")[0..8]
  return createHash('sha256').update(`global:${name}`).digest().subarray(0, 8);
}

function buildResizeInstruction(payer: PublicKey, protocolState: PublicKey, channelState: PublicKey) {
  return new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: protocolState, isSigner: false, isWritable: false },
      { pubkey: channelState, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: discriminator('resize_channel_state'),
  });
}

async function main() {
  const channel = process.argv[2] || 'youtube_lofi';
  console.log(`Resizing channel state for: ${channel}`);

  const { rpcUrl, keypairPath } = requireScriptEnv();

  const resolvedKeypair = keypairPath;
  const keypair = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(resolvedKeypair, 'utf8'))),
  );
  console.log(`Payer: ${keypair.publicKey.toBase58()}`);

  const connection = new Connection(rpcUrl, 'confirmed');
  console.log(`RPC: ${rpcUrl}`);

  const [protocolState] = PublicKey.findProgramAddressSync([PROTOCOL_SEED, CCM_MINT.toBuffer()], PROGRAM_ID);
  console.log(`Protocol State: ${protocolState.toBase58()}`);

  const subjectId = deriveSubjectId(channel);
  console.log(`Subject ID: ${subjectId.toBase58()}`);

  const [channelState] = PublicKey.findProgramAddressSync(
    [CHANNEL_STATE_SEED, CCM_MINT.toBuffer(), subjectId.toBuffer()],
    PROGRAM_ID,
  );
  console.log(`Channel State: ${channelState.toBase58()}`);

  let info = await connection.getAccountInfo(channelState, 'confirmed');
  if (!info) {
    console.error('Channel state account not found!');
    process.exit(1);
  }

  console.log(`Current size: ${info.data.length} bytes`);
  console.log(`Target size:  ${TARGET_SIZE} bytes`);

  if (info.data.length >= TARGET_SIZE) {
    console.log('Already resized. Nothing to do.');
    return;
  }

  // Chunked resize loop - Solana limits realloc to 10KB per instruction
  const MAX_REALLOC_DELTA = 10240;
  let iteration = 0;

  while (info.data.length < TARGET_SIZE) {
    iteration++;
    const remaining = TARGET_SIZE - info.data.length;
    const iterationsLeft = Math.ceil(remaining / MAX_REALLOC_DELTA);
    console.log(`\n=== Iteration ${iteration} (${iterationsLeft} remaining) ===`);
    console.log(`Current: ${info.data.length} bytes → Target: ${TARGET_SIZE} bytes`);

    const ix = buildResizeInstruction(keypair.publicKey, protocolState, channelState);

    const tx = new Transaction()
      .add(ComputeBudgetProgram.setComputeUnitLimit({ units: 1_200_000 }))
      .add(ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 10_000 }))
      .add(ix);

    console.log('Sending resize transaction...');
    const sig = await sendAndConfirmTransaction(connection, tx, [keypair], {
      skipPreflight: false,
      commitment: 'confirmed',
    });
    console.log(`Resize sent. Signature: ${sig}`);

    // Refresh account info for next iteration
    info = await connection.getAccountInfo(channelState, 'confirmed');
    if (!info) {
      console.error('Channel state account disappeared!');
      process.exit(1);
    }
    console.log(`New size: ${info.data.length} bytes`);

    // Delay to avoid rate limiting on public RPC
    await new Promise(resolve => setTimeout(resolve, 2000));
  }

  console.log(`\n✓ Resize complete! Final size: ${info.data.length} bytes`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
