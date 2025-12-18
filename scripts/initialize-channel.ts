#!/usr/bin/env ts-node

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { createHash } from "crypto";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import pkg from "js-sha3";
const { keccak256 } = pkg;

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const PROGRAM_ID = "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop";

function deriveSubjectIdHexKeccak256(channel: string): string {
  const lower = channel.toLowerCase();
  const input = Buffer.concat([Buffer.from("channel:"), Buffer.from(lower)]);
  return keccak256(input);
}

function tryDeriveSubjectIdHexSha3_256(channel: string): string | null {
  try {
    const lower = channel.toLowerCase();
    const input = Buffer.concat([Buffer.from("channel:"), Buffer.from(lower)]);
    return createHash("sha3-256").update(input).digest("hex");
  } catch {
    return null;
  }
}

function deriveSubjectId(channel: string): PublicKey {
  const hashBytes = Buffer.from(deriveSubjectIdHexKeccak256(channel), "hex");
  return new PublicKey(hashBytes);
}

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
  // Guard against missing account sizes
  if (idl.accounts) {
    idl.accounts.forEach((acc: any) => {
      if (acc.size === null || acc.size === undefined) {
        acc.size = 0;
      }
    });
  }
  const program = new Program(idl, PROGRAM_ID, provider);

  // Mint selection:
  // - Prefer CCM_V3_MINT (canonical for v3 ops)
  // - Fallback to ATTENTION_MINT for legacy environments
  // - Finally, default to the legacy mint constant
  const mintStr =
    process.env.CCM_V3_MINT ||
    process.env.ATTENTION_MINT ||
    "ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe";
  const mintSource = process.env.CCM_V3_MINT
    ? "CCM_V3_MINT"
    : process.env.ATTENTION_MINT
      ? "ATTENTION_MINT"
      : "default";
  const ATTENTION_MINT = new PublicKey(mintStr);
  const subjectId = deriveSubjectId(channel);
  const subjectIdHexKeccak = deriveSubjectIdHexKeccak256(channel);
  const subjectIdHexSha3 = tryDeriveSubjectIdHexSha3_256(channel);
  const subjectIdSha3 = subjectIdHexSha3
    ? new PublicKey(Buffer.from(subjectIdHexSha3, "hex"))
    : null;

  console.log(`Initializing channel: ${channel}`);
  console.log(`Mint (${mintSource}): ${ATTENTION_MINT.toString()}`);
  console.log(`Subject ID: ${subjectId.toString()}`);
  console.log(`Subject ID (Keccak-256 hex): ${subjectIdHexKeccak}`);
  if (subjectIdHexSha3) {
    console.log(`Subject ID (SHA3-256 hex):   ${subjectIdHexSha3} (diagnostic)`);
  }
  console.log(`Authority: ${wallet.publicKey.toString()}`);

  // Derive required accounts
  const programId = new PublicKey(PROGRAM_ID);
  const [protocolState] = PublicKey.findProgramAddressSync(
    [Buffer.from("protocol"), ATTENTION_MINT.toBuffer()],
    programId
  );

  const [channelState] = PublicKey.findProgramAddressSync(
    [Buffer.from("channel_state"), ATTENTION_MINT.toBuffer(), subjectId.toBuffer()],
    programId
  );

  console.log(`Protocol State: ${protocolState.toString()}`);
  console.log(`Channel State: ${channelState.toString()}`);

  // Preflight: validate derived PDA and existing state (if present).
  const channelAccount = await connection.getAccountInfo(channelState, "confirmed");
  if (channelAccount) {
    if (!channelAccount.owner.equals(programId)) {
      const sha3Hint = (() => {
        if (!subjectIdSha3) return "";
        const [sha3ChannelState] = PublicKey.findProgramAddressSync(
          [Buffer.from("channel_state"), ATTENTION_MINT.toBuffer(), subjectIdSha3.toBuffer()],
          programId
        );
        return ` If you accidentally used SHA3-256 for subject_id, the PDA would be ${sha3ChannelState.toBase58()}.`;
      })();
      throw new Error(
        `Preflight failed: ChannelState ${channelState.toBase58()} exists but is owned by ${channelAccount.owner.toBase58()} (expected ${programId.toBase58()}).\n` +
          `Hint: check PROGRAM_ID/mint/cluster.${sha3Hint}`
      );
    }

    const version = channelAccount.data.length >= 9 ? channelAccount.data[8] : undefined;
    if (version === 1) {
      console.log(`✓ Channel already initialized (version=1)`);
      console.log(`  Owner: ${channelAccount.owner.toString()}`);
      console.log(`  Size: ${channelAccount.data.length} bytes`);
      return;
    }

    if (version !== 0 && version !== undefined) {
      throw new Error(
        `Preflight failed: ChannelState ${channelState.toBase58()} has unexpected version=${version}.\n` +
          `Refusing to proceed. Verify you're targeting the correct channel/mint and using Keccak-256 for subject_id.`
      );
    }
  }

  try {
    // Anchor converts snake_case to camelCase for method names
    const tx = await (program.methods as any)
      .initializeChannel(subjectId)
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
