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
  if (args.length < 1) {
    console.error("Usage: ts-node initialize-channel.ts <channel>");
    console.error("Example: ts-node initialize-channel.ts youtube_lofi");
    process.exit(1);
  }

  const [channel] = args;

  // Load wallet
  const walletPath = process.env.ANCHOR_WALLET || `${process.env.HOME}/.config/solana/id.json`;
  const walletKeypair = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(walletPath, "utf-8")))
  );

  // Setup connection
  const connection = new Connection(
    process.env.ANCHOR_PROVIDER_URL || "https://api.mainnet-beta.solana.com",
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
  // Guard against missing account sizes
  if (idl.accounts) {
    idl.accounts.forEach((acc: any) => {
      if (acc.size === null || acc.size === undefined) {
        acc.size = 0;
      }
    });
  }
  const program = new Program(idl, PROGRAM_ID, provider);

  // Get mint from environment
  const ATTENTION_MINT = new PublicKey(
    process.env.ATTENTION_MINT || "ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe"
  );

  console.log(`Initializing channel: ${channel}`);
  console.log(`Mint: ${ATTENTION_MINT.toString()}`);
  console.log(`Authority: ${wallet.publicKey.toString()}`);

  // Derive required accounts
  const [protocolState] = PublicKey.findProgramAddressSync(
    [Buffer.from("protocol"), ATTENTION_MINT.toBuffer()],
    new PublicKey(PROGRAM_ID)
  );

  const [channelState] = PublicKey.findProgramAddressSync(
    [Buffer.from("channel"), ATTENTION_MINT.toBuffer(), Buffer.from(channel)],
    new PublicKey(PROGRAM_ID)
  );

  console.log(`Protocol State: ${protocolState.toString()}`);
  console.log(`Channel State: ${channelState.toString()}`);

  // Check if channel already exists
  try {
    const channelAccount = await connection.getAccountInfo(channelState);
    if (channelAccount && channelAccount.owner.equals(new PublicKey(PROGRAM_ID))) {
      console.log(`✓ Channel already initialized`);
      console.log(`  Owner: ${channelAccount.owner.toString()}`);
      console.log(`  Size: ${channelAccount.data.length} bytes`);
      return;
    }
  } catch (e) {
    // Account doesn't exist, proceed with initialization
  }

  try {
    // Anchor converts snake_case to camelCase for method names
    const tx = await (program.methods as any)
      .initializeChannel(channel)
      .accounts({
        payer: wallet.publicKey,
        protocolState,
        channelState,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    console.log(`✓ Channel initialized successfully`);
    console.log(`  Transaction: ${tx}`);
    console.log(`  View on Solscan: https://solscan.io/tx/${tx}`);
  } catch (error: any) {
    console.error("Initialization failed:", error);
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
