#!/usr/bin/env ts-node

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const PROGRAM_ID = "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop";

async function main() {
  const args = process.argv.slice(2);
  if (args.length < 3) {
    console.error("Usage: ts-node set-merkle-root.ts <channel> <epoch> <root_hex>");
    process.exit(1);
  }

  const [channel, epochStr, rootHex] = args;
  const epoch = BigInt(epochStr);

  // Convert hex root to [u8; 32]
  const rootBytes = Buffer.from(rootHex, "hex");
  if (rootBytes.length !== 32) {
    console.error("Root must be 32 bytes (64 hex chars)");
    process.exit(1);
  }

  // Load wallet
  const walletPath = process.env.ANCHOR_WALLET || `${process.env.HOME}/.config/solana/id.json`;
  const walletKeypair = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(walletPath, "utf-8")))
  );

  // Setup connection
  const connection = new Connection(
    process.env.SYNDICA_RPC!,
    "confirmed"
  );

  const wallet = new anchor.Wallet(walletKeypair);
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });
  anchor.setProvider(provider);

  // Load program
  const idl = JSON.parse(
    fs.readFileSync(`${__dirname}/../target/idl/token_2022.json`, "utf-8")
  );
  // Guard against missing account sizes in IDL by adding default size
  if (idl.accounts) {
    idl.accounts.forEach((acc: any) => {
      if (acc.size === null || acc.size === undefined) {
        acc.size = 0; // Default size, not used for instruction encoding
      }
    });
  }
  const program = new Program(idl, PROGRAM_ID, provider);

  console.log(`Setting merkle root for channel: ${channel}, epoch: ${epoch}`);
  console.log(`Root: ${rootHex}`);
  console.log(`Authority: ${wallet.publicKey.toString()}`);

  // Derive required accounts
  // First, we need to get the protocol_state to find the mint
  // For now, use the known ATTENTION mint from environment or hardcode
  const ATTENTION_MINT = new PublicKey(
    process.env.ATTENTION_MINT || "ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe"
  );

  // Derive protocol_state PDA
  const [protocolState] = PublicKey.findProgramAddressSync(
    [Buffer.from("protocol"), ATTENTION_MINT.toBuffer()],
    new PublicKey(PROGRAM_ID)
  );

  // Derive channel_state PDA
  const [channelState] = PublicKey.findProgramAddressSync(
    [Buffer.from("channel"), ATTENTION_MINT.toBuffer(), Buffer.from(channel)],
    new PublicKey(PROGRAM_ID)
  );

  console.log(`Protocol State: ${protocolState.toString()}`);
  console.log(`Channel State: ${channelState.toString()}`);

  try {
    // Anchor converts snake_case to camelCase for method names
    const tx = await (program.methods as any)
      .setChannelMerkleRoot(channel, epoch, Array.from(rootBytes))
      .accounts({
        payer: wallet.publicKey,
        protocolState,
        channelState,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    console.log(`✓ Transaction successful: ${tx}`);
    console.log(`View on Solscan: https://solscan.io/tx/${tx}`);
  } catch (error) {
    console.error("Transaction failed:", error);
    throw error;
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
