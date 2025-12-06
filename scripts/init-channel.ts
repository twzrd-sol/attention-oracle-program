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

// Discriminator for initialize_channel: sha256("global:initialize_channel")[0..8]
const INIT_CHANNEL_DISCRIMINATOR = Buffer.from(
  createHash('sha256').update('global:initialize_channel').digest().slice(0, 8)
);

function deriveSubjectId(channel: string): PublicKey {
  const input = `channel:${channel.toLowerCase()}`;
  const hash = keccak256(input);
  return new PublicKey(Buffer.from(hash, 'hex'));
}

async function main() {
  const connection = new Connection(RPC_URL, 'confirmed');
  const keypairData = JSON.parse(fs.readFileSync(KEYPAIR_PATH, 'utf-8'));
  const keypair = Keypair.fromSecretKey(Uint8Array.from(keypairData));

  const subjectId = deriveSubjectId(CHANNEL_NAME);
  const [protocolState] = PublicKey.findProgramAddressSync([Buffer.from('protocol'), CCM_MINT.toBuffer()], PROGRAM_ID);
  const [channelState] = PublicKey.findProgramAddressSync([Buffer.from('channel_state'), CCM_MINT.toBuffer(), subjectId.toBuffer()], PROGRAM_ID);

  console.log(`Initializing channel: ${CHANNEL_NAME}`);
  console.log(`Subject ID: ${subjectId.toBase58()}`);
  console.log(`Protocol state: ${protocolState.toBase58()}`);
  console.log(`Channel state: ${channelState.toBase58()}`);
  console.log(`Payer: ${keypair.publicKey.toBase58()}`);

  // Build instruction data: discriminator (8) + subject_id (32)
  const data = Buffer.concat([
    INIT_CHANNEL_DISCRIMINATOR,
    subjectId.toBuffer()
  ]);

  const ix = new TransactionInstruction({
    keys: [
      { pubkey: keypair.publicKey, isSigner: true, isWritable: true }, // payer
      { pubkey: protocolState, isSigner: false, isWritable: false }, // protocol_state
      { pubkey: channelState, isSigner: false, isWritable: true }, // channel_state (will be created)
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
  console.log('Logs:', simulation.value.logs?.slice(-5));

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
