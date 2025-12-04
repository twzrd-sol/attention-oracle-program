import * as fs from 'fs';
import { Connection, Keypair, PublicKey, Transaction, TransactionInstruction, SystemProgram } from '@solana/web3.js';
import jsSha3 from 'js-sha3';
const { keccak256 } = jsSha3;

const EPOCH = parseInt(process.argv[2] || '5882454');
const RPC_URL = 'https://api.mainnet-beta.solana.com';
const KEYPAIR_PATH = '/home/twzrd/.config/solana/amm-admin.json';
const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const CCM_MINT = new PublicKey('ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe');
const CHANNEL_NAME = 'pump.fun';

// Discriminator for set_channel_merkle_root: sha256("global:set_channel_merkle_root")[0..8]
// You can compute this with: Buffer.from(require('crypto').createHash('sha256').update('global:set_channel_merkle_root').digest().slice(0, 8))
const SET_CHANNEL_ROOT_DISCRIMINATOR = Buffer.from([0x41, 0x18, 0x10, 0x06, 0x3f, 0x69, 0x99, 0x7b]);

const claimsFile = `/tmp/claims-${EPOCH}.json`;
const claimsData = JSON.parse(fs.readFileSync(claimsFile, 'utf-8'));
const claims = claimsData.claims;

function deriveSubjectId(channel: string): Buffer {
  const input = `channel:${channel.toLowerCase()}`;
  return Buffer.from(keccak256(input), 'hex');
}

function buildMerkleRoot(claims: Record<string, string>): Buffer {
  const entries = Object.entries(claims).sort((a, b) => a[0].localeCompare(b[0]));
  if (entries.length === 0) return Buffer.alloc(32);

  const leaves: Buffer[] = entries.map(([wallet, amount], index) => {
    const claimer = new PublicKey(wallet);
    const indexBuf = Buffer.alloc(4);
    indexBuf.writeUInt32LE(index);
    const amountBuf = Buffer.alloc(8);
    amountBuf.writeBigUInt64LE(BigInt(amount));
    const data = Buffer.concat([claimer.toBuffer(), indexBuf, amountBuf, Buffer.from(wallet)]);
    return Buffer.from(keccak256(data), 'hex');
  });

  while (leaves.length > 1) {
    const newLevel: Buffer[] = [];
    for (let i = 0; i < leaves.length; i += 2) {
      if (i + 1 < leaves.length) {
        const combined = Buffer.concat([leaves[i], leaves[i + 1]].sort(Buffer.compare));
        newLevel.push(Buffer.from(keccak256(combined), 'hex'));
      } else {
        newLevel.push(leaves[i]);
      }
    }
    leaves.length = 0;
    leaves.push(...newLevel);
  }
  return leaves[0];
}

async function main() {
  const connection = new Connection(RPC_URL, 'confirmed');
  const keypairData = JSON.parse(fs.readFileSync(KEYPAIR_PATH, 'utf-8'));
  const keypair = Keypair.fromSecretKey(Uint8Array.from(keypairData));

  const subjectId = deriveSubjectId(CHANNEL_NAME);
  const [protocolState] = PublicKey.findProgramAddressSync([Buffer.from('protocol'), CCM_MINT.toBuffer()], PROGRAM_ID);
  const [channelState] = PublicKey.findProgramAddressSync([Buffer.from('channel_state'), CCM_MINT.toBuffer(), subjectId], PROGRAM_ID);

  const merkleRoot = buildMerkleRoot(claims);
  console.log(`Epoch: ${EPOCH}`);
  console.log(`Users: ${Object.keys(claims).length}`);
  console.log(`Merkle root: ${merkleRoot.toString('hex')}`);
  console.log(`Channel state: ${channelState.toBase58()}`);
  console.log(`Payer (admin): ${keypair.publicKey.toBase58()}`);

  // Build instruction data:
  // discriminator (8) + channel string (4 + len) + epoch (8) + root (32)
  const channelBytes = Buffer.from(CHANNEL_NAME);
  const dataSize = 8 + 4 + channelBytes.length + 8 + 32;
  const data = Buffer.alloc(dataSize);
  let offset = 0;

  // Discriminator
  SET_CHANNEL_ROOT_DISCRIMINATOR.copy(data, offset);
  offset += 8;

  // Channel string (length-prefixed)
  data.writeUInt32LE(channelBytes.length, offset);
  offset += 4;
  channelBytes.copy(data, offset);
  offset += channelBytes.length;

  // Epoch (u64)
  data.writeBigUInt64LE(BigInt(EPOCH), offset);
  offset += 8;

  // Root ([u8; 32])
  merkleRoot.copy(data, offset);

  const ix = new TransactionInstruction({
    keys: [
      { pubkey: keypair.publicKey, isSigner: true, isWritable: true },
      { pubkey: protocolState, isSigner: false, isWritable: true },
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

  console.log(`\nSimulating transaction...`);
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
