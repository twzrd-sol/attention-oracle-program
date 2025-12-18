#!/usr/bin/env ts-node

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

// Instruction discriminator for initialize_channel (from IDL)
const INITIALIZE_CHANNEL_DISCRIMINATOR = Buffer.from([
  232, 91, 177, 212, 122, 94, 227, 250
]);

function deriveSubjectId(channel: string): PublicKey {
  const lower = channel.toLowerCase();
  const input = Buffer.concat([Buffer.from("channel:"), Buffer.from(lower)]);
  const hashHex = keccak256(input);
  const hashBytes = Buffer.from(hashHex, "hex");
  return new PublicKey(hashBytes);
}

async function main() {
  const args = process.argv.slice(2);
  if (args.length < 1) {
    console.error("Usage: ts-node initialize-channel-raw.ts <channel>");
    console.error("Example: ts-node initialize-channel-raw.ts youtube_lofi");
    process.exit(1);
  }

  const [channel] = args;

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

  const ATTENTION_MINT = new PublicKey(
    process.env.ATTENTION_MINT || "ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe"
  );
  const subjectId = deriveSubjectId(channel);

  console.log(`Initializing channel: ${channel}`);
  console.log(`Mint: ${ATTENTION_MINT.toString()}`);
  console.log(`Subject ID: ${subjectId.toString()}`);
  console.log(`Payer: ${wallet.publicKey.toString()}`);

  // Derive PDAs
  const [protocolState] = PublicKey.findProgramAddressSync(
    [Buffer.from("protocol"), ATTENTION_MINT.toBuffer()],
    PROGRAM_ID
  );

  const [channelState] = PublicKey.findProgramAddressSync(
    [Buffer.from("channel_state"), ATTENTION_MINT.toBuffer(), subjectId.toBuffer()],
    PROGRAM_ID
  );

  console.log(`Protocol State: ${protocolState.toString()}`);
  console.log(`Channel State: ${channelState.toString()}`);

  // Check if channel already exists
  try {
    const channelAccount = await connection.getAccountInfo(channelState);
    if (channelAccount && channelAccount.owner.equals(PROGRAM_ID)) {
      console.log(`✓ Channel already initialized`);
      console.log(`  Owner: ${channelAccount.owner.toString()}`);
      console.log(`  Size: ${channelAccount.data.length} bytes`);
      return;
    }
  } catch (e) {
    // Account doesn't exist, proceed
  }

  // Serialize instruction args: subject_id (pubkey)
  const argsData = subjectId.toBuffer();

  // Build instruction data: discriminator + args
  const data = Buffer.concat([INITIALIZE_CHANNEL_DISCRIMINATOR, argsData]);

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
    const signature = await sendAndConfirmTransaction(connection, transaction, [wallet], {
      commitment: "confirmed",
    });

    console.log(`✓ Channel initialized successfully`);
    console.log(`  Transaction: ${signature}`);
    console.log(`  View on Solscan: https://solscan.io/tx/${signature}`);
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
