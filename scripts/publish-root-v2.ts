#!/usr/bin/env ts-node

/**
 * Publish CUMULATIVE merkle roots (V2) for channel epochs to mainnet.
 *
 * This script calls `publish_cumulative_root` on the V2 contract.
 * Use this to manually advance the merkle root if the aggregator is stuck.
 *
 * Usage:
 *   CLUSTER=mainnet-beta RPC_URL=... KEYPAIR=~/.config/solana/amm-admin.json \
 *   ts-node scripts/publish-cumulative-root.ts <channel> <root_seq> <root_hex> <dataset_hash_hex>
 *
 * Example:
 *   ts-node scripts/publish-cumulative-root.ts youtube_lofi 50 97fbf... 12345...
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
import path from "path";
import { createHash } from "crypto";
import jsSha3 from "js-sha3";
// @ts-ignore
const { keccak256 } = jsSha3;

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const SYSTEM_PROGRAM = new PublicKey("11111111111111111111111111111111");

// Seeds from constants.rs
const PROTOCOL_SEED = Buffer.from("protocol");
const CHANNEL_CONFIG_V2_SEED = Buffer.from("channel_cfg_v2");

// Compute discriminator for global:publish_cumulative_root
// anchor discriminator is first 8 bytes of sha256("global:<instr_name>")
function getDiscriminator(name: string): Buffer {
  const hash = createHash("sha256").update(`global:${name}`).digest();
  return hash.slice(0, 8);
}

const PUBLISH_CUMULATIVE_ROOT_DISCRIMINATOR = getDiscriminator("publish_cumulative_root");

const ALLOWED_CLUSTERS = new Set(["localnet", "devnet", "testnet", "mainnet-beta"]);

function normalizeCluster(input: string): string {
  const trimmed = input.trim();
  if (!trimmed) return trimmed;
  if (trimmed === "mainnet") return "mainnet-beta";
  return trimmed;
}

function expandHome(p: string): string {
  if (!p) return p;
  if (p.startsWith("~")) {
    const home = process.env.HOME || "";
    return path.join(home, p.slice(1));
  }
  return p;
}

export type ScriptEnv = {
  cluster: string;
  rpcUrl: string;
  keypairPath: string;
};

export function requireScriptEnv(): ScriptEnv {
  const rawCluster = process.env.CLUSTER || "";
  if (!rawCluster.trim()) {
    console.error("❌ Missing CLUSTER. Set CLUSTER=localnet|devnet|testnet|mainnet-beta");
    process.exit(2);
  }
  const cluster = normalizeCluster(rawCluster);
  if (!ALLOWED_CLUSTERS.has(cluster)) {
    console.error(`❌ Invalid CLUSTER: ${cluster}. Use localnet|devnet|testnet|mainnet-beta`);
    process.exit(2);
  }

  const rawKeypair = process.env.KEYPAIR || process.env.ANCHOR_WALLET || "";
  if (!rawKeypair.trim()) {
    console.error("❌ Missing KEYPAIR. Set KEYPAIR=/path/to/keypair.json");
    process.exit(2);
  }
  const keypairPath = expandHome(rawKeypair.trim());
  if (!fs.existsSync(keypairPath)) {
    console.error(`❌ Keypair not found: ${keypairPath}`);
    process.exit(2);
  }

  const rpcUrl =
    process.env.RPC_URL ||
    process.env.ANCHOR_PROVIDER_URL ||
    process.env.AO_RPC_URL ||
    process.env.SYNDICA_RPC ||
    process.env.SOLANA_RPC ||
    process.env.SOLANA_URL ||
    "";

  if (!rpcUrl.trim()) {
    console.error("❌ Missing RPC URL. Set RPC_URL or ANCHOR_PROVIDER_URL");
    process.exit(2);
  }

  return { cluster, rpcUrl: rpcUrl.trim(), keypairPath };
}


// Derive subject_id using keccak256("channel:", lowercase(channel))
function deriveSubjectId(channel: string): PublicKey {
  const lower = channel.toLowerCase();
  const input = Buffer.concat([
    Buffer.from("channel:"),
    Buffer.from(lower),
  ]);
  // @ts-ignore
  const hashHex = keccak256(input);
  const hashBytes = Buffer.from(hashHex, "hex");
  return new PublicKey(hashBytes);
}

// Manual borsh serialization for the instruction args
function serializeArgs(channel: string, rootSeq: bigint, root: Uint8Array, datasetHash: Uint8Array): Buffer {
  // String: u32 LE length + UTF-8 bytes
  const channelUtf8 = Buffer.from(channel, "utf-8");
  const channelLen = Buffer.alloc(4);
  channelLen.writeUInt32LE(channelUtf8.length, 0);

  // u64 LE root_seq
  const seqBuf = Buffer.alloc(8);
  seqBuf.writeBigUInt64LE(rootSeq, 0);

  // [u8; 32] root
  const rootBuf = Buffer.from(root);
  
  // [u8; 32] dataset_hash
  const hashBuf = Buffer.from(datasetHash);

  return Buffer.concat([channelLen, channelUtf8, seqBuf, rootBuf, hashBuf]);
}

async function main() {
  const args = process.argv.slice(2);
  if (args.length < 4) {
    console.error("Usage: ts-node scripts/publish-cumulative-root.ts <channel> <root_seq> <root_hex> <dataset_hash_hex>");
    console.error("Example: ts-node scripts/publish-cumulative-root.ts youtube_lofi 50 97fbf... 0000...");
    process.exit(1);
  }

  const [channel, seqStr, rootHex, datasetHashHex] = args;
  const rootSeq = BigInt(seqStr);

  // Convert hex root to bytes
  const rootBytes = Buffer.from(rootHex, "hex");
  if (rootBytes.length !== 32) {
    console.error("Root must be 32 bytes (64 hex chars)");
    process.exit(1);
  }
  
  const datasetBytes = Buffer.from(datasetHashHex, "hex");
  if (datasetBytes.length !== 32) {
    console.error("Dataset hash must be 32 bytes (64 hex chars)");
    process.exit(1);
  }

  const { rpcUrl, keypairPath } = requireScriptEnv();

  // Load wallet
  const walletPath = keypairPath;
  const wallet = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(walletPath, "utf-8")))
  );

  // Setup connection
  const connection = new Connection(rpcUrl, "confirmed");

  // Default to live v3 CCM token
  const ATTENTION_MINT = new PublicKey(
    process.env.CCM_V3_MINT || process.env.ATTENTION_MINT || "Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM"
  );

  console.log(`Publishing CUMULATIVE root (V2) for channel: ${channel}, seq: ${rootSeq}`);
  console.log(`Root: ${rootHex}`);
  console.log(`Dataset Hash: ${datasetHashHex}`);
  console.log(`Mint: ${ATTENTION_MINT.toString()}`);
  console.log(`Payer: ${wallet.publicKey.toString()}`);

  // Derive PDAs
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, ATTENTION_MINT.toBuffer()],
    PROGRAM_ID
  );

  const subjectId = deriveSubjectId(channel);
  // NOTE: V2 config seed is "channel_cfg_v2"
  const [channelConfigV2] = PublicKey.findProgramAddressSync(
    [CHANNEL_CONFIG_V2_SEED, ATTENTION_MINT.toBuffer(), subjectId.toBuffer()],
    PROGRAM_ID
  );

  console.log(`Protocol State: ${protocolState.toString()}`);
  console.log(`Subject ID: ${subjectId.toString()}`);
  console.log(`Channel Config V2: ${channelConfigV2.toString()}`);

  // Serialize instruction args
  const argsData = serializeArgs(channel, rootSeq, rootBytes, datasetBytes);

  // Build instruction data: discriminator + args
  const data = Buffer.concat([PUBLISH_CUMULATIVE_ROOT_DISCRIMINATOR, argsData]);

  // Build accounts array
  const keys = [
    { pubkey: wallet.publicKey, isSigner: true, isWritable: true }, // payer
    { pubkey: protocolState, isSigner: false, isWritable: false }, // protocol_state (READ ONLY)
    { pubkey: channelConfigV2, isSigner: false, isWritable: true }, // channel_config
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

    console.log(`✓ Cumulative root published successfully`);
    console.log(`  Transaction: ${signature}`);
    console.log(`  View on Solscan: https://solscan.io/tx/${signature}`);
    console.log(`
✅ Channel ${channel} seq ${rootSeq} is now live for claims!`);
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