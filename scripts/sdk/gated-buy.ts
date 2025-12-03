#!/usr/bin/env npx ts-node
/**
 * Attention-Gated Pump.fun Buy
 *
 * This script demonstrates how to gate pump.fun token purchases behind
 * attention thresholds using the Attention Oracle Protocol.
 *
 * Flow:
 * 1. Fetch user's attention proof from R2 claims data
 * 2. Verify attention >= minThreshold via CPI to require_attention_ge
 * 3. If verification passes, execute pump.fun buy via SDK
 *
 * Usage:
 *   npx ts-node scripts/gated-buy.ts <token-mint> <sol-amount> [min-attention]
 *
 * Example:
 *   npx ts-node scripts/gated-buy.ts TokenMintAddress 0.1 1000000
 */

import * as fs from 'fs';
import { execSync } from 'child_process';

// Use execSync with curl for HTTP requests (works in all Node versions)
import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
  SystemProgram,
} from '@solana/web3.js';
import {
  getAssociatedTokenAddress,
  TOKEN_2022_PROGRAM_ID,
} from '@solana/spl-token';
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

// Instruction discriminators (first 8 bytes of sha256("global:<name>"))
const REQUIRE_ATTENTION_GE_DISCRIMINATOR = Buffer.from([
  // sha256("global:require_attention_ge")[0..8]
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
  // Get current epoch (5-minute intervals)
  return Math.floor(Date.now() / 300000) - 1;
}

async function fetchClaimsData(epoch: number): Promise<ClaimsData | null> {
  // Use wrangler CLI to fetch from R2
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

  // Build leaves
  const leaves: Buffer[] = entries.map(([wallet, amount], index) => {
    const claimer = new PublicKey(wallet);
    return computeMerkleLeaf(claimer, index, BigInt(amount), wallet);
  });

  // Initialize proof paths
  entries.forEach(([wallet], index) => {
    proofs[wallet] = { index, proof: [] };
  });

  // Build tree bottom-up, collecting proofs
  let currentLevel = leaves.slice();
  while (currentLevel.length > 1) {
    const newLevel: Buffer[] = [];
    for (let i = 0; i < currentLevel.length; i += 2) {
      if (i + 1 < currentLevel.length) {
        // Sort siblings for deterministic ordering
        const [left, right] = [currentLevel[i], currentLevel[i + 1]].sort(Buffer.compare);
        const combined = Buffer.concat([left, right]);
        newLevel.push(Buffer.from(keccak256(combined), 'hex'));

        // Add sibling to proofs for both nodes
        const leftIdx = entries.findIndex(([w]) => {
          const leafIndex = proofs[w]?.index;
          return leafIndex !== undefined && Math.floor(leafIndex / (leaves.length / currentLevel.length)) === i;
        });
        const rightIdx = entries.findIndex(([w]) => {
          const leafIndex = proofs[w]?.index;
          return leafIndex !== undefined && Math.floor(leafIndex / (leaves.length / currentLevel.length)) === i + 1;
        });

        // Simplified: just collect all paths
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

  // Compute merkle tree and proof
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
    id: walletStr, // Using wallet address as ID
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
  // Encode instruction data
  // Format: discriminator (8) + channel_len (4) + channel + epoch (8) + index (4) + amount (8) + id_len (4) + id + proof_len (4) + proof + min_attention (8)

  const channelBytes = Buffer.from(channel);
  const idBytes = Buffer.from(proof.id);

  const dataSize =
    8 + // discriminator
    4 + channelBytes.length + // channel string
    8 + // epoch
    4 + // index
    8 + // amount
    4 + idBytes.length + // id string
    4 + (proof.proof.length * 32) + // proof vec
    8; // min_attention

  const data = Buffer.alloc(dataSize);
  let offset = 0;

  // Discriminator
  REQUIRE_ATTENTION_GE_DISCRIMINATOR.copy(data, offset);
  offset += 8;

  // Channel string (length-prefixed)
  data.writeUInt32LE(channelBytes.length, offset);
  offset += 4;
  channelBytes.copy(data, offset);
  offset += channelBytes.length;

  // Epoch
  data.writeBigUInt64LE(BigInt(proof.epoch), offset);
  offset += 8;

  // Index
  data.writeUInt32LE(proof.index, offset);
  offset += 4;

  // Amount
  data.writeBigUInt64LE(proof.amount, offset);
  offset += 8;

  // ID string (length-prefixed)
  data.writeUInt32LE(idBytes.length, offset);
  offset += 4;
  idBytes.copy(data, offset);
  offset += idBytes.length;

  // Proof vec (length-prefixed array of [u8; 32])
  data.writeUInt32LE(proof.proof.length, offset);
  offset += 4;
  for (const node of proof.proof) {
    node.copy(data, offset);
    offset += 32;
  }

  // Min attention
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
  minAttention: bigint = 1_000_000n // Default: 1 token (6 decimals)
) {
  const connection = new Connection(RPC_URL, 'confirmed');
  const keypairData = JSON.parse(fs.readFileSync(KEYPAIR_PATH, 'utf-8'));
  const keypair = Keypair.fromSecretKey(Uint8Array.from(keypairData));

  console.log(`\n=== Attention-Gated Pump.fun Buy ===`);
  console.log(`Wallet: ${keypair.publicKey.toBase58()}`);
  console.log(`Token: ${tokenMint.toBase58()}`);
  console.log(`Amount: ${solAmount} SOL`);
  console.log(`Min Attention: ${minAttention.toString()} micro-tokens`);

  // Step 1: Get attention proof
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

  // Step 2: Build attention gate instruction
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

  // Step 3: Build pump.fun buy instruction (placeholder - integrate with actual SDK)
  console.log(`\nBuilding pump.fun buy instruction...`);
  // NOTE: Replace with actual pumpdotfun-sdk integration
  // import { PumpFunSDK } from 'pumpdotfun-sdk';
  // const sdk = new PumpFunSDK(provider);
  // const buyIx = await sdk.buildBuyInstruction(tokenMint, solAmount, slippage);

  // For now, just demonstrate the gating transaction
  const tx = new Transaction().add(gateIx);
  // .add(buyIx); // Add pump.fun buy instruction

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
  } catch (err: any) {
    console.error('Simulation error:', err.message);
    process.exit(1);
  }

  // Step 4: Execute transaction (uncomment for real execution)
  // console.log(`\nSending transaction...`);
  // const sig = await connection.sendRawTransaction(tx.serialize());
  // await connection.confirmTransaction({ signature: sig, blockhash, lastValidBlockHeight }, 'confirmed');
  // console.log(`\n=== SUCCESS ===`);
  // console.log(`https://solscan.io/tx/${sig}`);

  console.log(`\n=== GATE CHECK PASSED ===`);
  console.log(`Ready to execute gated buy with proof from epoch ${proof.epoch}`);
}

// CLI entry point
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

main().catch(err => {
  console.error('Error:', err.message);
  process.exit(1);
});
