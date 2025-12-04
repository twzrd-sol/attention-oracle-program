import * as fs from 'fs';
import { createHash } from 'crypto';
import { Connection, Keypair, PublicKey, Transaction, TransactionInstruction, SystemProgram } from '@solana/web3.js';
import jsSha3 from 'js-sha3';
const { keccak256 } = jsSha3;

const RPC_URL = 'https://api.mainnet-beta.solana.com';
const KEYPAIR_PATH = '/home/twzrd/.config/solana/amm-admin.json';
const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const CCM_MINT = new PublicKey('ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe');
const CHANNEL_NAME = 'pump.fun';

// Discriminator for close_channel_migration: sha256("global:close_channel_migration")[0..8]
// This version uses UncheckedAccount to handle schema size mismatches
const CLOSE_CHANNEL_MIGRATION_DISCRIMINATOR = Buffer.from(
  createHash('sha256').update('global:close_channel_migration').digest().slice(0, 8)
);

function deriveSubjectId(channel: string): Buffer {
  const input = `channel:${channel.toLowerCase()}`;
  return Buffer.from(keccak256(input), 'hex');
}

async function main() {
  const connection = new Connection(RPC_URL, 'confirmed');
  const keypairData = JSON.parse(fs.readFileSync(KEYPAIR_PATH, 'utf-8'));
  const keypair = Keypair.fromSecretKey(Uint8Array.from(keypairData));

  const subjectId = deriveSubjectId(CHANNEL_NAME);
  const [protocolState] = PublicKey.findProgramAddressSync([Buffer.from('protocol'), CCM_MINT.toBuffer()], PROGRAM_ID);
  const [channelState] = PublicKey.findProgramAddressSync([Buffer.from('channel_state'), CCM_MINT.toBuffer(), subjectId], PROGRAM_ID);

  console.log(`Closing channel_state: ${channelState.toBase58()}`);
  console.log(`Rent will be refunded to: ${keypair.publicKey.toBase58()}`);

  // Build close_channel instruction
  // Args: channel (String)
  const channelBytes = Buffer.from(CHANNEL_NAME);
  const dataSize = 8 + 4 + channelBytes.length;
  const data = Buffer.alloc(dataSize);
  let offset = 0;

  CLOSE_CHANNEL_MIGRATION_DISCRIMINATOR.copy(data, offset);
  offset += 8;
  data.writeUInt32LE(channelBytes.length, offset);
  offset += 4;
  channelBytes.copy(data, offset);

  const ix = new TransactionInstruction({
    keys: [
      { pubkey: keypair.publicKey, isSigner: true, isWritable: true }, // admin
      { pubkey: protocolState, isSigner: false, isWritable: false },
      { pubkey: channelState, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    programId: PROGRAM_ID,
    data,
  });

  const tx = new Transaction().add(ix);
  const { blockhash } = await connection.getLatestBlockhash();
  tx.recentBlockhash = blockhash;
  tx.feePayer = keypair.publicKey;

  console.log(`\nSimulating...`);
  const simulation = await connection.simulateTransaction(tx);
  if (simulation.value.err) {
    console.error('Simulation failed:', simulation.value.err);
    console.error('Logs:', simulation.value.logs);
    return;
  }
  console.log('Simulation passed');

  tx.sign(keypair);
  console.log(`\nSending transaction...`);
  const sig = await connection.sendRawTransaction(tx.serialize());
  console.log(`Confirming...`);
  await connection.confirmTransaction(sig, 'confirmed');

  console.log(`\n=== SUCCESS ===`);
  console.log(`https://solscan.io/tx/${sig}`);
}

main().catch(err => {
  console.error('Error:', err.message);
  process.exit(1);
});
