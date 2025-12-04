import * as fs from 'fs';
import { createHash } from 'crypto';
import { Connection, Keypair, PublicKey, Transaction, TransactionInstruction, SystemProgram } from '@solana/web3.js';
import jsSha3 from 'js-sha3';
const { keccak256 } = jsSha3;

const EPOCH = parseInt(process.argv[2] || '5882454');
const TEST_WALLET = process.argv[3] || '14ShSFxffRwoeyPmgS2tqUTS6A7WhmP6edpn7Y2329Gj';

const RPC_URL = 'https://api.mainnet-beta.solana.com';
const KEYPAIR_PATH = '/home/twzrd/.config/solana/amm-admin.json';
const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const CCM_MINT = new PublicKey('ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe');
const CHANNEL_NAME = 'pump.fun';

// Discriminator for require_attention_ge: sha256("global:require_attention_ge")[0..8]
const REQUIRE_ATTENTION_GE_DISCRIMINATOR = Buffer.from(
  createHash('sha256').update('global:require_attention_ge').digest().slice(0, 8)
);

function deriveSubjectId(channel: string): PublicKey {
  const input = `channel:${channel.toLowerCase()}`;
  const hash = keccak256(input);
  return new PublicKey(Buffer.from(hash, 'hex'));
}

interface MerkleResult {
  root: Buffer;
  proof: Buffer[];
  index: number;
  amount: bigint;
}

function buildMerkleTreeWithProof(claims: Record<string, string>, targetWallet: string): MerkleResult {
  const entries = Object.entries(claims).sort((a, b) => a[0].localeCompare(b[0]));
  if (entries.length === 0) throw new Error('No claims');

  // Find target index
  const targetIndex = entries.findIndex(([wallet]) => wallet === targetWallet);
  if (targetIndex < 0) throw new Error(`Wallet ${targetWallet} not found in claims`);
  const targetAmount = BigInt(entries[targetIndex][1]);

  // Build leaves
  const leaves: Buffer[] = entries.map(([wallet, amount], index) => {
    const claimer = new PublicKey(wallet);
    const indexBuf = Buffer.alloc(4);
    indexBuf.writeUInt32LE(index);
    const amountBuf = Buffer.alloc(8);
    amountBuf.writeBigUInt64LE(BigInt(amount));
    const data = Buffer.concat([claimer.toBuffer(), indexBuf, amountBuf, Buffer.from(wallet)]);
    return Buffer.from(keccak256(data), 'hex');
  });

  // Build tree and collect proof
  const proof: Buffer[] = [];
  let currentIndex = targetIndex;
  let level = [...leaves];

  while (level.length > 1) {
    const newLevel: Buffer[] = [];
    for (let i = 0; i < level.length; i += 2) {
      if (i + 1 < level.length) {
        // Record sibling in proof if on our path
        if (Math.floor(currentIndex / 2) * 2 === i) {
          const siblingIdx = (currentIndex % 2 === 0) ? i + 1 : i;
          proof.push(level[siblingIdx]);
        }
        const combined = Buffer.concat([level[i], level[i + 1]].sort(Buffer.compare));
        newLevel.push(Buffer.from(keccak256(combined), 'hex'));
      } else {
        // Odd element gets promoted
        if (i === Math.floor(currentIndex / 2) * 2) {
          // Our node is the odd one, no sibling to add
        }
        newLevel.push(level[i]);
      }
    }
    currentIndex = Math.floor(currentIndex / 2);
    level = newLevel;
  }

  return {
    root: level[0],
    proof,
    index: targetIndex,
    amount: targetAmount
  };
}

async function main() {
  const connection = new Connection(RPC_URL, 'confirmed');
  const keypairData = JSON.parse(fs.readFileSync(KEYPAIR_PATH, 'utf-8'));
  const keypair = Keypair.fromSecretKey(Uint8Array.from(keypairData));

  const claimsFile = `/tmp/claims-${EPOCH}.json`;
  const claimsData = JSON.parse(fs.readFileSync(claimsFile, 'utf-8'));
  const claims = claimsData.claims;

  const subjectId = deriveSubjectId(CHANNEL_NAME);
  const [protocolState] = PublicKey.findProgramAddressSync([Buffer.from('protocol'), CCM_MINT.toBuffer()], PROGRAM_ID);
  const [channelState] = PublicKey.findProgramAddressSync([Buffer.from('channel_state'), CCM_MINT.toBuffer(), subjectId.toBuffer()], PROGRAM_ID);

  console.log(`Testing gate for wallet: ${TEST_WALLET}`);
  console.log(`Epoch: ${EPOCH}`);
  console.log(`Channel: ${CHANNEL_NAME}`);

  // Build merkle proof
  const { root, proof, index, amount } = buildMerkleTreeWithProof(claims, TEST_WALLET);
  console.log(`\nMerkle proof:`);
  console.log(`  Index: ${index}`);
  console.log(`  Amount: ${amount} (${Number(amount) / 1e9} CCM)`);
  console.log(`  Root: ${root.toString('hex')}`);
  console.log(`  Proof length: ${proof.length} nodes`);

  const minAttention = 100n * 1_000_000_000n; // 100 CCM threshold
  console.log(`\nMin attention threshold: ${minAttention} (${Number(minAttention) / 1e9} CCM)`);

  // Build instruction data
  // Args: channel (String), epoch (u64), index (u32), amount (u64), id (String), proof (Vec<[u8;32]>), min_attention (u64)
  const channelBytes = Buffer.from(CHANNEL_NAME);
  const idBytes = Buffer.from(TEST_WALLET);

  // Calculate size
  const dataSize = 8 + // discriminator
    4 + channelBytes.length + // channel string
    8 + // epoch
    4 + // index
    8 + // amount
    4 + idBytes.length + // id string
    4 + (proof.length * 32) + // proof vector
    8; // min_attention

  const data = Buffer.alloc(dataSize);
  let offset = 0;

  // Discriminator
  REQUIRE_ATTENTION_GE_DISCRIMINATOR.copy(data, offset);
  offset += 8;

  // Channel string
  data.writeUInt32LE(channelBytes.length, offset);
  offset += 4;
  channelBytes.copy(data, offset);
  offset += channelBytes.length;

  // Epoch (u64)
  data.writeBigUInt64LE(BigInt(EPOCH), offset);
  offset += 8;

  // Index (u32)
  data.writeUInt32LE(index, offset);
  offset += 4;

  // Amount (u64)
  data.writeBigUInt64LE(amount, offset);
  offset += 8;

  // ID string
  data.writeUInt32LE(idBytes.length, offset);
  offset += 4;
  idBytes.copy(data, offset);
  offset += idBytes.length;

  // Proof vector
  data.writeUInt32LE(proof.length, offset);
  offset += 4;
  for (const node of proof) {
    node.copy(data, offset);
    offset += 32;
  }

  // Min attention (u64)
  data.writeBigUInt64LE(minAttention, offset);

  const claimer = new PublicKey(TEST_WALLET);

  const ix = new TransactionInstruction({
    keys: [
      { pubkey: claimer, isSigner: false, isWritable: false }, // owner
      { pubkey: CCM_MINT, isSigner: false, isWritable: false }, // mint
      { pubkey: channelState, isSigner: false, isWritable: false }, // channel_state (read-only)
    ],
    programId: PROGRAM_ID,
    data,
  });

  const tx = new Transaction().add(ix);
  const { blockhash } = await connection.getLatestBlockhash();
  tx.recentBlockhash = blockhash;
  tx.feePayer = keypair.publicKey;

  console.log(`\nSimulating require_attention_ge...`);
  const simulation = await connection.simulateTransaction(tx);

  if (simulation.value.err) {
    console.error('\n=== GATE FAILED ===');
    console.error('Error:', simulation.value.err);
    console.error('Logs:', simulation.value.logs);
  } else {
    console.log('\n=== GATE PASSED ===');
    console.log('Logs:', simulation.value.logs?.slice(-5));
    console.log(`\nUser ${TEST_WALLET} has ${Number(amount) / 1e9} CCM attention >= ${Number(minAttention) / 1e9} CCM threshold`);
  }
}

main().catch(err => {
  console.error('Error:', err.message);
  process.exit(1);
});
