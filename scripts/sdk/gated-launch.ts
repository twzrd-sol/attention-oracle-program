#!/usr/bin/env npx tsx
/**
 * Attention-Gated Token Launch
 *
 * Gate pump.fun token creation behind attention thresholds.
 * Only wallets with sufficient attention can launch new tokens.
 *
 * Flow:
 * 1. Verify attention via CPI to require_attention_ge
 * 2. If gate passes, execute pump.fun createAndBuy
 *
 * Usage:
 *   npx tsx scripts/sdk/gated-launch.ts launch <name> <symbol> <uri> <initial-sol> [min-attention]
 *   npx tsx scripts/sdk/gated-launch.ts check <epoch> <amount> <min>
 */

import * as fs from 'fs';
import { PumpFunSDK } from 'pumpdotfun-sdk';
import { AnchorProvider, Wallet } from '@coral-xyz/anchor';
import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
  LAMPORTS_PER_SOL,
} from '@solana/web3.js';
import jsSha3 from 'js-sha3';
const { keccak256 } = jsSha3;

// Program constants
const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const CCM_MINT = new PublicKey('ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe');
const CHANNEL_NAME = 'pump.fun';

// Discriminator: sha256("global:require_attention_ge")[0..8]
const REQUIRE_ATTENTION_GE_DISC = Buffer.from([0x78, 0x6d, 0xba, 0x18, 0xb5, 0x34, 0x46, 0x91]);

// Config
const RPC_URL = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com';
const KEYPAIR_PATH = process.env.KEYPAIR_PATH || `${process.env.HOME}/.config/solana/id.json`;

// R2 claims config (for fetching proofs)
const BUCKET = 'twzrd-claims';
const CF_TOKEN = process.env.CLOUDFLARE_API_TOKEN || '';

function deriveSubjectId(channel: string): Buffer {
  const input = `channel:${channel.toLowerCase()}`;
  return Buffer.from(keccak256(input), 'hex');
}

function deriveChannelState(mint: PublicKey, subjectId: Buffer): PublicKey {
  const [pda] = PublicKey.findProgramAddressSync(
    [Buffer.from('channel_state'), mint.toBuffer(), subjectId],
    PROGRAM_ID
  );
  return pda;
}

interface AttentionProof {
  epoch: number;
  index: number;
  amount: bigint;
  proof: Buffer[];
}

export class GatedLaunchSDK {
  connection: Connection;
  provider: AnchorProvider;
  pumpSdk: PumpFunSDK;
  programId: PublicKey;
  ccmMint: PublicKey;
  channelState: PublicKey;
  channel: string;

  constructor(provider: AnchorProvider) {
    this.connection = provider.connection;
    this.provider = provider;
    this.pumpSdk = new PumpFunSDK(provider);
    this.programId = PROGRAM_ID;
    this.ccmMint = CCM_MINT;
    this.channel = CHANNEL_NAME;

    const subjectId = deriveSubjectId(CHANNEL_NAME);
    this.channelState = deriveChannelState(CCM_MINT, subjectId);
  }

  /**
   * Build require_attention_ge instruction
   */
  buildGateInstruction(
    owner: PublicKey,
    epoch: bigint,
    index: number,
    amount: bigint,
    id: string,
    proof: Buffer[],
    minAttention: bigint
  ): TransactionInstruction {
    const channelBytes = Buffer.from(this.channel);
    const idBytes = Buffer.from(id);

    const dataSize =
      8 + 4 + channelBytes.length + 8 + 4 + 8 + 4 + idBytes.length + 4 + (proof.length * 32) + 8;

    const data = Buffer.alloc(dataSize);
    let offset = 0;

    REQUIRE_ATTENTION_GE_DISC.copy(data, offset); offset += 8;
    data.writeUInt32LE(channelBytes.length, offset); offset += 4;
    channelBytes.copy(data, offset); offset += channelBytes.length;
    data.writeBigUInt64LE(epoch, offset); offset += 8;
    data.writeUInt32LE(index, offset); offset += 4;
    data.writeBigUInt64LE(amount, offset); offset += 8;
    data.writeUInt32LE(idBytes.length, offset); offset += 4;
    idBytes.copy(data, offset); offset += idBytes.length;
    data.writeUInt32LE(proof.length, offset); offset += 4;
    for (const node of proof) { node.copy(data, offset); offset += 32; }
    data.writeBigUInt64LE(minAttention, offset);

    return new TransactionInstruction({
      keys: [
        { pubkey: owner, isSigner: false, isWritable: false },
        { pubkey: this.ccmMint, isSigner: false, isWritable: false },
        { pubkey: this.channelState, isSigner: false, isWritable: false },
      ],
      programId: this.programId,
      data,
    });
  }

  /**
   * Simulate gate check
   */
  async simulateGate(
    owner: PublicKey,
    epoch: bigint,
    index: number,
    amount: bigint,
    proof: Buffer[],
    minAttention: bigint
  ): Promise<{ success: boolean; logs: string[] }> {
    const gateIx = this.buildGateInstruction(owner, epoch, index, amount, owner.toBase58(), proof, minAttention);
    const tx = new Transaction().add(gateIx);
    const { blockhash } = await this.connection.getLatestBlockhash();
    tx.recentBlockhash = blockhash;
    tx.feePayer = owner;

    const simulation = await this.connection.simulateTransaction(tx);
    return {
      success: !simulation.value.err,
      logs: simulation.value.logs || [],
    };
  }

  /**
   * Execute gate check (on-chain verification)
   */
  async gateCheck(
    buyer: Keypair,
    epoch: bigint,
    index: number,
    amount: bigint,
    proof: Buffer[],
    minAttention: bigint
  ): Promise<string> {
    const gateIx = this.buildGateInstruction(
      buyer.publicKey, epoch, index, amount, buyer.publicKey.toBase58(), proof, minAttention
    );
    const tx = new Transaction().add(gateIx);
    return sendAndConfirmTransaction(this.connection, tx, [buyer], { commitment: 'confirmed' });
  }

  /**
   * Gated token launch: verify attention, then create token
   *
   * Two-step process (gate verification is separate tx for now):
   * 1. Verify attention threshold via on-chain gate
   * 2. Execute pump.fun createAndBuy
   */
  async gatedLaunch(
    creator: Keypair,
    metadata: { name: string; symbol: string; uri: string },
    initialSolBuy: bigint,
    epoch: bigint,
    index: number,
    amount: bigint,
    proof: Buffer[],
    minAttention: bigint = 1_000_000n,
    slippageBps: bigint = 500n
  ): Promise<{ gateSig: string; launchResult: any }> {
    console.log(`\n=== Gated Token Launch ===`);
    console.log(`Creator: ${creator.publicKey.toBase58()}`);
    console.log(`Token: ${metadata.name} (${metadata.symbol})`);
    console.log(`Initial Buy: ${Number(initialSolBuy) / LAMPORTS_PER_SOL} SOL`);
    console.log(`Min Attention: ${minAttention}`);

    // Step 1: Verify attention gate
    console.log(`\nStep 1: Verifying attention gate...`);
    const gateResult = await this.simulateGate(creator.publicKey, epoch, index, amount, proof, minAttention);

    if (!gateResult.success) {
      console.error(`Gate FAILED - Launch blocked`);
      console.error(`Logs:`, gateResult.logs);
      throw new Error('Insufficient attention for token launch');
    }

    // Execute gate on-chain for permanent record
    const gateSig = await this.gateCheck(creator, epoch, index, amount, proof, minAttention);
    console.log(`Gate PASSED: ${gateSig}`);

    // Step 2: Create and buy token
    console.log(`\nStep 2: Creating token on pump.fun...`);
    const mintKeypair = Keypair.generate();

    const launchResult = await this.pumpSdk.createAndBuy(
      creator,
      mintKeypair,
      {
        name: metadata.name,
        symbol: metadata.symbol,
        uri: metadata.uri,
      },
      initialSolBuy,
      slippageBps
    );

    console.log(`\n=== Launch Complete ===`);
    console.log(`Mint: ${mintKeypair.publicKey.toBase58()}`);
    console.log(`Gate TX: ${gateSig}`);

    return { gateSig, launchResult };
  }

  /**
   * Check if wallet has sufficient attention for launch
   */
  async checkEligibility(
    wallet: PublicKey,
    epoch: bigint,
    amount: bigint,
    minAttention: bigint
  ): Promise<boolean> {
    const result = await this.simulateGate(wallet, epoch, 0, amount, [], minAttention);
    return result.success;
  }
}

// CLI
async function main() {
  const args = process.argv.slice(2);

  if (args.length < 1) {
    console.log(`Usage: npx tsx scripts/sdk/gated-launch.ts <action> [args...]`);
    console.log(`\nActions:`);
    console.log(`  check <epoch> <amount> <min>                    - Check gate eligibility`);
    console.log(`  launch <name> <symbol> <uri> <sol> [min]        - Execute gated launch`);
    console.log(`  info                                            - Show SDK info`);
    process.exit(1);
  }

  const action = args[0];
  const connection = new Connection(RPC_URL, 'confirmed');

  // Load keypair
  let keypair: Keypair;
  try {
    const keypairData = JSON.parse(fs.readFileSync(KEYPAIR_PATH, 'utf-8'));
    keypair = Keypair.fromSecretKey(Uint8Array.from(keypairData));
  } catch {
    keypair = Keypair.generate();
  }

  const wallet = new Wallet(keypair);
  const provider = new AnchorProvider(connection, wallet, { commitment: 'confirmed' });
  const sdk = new GatedLaunchSDK(provider);

  if (action === 'info') {
    console.log(`\n=== GatedLaunchSDK Info ===`);
    console.log(`Program ID: ${sdk.programId.toBase58()}`);
    console.log(`CCM Mint: ${sdk.ccmMint.toBase58()}`);
    console.log(`Channel: ${sdk.channel}`);
    console.log(`Channel State: ${sdk.channelState.toBase58()}`);
    console.log(`Wallet: ${keypair.publicKey.toBase58()}`);
    return;
  }

  if (action === 'check') {
    const epoch = BigInt(args[1] || Math.floor(Date.now() / 300000) - 1);
    const amount = BigInt(args[2] || '1000000');
    const min = BigInt(args[3] || '1000000');

    console.log(`\n=== Gate Eligibility Check ===`);
    console.log(`Wallet: ${keypair.publicKey.toBase58()}`);
    console.log(`Epoch: ${epoch}`);
    console.log(`Amount: ${amount}`);
    console.log(`Min: ${min}`);

    const result = await sdk.simulateGate(keypair.publicKey, epoch, 0, amount, [], min);
    console.log(`\nResult: ${result.success ? 'ELIGIBLE' : 'NOT ELIGIBLE'}`);
    if (result.logs.length > 0) {
      result.logs.slice(-5).forEach(log => console.log(`  ${log}`));
    }
    return;
  }

  if (action === 'launch') {
    if (args.length < 5) {
      console.error('Usage: launch <name> <symbol> <uri> <initial-sol> [min-attention]');
      process.exit(1);
    }

    const name = args[1];
    const symbol = args[2];
    const uri = args[3];
    const initialSol = BigInt(Math.floor(parseFloat(args[4]) * LAMPORTS_PER_SOL));
    const minAttention = args[5] ? BigInt(args[5]) : 1_000_000n;

    // For demo: use current epoch with placeholder proof
    const epoch = BigInt(Math.floor(Date.now() / 300000) - 1);

    console.log(`\nNote: This is a gated launch test.`);
    console.log(`Requires valid attention proof for epoch ${epoch}.`);
    console.log(`Without proof, gate will fail (expected behavior).\n`);

    try {
      const result = await sdk.gatedLaunch(
        keypair,
        { name, symbol, uri },
        initialSol,
        epoch,
        0,
        minAttention, // Use min as amount for test
        [],
        minAttention
      );
      console.log(`Launch successful!`);
      console.log(`Gate TX: ${result.gateSig}`);
    } catch (err: any) {
      console.error(`Launch failed: ${err.message}`);
      console.log(`\nTo launch, you need attention >= ${minAttention} in epoch ${epoch}`);
    }
    return;
  }

  console.error(`Unknown action: ${action}`);
  process.exit(1);
}

main().catch(err => {
  console.error('Error:', err.message);
  process.exit(1);
});
