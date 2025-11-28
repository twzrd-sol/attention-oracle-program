#!/usr/bin/env ts-node

/**
 * On-demand test epoch publisher for rapid frontend iteration.
 *
 * Creates a single-entry merkle tree for the current epoch and publishes it on-chain.
 * Perfect for testing claim flows without waiting for natural epoch boundaries.
 *
 * Usage:
 *   # Default: uses amm-admin wallet with 100 tokens
 *   ANCHOR_WALLET=~/.config/solana/amm-admin.json ts-node test-epoch-now.ts
 *
 *   # Custom wallet and amount
 *   ts-node test-epoch-now.ts <WALLET_PUBKEY> <AMOUNT>
 *
 * Example:
 *   ts-node test-epoch-now.ts 9HXDBiuLVFEVpd7gWepYKWHB6HRPJi2A3vcm3x1WfHrF 250
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
const ATTENTION_MINT = new PublicKey("ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe");

// Epoch configuration (from constants.rs)
const EPOCH_START_SLOT = 293000000n;
const EPOCH_DURATION_SLOTS = 28800n; // ~3.2 hours

// Seeds
const PROTOCOL_SEED = Buffer.from("protocol");
const CHANNEL_STATE_SEED = Buffer.from("channel_state");

// Instruction discriminator for set_channel_merkle_root
// From current deployed program IDL
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

// Manual borsh serialization for set_channel_merkle_root args
function serializeArgs(channel: string, epoch: bigint, root: Uint8Array): Buffer {
  const channelUtf8 = Buffer.from(channel, "utf-8");
  const channelLen = Buffer.alloc(4);
  channelLen.writeUInt32LE(channelUtf8.length, 0);

  const epochBuf = Buffer.alloc(8);
  epochBuf.writeBigUInt64LE(epoch, 0);

  const rootBuf = Buffer.from(root);

  return Buffer.concat([channelLen, channelUtf8, epochBuf, rootBuf]);
}

// Compute merkle root for single leaf (no hashing needed)
function computeSingleLeafRoot(walletPubkey: string, amount: bigint): Buffer {
  // For single entry, the leaf IS the root
  // Leaf format: keccak256(keccak256(id) || amount_u64_le)
  const idHash = keccak256(Buffer.from(walletPubkey, "utf-8"));
  const amountBuf = Buffer.alloc(8);
  amountBuf.writeBigUInt64LE(amount, 0);

  const leaf = Buffer.concat([
    Buffer.from(idHash, "hex"),
    amountBuf
  ]);

  const rootHex = keccak256(leaf);
  return Buffer.from(rootHex, "hex");
}

// Calculate current epoch from slot
async function getCurrentEpoch(connection: Connection): Promise<bigint> {
  const slot = await connection.getSlot("confirmed");
  const slotBigInt = BigInt(slot);

  if (slotBigInt < EPOCH_START_SLOT) {
    throw new Error(`Current slot ${slot} is before epoch start ${EPOCH_START_SLOT}`);
  }

  const epoch = (slotBigInt - EPOCH_START_SLOT) / EPOCH_DURATION_SLOTS;
  return epoch;
}

async function main() {
  const args = process.argv.slice(2);

  // Load wallet for publishing
  const walletPath =
    process.env.ANCHOR_WALLET || `${process.env.HOME}/.config/solana/id.json`;
  const payerWallet = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(walletPath, "utf-8")))
  );

  // Parse args: wallet pubkey and amount (default to payer wallet and 100 tokens)
  const targetWallet = args[0] || payerWallet.publicKey.toString();
  const amount = args[1] ? BigInt(args[1]) : 100n;

  // Setup connection
  const connection = new Connection(
    process.env.ANCHOR_PROVIDER_URL || "https://api.mainnet-beta.solana.com",
    "confirmed"
  );

  // Get current epoch
  const currentEpoch = await getCurrentEpoch(connection);
  const channel = "youtube_lofi";
  const slotIndex = Number(currentEpoch % 10n);

  console.log("⚡ Publishing on-demand test epoch");
  console.log(`Target epoch: ${currentEpoch} (slot ${slotIndex})`);
  console.log(`Wallet: ${targetWallet}`);
  console.log(`Amount: ${amount} tokens`);
  console.log();

  // Compute merkle root (single leaf = root)
  const root = computeSingleLeafRoot(targetWallet, amount);
  const rootHex = root.toString("hex");

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

  // Serialize instruction
  const argsData = serializeArgs(channel, currentEpoch, root);
  const data = Buffer.concat([SET_MERKLE_ROOT_DISCRIMINATOR, argsData]);

  const keys = [
    { pubkey: payerWallet.publicKey, isSigner: true, isWritable: true },
    { pubkey: protocolState, isSigner: false, isWritable: true },
    { pubkey: channelState, isSigner: false, isWritable: true },
    { pubkey: SYSTEM_PROGRAM, isSigner: false, isWritable: false },
  ];

  const instruction = new TransactionInstruction({
    keys,
    programId: PROGRAM_ID,
    data,
  });

  const transaction = new Transaction().add(instruction);

  try {
    const signature = await sendAndConfirmTransaction(
      connection,
      transaction,
      [payerWallet],
      {
        commitment: "confirmed",
        skipPreflight: false,
      }
    );

    console.log(`✅ Published! Tx: ${signature.slice(0, 8)}...`);
    console.log(`   View: https://solscan.io/tx/${signature}`);
    console.log();
    console.log("// Claim data for frontend:");
    console.log("const claimData = {");
    console.log(`  channel: "${channel}",`);
    console.log(`  epoch: ${currentEpoch}n,`);
    console.log(`  index: 0,  // single entry`);
    console.log(`  amount: ${amount}n,`);
    console.log(`  id: "${targetWallet}",`);
    console.log(`  proof: [],  // single leaf requires no proof`);
    console.log("};");
    console.log();
    console.log(`Root: ${rootHex}`);
  } catch (error: any) {
    console.error("❌ Failed:", error.message);
    if (error.logs) {
      console.error("\nProgram logs:");
      error.logs.forEach((log: string) => console.error(`  ${log}`));
    }
    process.exit(1);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
