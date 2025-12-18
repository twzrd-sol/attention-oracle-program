#!/usr/bin/env ts-node

/**
 * Publish merkle roots for channel epochs to mainnet.
 *
 * NOTE: This is the canonical publish script for mainnet.
 * The contract on mainnet does NOT include the demo feature,
 * so set_merkle_root_ring is unavailable. Use set_channel_merkle_root only.
 *
 * Usage:
 *   ANCHOR_WALLET=~/.config/solana/amm-admin.json \
 *   ts-node publish-merkle-root.ts <channel> <epoch> <root_hex>
 *
 * Example:
 *   ANCHOR_WALLET=~/.config/solana/amm-admin.json \
 *   ts-node publish-merkle-root.ts youtube_lofi 122523 97fbfdc785963a7fcd1c05d05fc4f893742d68292763ad2f6c500846d87826d1
 */

import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import fs from "fs";
import pkg from "js-sha3";
const { keccak256 } = pkg;

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const SYSTEM_PROGRAM = new PublicKey("11111111111111111111111111111111");

// Seeds from constants.rs
const PROTOCOL_SEED = Buffer.from("protocol");
const CHANNEL_STATE_SEED = Buffer.from("channel_state");

// Instruction discriminator for set_channel_merkle_root
// From current IDL (mainnet program): [65, 24, 16, 6, 63, 105, 153, 123]
// sha256("global:set_channel_merkle_root")[0..8]
const SET_MERKLE_ROOT_DISCRIMINATOR = Buffer.from([
  65, 24, 16, 6, 63, 105, 153, 123
]);

// Derive subject_id using keccak256("channel:", lowercase(channel))
function deriveSubjectId(channel: string): PublicKey {
  const lower = channel.toLowerCase();
  const input = Buffer.concat([
    Buffer.from("channel:"),
    Buffer.from(lower),
  ]);
  const hashHex = keccak256(input);
  const hashBytes = Buffer.from(hashHex, "hex");
  return new PublicKey(hashBytes);
}

// Manual borsh serialization for the instruction args
function serializeArgs(channel: string, epoch: bigint, root: Uint8Array): Buffer {
  // String: u32 LE length + UTF-8 bytes
  const channelUtf8 = Buffer.from(channel, "utf-8");
  const channelLen = Buffer.alloc(4);
  channelLen.writeUInt32LE(channelUtf8.length, 0);

  // u64 LE epoch
  const epochBuf = Buffer.alloc(8);
  epochBuf.writeBigUInt64LE(epoch, 0);

  // [u8; 32] root
  const rootBuf = Buffer.from(root);

  return Buffer.concat([channelLen, channelUtf8, epochBuf, rootBuf]);
}

async function main() {
  const args = process.argv.slice(2);
  if (args.length < 3) {
    console.error("Usage: ts-node publish-merkle-root.ts <channel> <epoch> <root_hex>");
    console.error("Example: ts-node publish-merkle-root.ts youtube_lofi 122513 73bccf0d...");
    process.exit(1);
  }

  const [channel, epochStr, rootHex] = args;
  const epoch = BigInt(epochStr);

  // Convert hex root to bytes
  const rootBytes = Buffer.from(rootHex, "hex");
  if (rootBytes.length !== 32) {
    console.error("Root must be 32 bytes (64 hex chars)");
    process.exit(1);
  }

  // Load wallet
  const walletPath =
    process.env.ANCHOR_WALLET || `${process.env.HOME}/.config/solana/id.json`;
  const wallet = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(walletPath, "utf-8")))
  );

  // Setup connection
  const connection = new Connection(
    process.env.SYNDICA_RPC!,
    "confirmed"
  );

  // Default to live v3 CCM token
  const ATTENTION_MINT = new PublicKey(
    process.env.CCM_V3_MINT || process.env.ATTENTION_MINT || "Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM"
  );

  console.log(`Publishing merkle root for channel: ${channel}, epoch: ${epoch}`);
  console.log(`Root: ${rootHex}`);
  console.log(`Mint: ${ATTENTION_MINT.toString()}`);
  console.log(`Payer: ${wallet.publicKey.toString()}`);

  // Derive PDAs
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, ATTENTION_MINT.toBuffer()],
    PROGRAM_ID
  );

  const subjectId = deriveSubjectId(channel);
  const [channelState] = PublicKey.findProgramAddressSync(
    [CHANNEL_STATE_SEED, ATTENTION_MINT.toBuffer(), subjectId.toBuffer()],
    PROGRAM_ID
  );

  console.log(`Protocol State: ${protocolState.toString()}`);
  console.log(`Subject ID: ${subjectId.toString()}`);
  console.log(`Channel State: ${channelState.toString()}`);

  // Serialize instruction args
  const argsData = serializeArgs(channel, epoch, rootBytes);

  // Build instruction data: discriminator + args
  const data = Buffer.concat([SET_MERKLE_ROOT_DISCRIMINATOR, argsData]);

  // Build accounts array
  const keys = [
    { pubkey: wallet.publicKey, isSigner: true, isWritable: true }, // payer
    { pubkey: protocolState, isSigner: false, isWritable: true }, // protocol_state
    { pubkey: channelState, isSigner: false, isWritable: true }, // channel_state
    { pubkey: SYSTEM_PROGRAM, isSigner: false, isWritable: false }, // system_program
  ];

  const instruction = new TransactionInstruction({
    keys,
    programId: PROGRAM_ID,
    data,
  });

  const transaction = new Transaction().add(instruction);

  try {
    console.log(`Sending transaction...`);
    const signature = await sendAndConfirmTransaction(
      connection,
      transaction,
      [wallet],
      {
        commitment: "confirmed",
        skipPreflight: false,
      }
    );

    console.log(`✓ Merkle root published successfully`);
    console.log(`  Transaction: ${signature}`);
    console.log(`  View on Solscan: https://solscan.io/tx/${signature}`);
    console.log(`\n✅ Channel ${channel} epoch ${epoch} is now live for claims!`);
  } catch (error: any) {
    console.error("Transaction failed:", error);
    if (error.logs) {
      console.error("Program logs:");
      error.logs.forEach((log: string) => console.error(`  ${log}`));
    }
    throw error;
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
