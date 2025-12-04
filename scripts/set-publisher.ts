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
const ATTENTION_MINT = new PublicKey("ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe");

// Seeds
const PROTOCOL_SEED = Buffer.from("protocol");

// Instruction discriminator for update_publisher
// sha256("global:update_publisher")[0..8]
const UPDATE_PUBLISHER_DISCRIMINATOR = Buffer.from([
  232, 168, 138, 214, 95, 57, 224, 234
]);

async function main() {
  const args = process.argv.slice(2);
  if (args.length < 1) {
    console.error("Usage: ts-node set-publisher.ts <new_publisher_pubkey>");
    console.error(
      "Example: ts-node set-publisher.ts AmMftc4zHgR4yYfv29awV9Q46emo2aGPFW8utP81CsBv"
    );
    process.exit(1);
  }

  const newPublisherStr = args[0];
  const newPublisher = new PublicKey(newPublisherStr);

  console.log("=".repeat(70));
  console.log("Update Publisher");
  console.log("=".repeat(70));
  console.log(`New Publisher: ${newPublisher.toString()}`);

  // Load admin wallet
  const walletPath =
    process.env.ANCHOR_WALLET || `${process.env.HOME}/.config/solana/id.json`;
  const admin = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(walletPath, "utf-8")))
  );

  console.log(`Admin: ${admin.publicKey.toString()}`);

  // Setup connection
  const connection = new Connection(
    process.env.ANCHOR_PROVIDER_URL || "https://api.mainnet-beta.solana.com",
    "confirmed"
  );

  // Derive PDAs
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, ATTENTION_MINT.toBuffer()],
    PROGRAM_ID
  );

  console.log(`Protocol State: ${protocolState.toString()}`);

  // Serialize instruction args: new_publisher (32 bytes pubkey)
  const argsData = Buffer.from(newPublisher.toBuffer());

  // Build instruction data: discriminator + args
  const data = Buffer.concat([UPDATE_PUBLISHER_DISCRIMINATOR, argsData]);

  // Build accounts array
  const keys = [
    { pubkey: admin.publicKey, isSigner: true, isWritable: true }, // admin/signer
    { pubkey: protocolState, isSigner: false, isWritable: true }, // protocol_state
  ];

  const instruction = new TransactionInstruction({
    keys,
    programId: PROGRAM_ID,
    data,
  });

  const transaction = new Transaction().add(instruction);

  try {
    console.log(`\nSending update publisher transaction...`);
    const signature = await sendAndConfirmTransaction(
      connection,
      transaction,
      [admin],
      {
        commitment: "confirmed",
        skipPreflight: false,
      }
    );

    console.log(`✓ Publisher updated successfully`);
    console.log(`  Transaction: ${signature}`);
    console.log(`  View on Solscan: https://solscan.io/tx/${signature}`);
    console.log(
      `\n✅ Protocol state updated: new publisher = ${newPublisher.toString()}`
    );
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
