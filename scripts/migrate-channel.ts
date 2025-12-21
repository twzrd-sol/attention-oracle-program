/**
 * Migrate ChannelState account to new size (728 → 5688 bytes)
 * Required after CHANNEL_MAX_CLAIMS upgrade from 1024 to 4096
 *
 * Usage: npx ts-node scripts/migrate-channel.ts [channel_name]
 */

import { Connection, Keypair, PublicKey, SystemProgram, Transaction, TransactionInstruction, sendAndConfirmTransaction, ComputeBudgetProgram } from '@solana/web3.js';
import * as fs from 'fs';
import { createHash } from 'crypto';
import jsSha3 from 'js-sha3';
import { requireScriptEnv } from './script-guard.js';
const { keccak256 } = jsSha3;

const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const CCM_MINT = new PublicKey('ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe');
const CHANNEL_STATE_SEED = Buffer.from('channel_state');
const PROTOCOL_SEED = Buffer.from('protocol');

// Derive subject_id from channel name (matching Rust logic - uses keccak256)
function deriveSubjectId(channel: string): PublicKey {
  const lower = channel.toLowerCase();
  const hashHex = keccak256('channel:' + lower);
  return new PublicKey(Buffer.from(hashHex, 'hex'));
}

// Build migrate_channel_state instruction
function buildMigrateInstruction(
  payer: PublicKey,
  protocolState: PublicKey,
  channelState: PublicKey,
  channel: string
): TransactionInstruction {
  // Discriminator for migrate_channel_state (Anchor IDL hash)
  // sha256("global:migrate_channel_state")[0..8]
  const discriminator = Buffer.from([0x9f, 0x5a, 0x3e, 0x8b, 0x12, 0x4c, 0x7d, 0x01]); // Placeholder - will calculate

  // Actually compute the discriminator
  const preimage = 'global:migrate_channel_state';
  const hash = createHash('sha256').update(preimage).digest();
  const disc = hash.slice(0, 8);

  // Encode channel string (length prefix + bytes)
  const channelBytes = Buffer.from(channel, 'utf8');
  const channelLen = Buffer.alloc(4);
  channelLen.writeUInt32LE(channelBytes.length);

  const data = Buffer.concat([disc, channelLen, channelBytes]);

  return new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: protocolState, isSigner: false, isWritable: false },
      { pubkey: channelState, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });
}

async function main() {
  const channel = process.argv[2] || 'pumpfun_attention';
  console.log(`Migrating channel: ${channel}`);

  const { rpcUrl, keypairPath } = requireScriptEnv();

  // Load keypair
  const resolvedKeypair = keypairPath;
  const keypair = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(resolvedKeypair, 'utf8')))
  );
  console.log(`Payer: ${keypair.publicKey.toString()}`);

  const connection = new Connection(rpcUrl, 'confirmed');

  // Derive PDAs
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, CCM_MINT.toBuffer()],
    PROGRAM_ID
  );
  console.log(`Protocol State: ${protocolState.toString()}`);

  const subjectId = deriveSubjectId(channel);
  console.log(`Subject ID: ${subjectId.toString()}`);

  const [channelState] = PublicKey.findProgramAddressSync(
    [CHANNEL_STATE_SEED, CCM_MINT.toBuffer(), subjectId.toBuffer()],
    PROGRAM_ID
  );
  console.log(`Channel State: ${channelState.toString()}`);

  // Check current account size
  const accountInfo = await connection.getAccountInfo(channelState);
  if (!accountInfo) {
    console.error('Channel state account not found!');
    process.exit(1);
  }
  console.log(`Current account size: ${accountInfo.data.length} bytes`);

  if (accountInfo.data.length >= 5688) {
    console.log('Account already at target size. No migration needed.');
    process.exit(0);
  }

  // Build transaction
  const ix = buildMigrateInstruction(keypair.publicKey, protocolState, channelState, channel);

  const tx = new Transaction()
    .add(ComputeBudgetProgram.setComputeUnitLimit({ units: 200_000 }))
    .add(ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 10_000 }))
    .add(ix);

  console.log('Sending migration transaction...');

  try {
    const sig = await sendAndConfirmTransaction(connection, tx, [keypair], {
      skipPreflight: false,
      commitment: 'confirmed',
    });
    console.log(`Migration successful! Signature: ${sig}`);

    // Verify new size
    const newInfo = await connection.getAccountInfo(channelState);
    console.log(`New account size: ${newInfo?.data.length} bytes`);
  } catch (err: any) {
    console.error('Migration failed:', err.message);
    if (err.logs) {
      console.error('Logs:', err.logs);
    }
    process.exit(1);
  }
}

main().catch(console.error);
