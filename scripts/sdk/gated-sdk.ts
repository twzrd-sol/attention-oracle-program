#!/usr/bin/env npx ts-node
/**
 * Gated Pump.fun SDK Wrapper
 *
 * Trustless attention-gated buys via CPI to Attention Oracle Protocol.
 * Gate check runs on-chain BEFORE pump.fun buy executes.
 */

import * as fs from 'fs';
import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
  SystemProgram,
  sendAndConfirmTransaction,
} from '@solana/web3.js';
import jsSha3 from 'js-sha3';
const { keccak256 } = jsSha3;

// Config
const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const CCM_MINT = new PublicKey('ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe');
const CHANNEL_NAME = 'pump.fun';

// Discriminator: sha256("global:require_attention_ge")[0..8]
const REQUIRE_ATTENTION_GE_DISC = Buffer.from([0x78, 0x6d, 0xba, 0x18, 0xb5, 0x34, 0x46, 0x91]);

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

export class GatedPumpSDK {
  connection: Connection;
  programId: PublicKey;
  ccmMint: PublicKey;
  channelState: PublicKey;
  channel: string;

  constructor(
    connection: Connection,
    programId = PROGRAM_ID,
    ccmMint = CCM_MINT,
    channel = CHANNEL_NAME
  ) {
    this.connection = connection;
    this.programId = programId;
    this.ccmMint = ccmMint;
    this.channel = channel;

    const subjectId = deriveSubjectId(channel);
    this.channelState = deriveChannelState(ccmMint, subjectId);
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

    // Calculate data size
    const dataSize =
      8 + // discriminator
      4 + channelBytes.length + // channel string
      8 + // epoch
      4 + // index
      8 + // amount
      4 + idBytes.length + // id string
      4 + (proof.length * 32) + // proof vec
      8; // min_attention

    const data = Buffer.alloc(dataSize);
    let offset = 0;

    // Discriminator
    REQUIRE_ATTENTION_GE_DISC.copy(data, offset);
    offset += 8;

    // Channel string (length-prefixed)
    data.writeUInt32LE(channelBytes.length, offset);
    offset += 4;
    channelBytes.copy(data, offset);
    offset += channelBytes.length;

    // Epoch (u64)
    data.writeBigUInt64LE(epoch, offset);
    offset += 8;

    // Index (u32)
    data.writeUInt32LE(index, offset);
    offset += 4;

    // Amount (u64)
    data.writeBigUInt64LE(amount, offset);
    offset += 8;

    // ID string (length-prefixed)
    data.writeUInt32LE(idBytes.length, offset);
    offset += 4;
    idBytes.copy(data, offset);
    offset += idBytes.length;

    // Proof vec (length-prefixed array of [u8; 32])
    data.writeUInt32LE(proof.length, offset);
    offset += 4;
    for (const node of proof) {
      node.copy(data, offset);
      offset += 32;
    }

    // Min attention (u64)
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
   * Execute gated buy: CPI gate check + pump.fun buy
   *
   * @param buyer - Buyer keypair
   * @param tokenMint - Token to buy
   * @param solAmount - SOL amount in lamports
   * @param epoch - Attention epoch
   * @param index - Merkle leaf index
   * @param amount - Attention amount from proof
   * @param proof - Merkle proof nodes
   * @param minAttention - Minimum attention threshold (micro-tokens)
   * @param pumpBuyIx - Optional pump.fun buy instruction (if pre-built)
   */
  async gatedBuy(
    buyer: Keypair,
    tokenMint: PublicKey,
    solAmount: bigint,
    epoch: bigint,
    index: number,
    amount: bigint,
    proof: Buffer[],
    minAttention: bigint = 1_000_000n,
    pumpBuyIx?: TransactionInstruction
  ): Promise<string> {
    // Build gate instruction
    const gateIx = this.buildGateInstruction(
      buyer.publicKey,
      epoch,
      index,
      amount,
      buyer.publicKey.toBase58(), // id = wallet address
      proof,
      minAttention
    );

    // Build transaction
    const tx = new Transaction().add(gateIx);

    // Add pump.fun buy instruction if provided
    if (pumpBuyIx) {
      tx.add(pumpBuyIx);
    }

    // Send and confirm
    const sig = await sendAndConfirmTransaction(this.connection, tx, [buyer], {
      commitment: 'confirmed',
    });

    return sig;
  }

  /**
   * Simulate gate check without executing
   */
  async simulateGate(
    owner: PublicKey,
    epoch: bigint,
    index: number,
    amount: bigint,
    proof: Buffer[],
    minAttention: bigint
  ): Promise<{ success: boolean; logs: string[] }> {
    const gateIx = this.buildGateInstruction(
      owner,
      epoch,
      index,
      amount,
      owner.toBase58(),
      proof,
      minAttention
    );

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
}

// CLI Test
async function main() {
  const args = process.argv.slice(2);

  if (args.length < 1) {
    console.log(`Usage: npx ts-node scripts/gated-sdk.ts <action> [args...]`);
    console.log(`\nActions:`);
    console.log(`  simulate <wallet> <epoch> <amount> <min>  - Simulate gate check`);
    console.log(`  info                                      - Show SDK info`);
    process.exit(1);
  }

  const action = args[0];
  const connection = new Connection('https://api.mainnet-beta.solana.com', 'confirmed');
  const sdk = new GatedPumpSDK(connection);

  if (action === 'info') {
    console.log(`\n=== GatedPumpSDK Info ===`);
    console.log(`Program ID: ${sdk.programId.toBase58()}`);
    console.log(`CCM Mint: ${sdk.ccmMint.toBase58()}`);
    console.log(`Channel: ${sdk.channel}`);
    console.log(`Channel State: ${sdk.channelState.toBase58()}`);
    return;
  }

  if (action === 'simulate') {
    const wallet = new PublicKey(args[1] || 'AbSAtodi1WUCudvXcMGa5WNhZYZocsDn6y1VcUgyKjSm');
    const epoch = BigInt(args[2] || '5882453');
    const amount = BigInt(args[3] || '1000000000000');
    const min = BigInt(args[4] || '1000000');

    console.log(`\n=== Simulating Gate Check ===`);
    console.log(`Wallet: ${wallet.toBase58()}`);
    console.log(`Epoch: ${epoch}`);
    console.log(`Amount: ${amount}`);
    console.log(`Min: ${min}`);

    const result = await sdk.simulateGate(wallet, epoch, 0, amount, [], min);
    console.log(`\nResult: ${result.success ? 'PASS' : 'FAIL'}`);
    if (result.logs.length > 0) {
      console.log(`Logs:`);
      result.logs.forEach(log => console.log(`  ${log}`));
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
