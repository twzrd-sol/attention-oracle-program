#!/usr/bin/env npx ts-node
/**
 * Attention-Gated Pump.fun Buy
 *
 * Usage:
 *   npx ts-node scripts/gated-buy.ts <token-mint> <sol-amount> [min-attention]
 */

import * as fs from 'fs';
import { execSync } from 'child_process';
import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
} from '@solana/web3.js';
import jsSha3 from 'js-sha3';
const { keccak256 } = jsSha3;

// Config
const RPC_URL = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com';
const KEYPAIR_PATH = process.env.KEYPAIR_PATH || '/home/twzrd/.config/solana/id.json';
const BUCKET = 'twzrd-claims';
const ACCOUNT_ID = 'eeeb7e9257decbce1ca1ac221be82e7a';
const CF_TOKEN = process.env.CLOUDFLARE_API_TOKEN || '';

// Program constants
const ATTENTION_ORACLE_PROGRAM = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const CCM_MINT = new PublicKey('ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe');
const CHANNEL_NAME = 'pump.fun';
const CHANNEL_STATE_SEED = Buffer.from('channel_state');

const REQUIRE_ATTENTION_GE_DISCRIMINATOR = Buffer.from([
  0x78, 0x6d, 0xba, 0x18, 0xb5, 0x34, 0x46, 0x91
]);

interface ClaimsData {
  epoch: number;
  claims: Record<string, string>;
  merkleProofs?: Record<string, { index: number; proof: string[] }>;
}

interface AttentionProof {
  epoch: number;
  index: number;
  amount: bigint;
  id: string;
  proof: Buffer[];
}

function deriveSubjectId(channel: string): PublicKey {
  const input = `channel:${channel.toLowerCase()}`;
  const hash = keccak256(input);
  return new PublicKey(Buffer.from(hash, 'hex'));
}

function deriveChannelState(mint: PublicKey, subjectId: PublicKey): PublicKey {
  const [pda] = PublicKey.findProgramAddressSync(
    [CHANNEL_STATE_SEED, mint.toBuffer(), subjectId.toBuffer()],
    ATTENTION_ORACLE_PROGRAM
  );
  return pda;
}

async function fetchLatestEpoch(): Promise<number> {
  return Math.floor(Date.now() / 300000) - 1;
}

async function fetchClaimsData(epoch: number): Promise<ClaimsData | null> {
  try {
    const tmpFile = `/tmp/claims-${epoch}.json`;
    execSync(
      `CLOUDFLARE_API_TOKEN='${CF_TOKEN}' npx wrangler r2 object get ${BUCKET}/claims-epoch-${epoch}.json --file=${tmpFile} --remote 2>/dev/null`,
      { stdio: 'pipe' }
    );
    return JSON.parse(fs.readFileSync(tmpFile, 'utf-8'));
  } catch {
    return null;
  }
}

function computeMerkleLeaf(claimer: PublicKey, index: number, amount: bigint, id: string): Buffer {
  const indexBuf = Buffer.alloc(4);
  indexBuf.writeUInt32LE(index);
  const amountBuf = Buffer.alloc(8);
  amountBuf.writeBigUInt64LE(amount);
  const data = Buffer.concat([claimer.toBuffer(), indexBuf, amountBuf, Buffer.from(id)]);
  return Buffer.from(keccak256(data), 'hex');
}

function computeMerkleRoot(claims: Record<string, string>): { root: Buffer; proofs: Record<string, { index: number; proof: Buffer[] }> } {
  const entries = Object.entries(claims).sort((a, b) => a[0].localeCompare(b[0]));
  const proofs: Record<string, { index: number; proof: Buffer[] }> = {};

  if (entries.length === 0) {
    return { root: Buffer.alloc(32), proofs };
  }

  const leaves: Buffer[] = entries.map(([wallet, amount], index) => {
    const claimer = new PublicKey(wallet);
    return computeMerkleLeaf(claimer, index, BigInt(amount), wallet);
  });

  entries.forEach(([wallet], index) => {
    proofs[wallet] = { index, proof: [] };
  });

  let currentLevel = leaves.slice();
  while (currentLevel.length > 1) {
    const newLevel: Buffer[] = [];
    for (let i = 0; i < currentLevel.length; i += 2) {
      if (i + 1 < currentLevel.length) {
        const [left, right] = [currentLevel[i], currentLevel[i + 1]].sort(Buffer.compare);
        const combined = Buffer.concat([left, right]);
        newLevel.push(Buffer.from(keccak256(combined), 'hex'));

        entries.forEach(([wallet], origIdx) => {
          const levelIdx = Math.floor(origIdx / Math.pow(2, Math.log2(leaves.length / currentLevel.length)));
          if (levelIdx === i && i + 1 < currentLevel.length) {
            proofs[wallet].proof.push(currentLevel[i + 1]);
          } else if (levelIdx === i + 1) {
            proofs[wallet].proof.push(currentLevel[i]);
          }
        });
      } else {
        newLevel.push(currentLevel[i]);
      }
    }
    currentLevel = newLevel;
  }

  return { root: currentLevel[0], proofs };
}

async function getAttentionProof(
  wallet: PublicKey,
  epoch?: number
): Promise<AttentionProof | null> {
  const targetEpoch = epoch ?? await fetchLatestEpoch();
  const claimsData = await fetchClaimsData(targetEpoch);

  if (!claimsData) {
    console.log(`No claims data for epoch ${targetEpoch}`);
    return null;
  }

  const walletStr = wallet.toBase58();
  const amount = claimsData.claims[walletStr];

  if (!amount) {
    console.log(`Wallet ${walletStr} has no attention in epoch ${targetEpoch}`);
    return null;
  }

  const { proofs } = computeMerkleRoot(claimsData.claims);
  const walletProof = proofs[walletStr];

  if (!walletProof) {
    console.log(`Could not generate proof for wallet ${walletStr}`);
    return null;
  }

  return {
    epoch: targetEpoch,
    index: walletProof.index,
    amount: BigInt(amount),
    id: walletStr,
    proof: walletProof.proof,
  };
}

function buildRequireAttentionInstruction(
  owner: PublicKey,
  mint: PublicKey,
  channelState: PublicKey,
  channel: string,
  proof: AttentionProof,
  minAttention: bigint
): TransactionInstruction {
  const channelBytes = Buffer.from(channel);
  const idBytes = Buffer.from(proof.id);

  const dataSize =
    8 +
    4 + channelBytes.length +
    8 +
    4 +
    8 +
    4 + idBytes.length +
    4 + (proof.proof.length * 32) +
    8;

  const data = Buffer.alloc(dataSize);
  let offset = 0;

  REQUIRE_ATTENTION_GE_DISCRIMINATOR.copy(data, offset);
  offset += 8;

  data.writeUInt32LE(channelBytes.length, offset);
  offset += 4;
  channelBytes.copy(data, offset);
  offset += channelBytes.length;

  data.writeBigUInt64LE(BigInt(proof.epoch), offset);
  offset += 8;

  data.writeUInt32LE(proof.index, offset);
  offset += 4;

  data.writeBigUInt64LE(proof.amount, offset);
  offset += 8;

  data.writeUInt32LE(idBytes.length, offset);
  offset += 4;
  idBytes.copy(data, offset);
  offset += idBytes.length;

  data.writeUInt32LE(proof.proof.length, offset);
  offset += 4;
  for (const node of proof.proof) {
    node.copy(data, offset);
    offset += 32;
  }

  data.writeBigUInt64LE(minAttention, offset);

  return new TransactionInstruction({
    keys: [
      { pubkey: owner, isSigner: false, isWritable: false },
      { pubkey: mint, isSigner: false, isWritable: false },
      { pubkey: channelState, isSigner: false, isWritable: false },
    ],
    programId: ATTENTION_ORACLE_PROGRAM,
    data,
  });
}

async function gatedBuy(
  tokenMint: PublicKey,
  solAmount: number,
  minAttention: bigint = 1_000_000n
) {
  const connection = new Connection(RPC_URL, 'confirmed');
  const keypairData = JSON.parse(fs.readFileSync(KEYPAIR_PATH, 'utf-8'));
  const keypair = Keypair.fromSecretKey(Uint8Array.from(keypairData));

  console.log(`\n=== Attention-Gated Pump.fun Buy ===`);
  console.log(`Wallet: ${keypair.publicKey.toBase58()}`);
  console.log(`Token: ${tokenMint.toBase58()}`);
  console.log(`Amount: ${solAmount} SOL`);
  console.log(`Min Attention: ${minAttention.toString()} micro-tokens`);
  console.log(`\nFetching attention proof...`);
  const proof = await getAttentionProof(keypair.publicKey);

  if (!proof) {
    console.error('No attention proof found. Cannot proceed with gated buy.');
    process.exit(1);
  }

  console.log(`Found attention: ${proof.amount.toString()} in epoch ${proof.epoch}`);

  if (proof.amount < minAttention) {
    console.error(`Insufficient attention: ${proof.amount} < ${minAttention}`);
    process.exit(1);
  }

  const subjectId = deriveSubjectId(CHANNEL_NAME);
  const channelState = deriveChannelState(CCM_MINT, subjectId);

  console.log(`\nBuilding attention gate instruction...`);
  const gateIx = buildRequireAttentionInstruction(
    keypair.publicKey,
    CCM_MINT,
    channelState,
    CHANNEL_NAME,
    proof,
    minAttention
  );

  console.log(`\nBuilding pump.fun buy instruction...`);

  const tx = new Transaction().add(gateIx);

  const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash();
  tx.recentBlockhash = blockhash;
  tx.feePayer = keypair.publicKey;
  tx.sign(keypair);

  console.log(`\nSimulating transaction...`);
  try {
    const simulation = await connection.simulateTransaction(tx);
    if (simulation.value.err) {
      console.error('Simulation failed:', simulation.value.err);
      console.error('Logs:', simulation.value.logs);
      process.exit(1);
    }
    console.log('Simulation passed!');
    console.log('Logs:', simulation.value.logs);
  } catch (err) {
    console.error('Simulation error:', (err as Error).message);
    process.exit(1);
  }

  console.log(`\n=== GATE CHECK PASSED ===`);
  console.log(`Ready to execute gated buy with proof from epoch ${proof.epoch}`);
}

async function main() {
  const args = process.argv.slice(2);

  if (args.length < 2) {
    console.log(`Usage: npx ts-node scripts/gated-buy.ts <token-mint> <sol-amount> [min-attention]`);
    console.log(`\nExample:`);
    console.log(`  npx ts-node scripts/gated-buy.ts TokenMintAddress 0.1 1000000`);
    process.exit(1);
  }

  const tokenMint = new PublicKey(args[0]);
  const solAmount = parseFloat(args[1]);
  const minAttention = args[2] ? BigInt(args[2]) : 1_000_000n;

  await gatedBuy(tokenMint, solAmount, minAttention);
}

main().catch((err: Error) => {
  console.error('Error:', err.message);
  process.exit(1);
});
