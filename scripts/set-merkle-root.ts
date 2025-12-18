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
  if (args.length < 3) {
    console.error("Usage: ts-node set-merkle-root.ts <channel> <epoch> <root_hex>");
    process.exit(1);
  }

  const [channel, epochStr, rootHex] = args;
  const epoch = new anchor.BN(epochStr);

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

  // Mint selection: default to live v3 CCM token
  const mintStr =
    process.env.CCM_V3_MINT ||
    process.env.ATTENTION_MINT ||
    "Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM";
  const mintSource = process.env.CCM_V3_MINT
    ? "CCM_V3_MINT"
    : process.env.ATTENTION_MINT
      ? "ATTENTION_MINT"
      : "default";
  const ATTENTION_MINT = new PublicKey(mintStr);
  const subjectId = deriveSubjectId(channel);

  console.log(`Mint (${mintSource}): ${ATTENTION_MINT.toBase58()}`);

  // Derive protocol_state PDA
  const [protocolState] = PublicKey.findProgramAddressSync(
    [Buffer.from("protocol"), ATTENTION_MINT.toBuffer()],
    new PublicKey(PROGRAM_ID)
  );

  // Derive channel_state PDA
  const [channelState] = PublicKey.findProgramAddressSync(
    [Buffer.from("channel_state"), ATTENTION_MINT.toBuffer(), subjectId.toBuffer()],
    new PublicKey(PROGRAM_ID)
  );

  console.log(`Protocol State: ${protocolState.toString()}`);
  console.log(`Subject ID: ${subjectId.toString()}`);
  console.log(`Channel State: ${channelState.toString()}`);

  // Preflight: verify channel_state exists + is initialized (version==1) under Keccak-256 seeds.
  const programId = new PublicKey(PROGRAM_ID);
  const channelInfo = await connection.getAccountInfo(channelState, "confirmed");
  if (!channelInfo) {
    const keccakHex = deriveSubjectIdHexKeccak256(channel);
    const sha3Hex = tryDeriveSubjectIdHexSha3_256(channel);
    const sha3Lines = (() => {
      if (!sha3Hex) {
        return `Hint: verify you're using Keccak-256 (not SHA3-256) for subject_id derivation.`;
      }
      const sha3Subject = new PublicKey(Buffer.from(sha3Hex, "hex"));
      const [sha3ChannelState] = PublicKey.findProgramAddressSync(
        [Buffer.from("channel_state"), ATTENTION_MINT.toBuffer(), sha3Subject.toBuffer()],
        programId
      );
      return (
        `Hint: If you accidentally used SHA3-256 (FIPS-202) instead of Keccak-256, you'd derive:\n` +
        `- subject_id (SHA3-256): ${sha3Subject.toBase58()} (hex=${sha3Hex})\n` +
        `- channel_state PDA (SHA3-256): ${sha3ChannelState.toBase58()}`
      );
    })();

    throw new Error(
      `Preflight failed: ChannelState not found at ${channelState.toBase58()}.\n` +
        `- Expected seeds: ["channel_state", mint, subject_id]\n` +
        `- mint: ${ATTENTION_MINT.toBase58()}\n` +
        `- subject_id (Keccak-256): ${subjectId.toBase58()} (hex=${keccakHex})\n` +
        `${sha3Lines}\n` +
        `Fix: Use Keccak-256 (e.g. js-sha3 keccak256 / sha3::Keccak256) and/or run initialize_channel first.`
    );
  }

  if (!channelInfo.owner.equals(programId)) {
    throw new Error(
      `Preflight failed: ChannelState ${channelState.toBase58()} is owned by ${channelInfo.owner.toBase58()}, expected ${programId.toBase58()}.\n` +
        `Hint: check you're on the right cluster/RPC, and using the correct PROGRAM_ID and mint.`
    );
  }

  const version = channelInfo.data.length >= 9 ? channelInfo.data[8] : undefined;
  if (version !== 1) {
    throw new Error(
      `Preflight failed: ChannelState ${channelState.toBase58()} has version=${version}, expected 1.\n` +
        `Fix: run initialize_channel for "${channel}" (and ensure subject_id uses Keccak-256, not SHA3-256).`
    );
  }

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
