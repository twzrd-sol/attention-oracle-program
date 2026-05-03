/**
 * Safely set reward_rate_per_slot on a wzrd-rails StakePool.
 *
 * Default mode is dry-run: derive accounts, verify authority, sign, and simulate.
 * Broadcast requires BROADCAST=1 or --send plus an explicit confirmation phrase.
 *
 * Usage:
 *   CLUSTER=devnet RPC_URL="https://..." KEYPAIR=/path/admin.json POOL_ID=0 NEW_RATE=1000 \
 *     npx tsx scripts/set-reward-rate.ts
 *
 *   CLUSTER=mainnet-beta RPC_URL="https://..." KEYPAIR=/path/admin.json POOL_ID=0 NEW_RATE=1000 \
 *     BROADCAST=1 I_UNDERSTAND_MAINNET=1 npx tsx scripts/set-reward-rate.ts
 */

import {
  Connection,
  Keypair,
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { createHash } from "node:crypto";
import * as fs from "node:fs";
import * as path from "node:path";
import * as readline from "node:readline/promises";

const RAILS_PROGRAM = new PublicKey("BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9");
const CONFIG_SEED = Buffer.from("config");
const POOL_SEED = Buffer.from("pool");
const MAX_REWARD_RATE_PER_SLOT = 1_000_000n;
const ALLOWED_CLUSTERS = new Set(["localnet", "devnet", "testnet", "mainnet-beta"]);
const RATE_OFFSET = 8 + 4 + 8 + 16;

function fail(message: string): never {
  console.error(`ERROR: ${message}`);
  process.exit(2);
}

function env(name: string): string {
  return process.env[name]?.trim() ?? "";
}

function requireEnv(name: string): string {
  const value = env(name);
  if (!value) fail(`Missing ${name}`);
  return value;
}

function normalizeCluster(raw: string): string {
  const cluster = raw === "mainnet" ? "mainnet-beta" : raw;
  if (!ALLOWED_CLUSTERS.has(cluster)) {
    fail(`Invalid CLUSTER=${raw}. Use localnet, devnet, testnet, or mainnet-beta`);
  }
  return cluster;
}

function expandHome(inputPath: string): string {
  if (inputPath === "~") return process.env.HOME ?? "";
  if (inputPath.startsWith("~/")) return path.join(process.env.HOME ?? "", inputPath.slice(2));
  return inputPath;
}

function loadKeypair(keypairPath: string): Keypair {
  const expanded = expandHome(keypairPath);
  if (!expanded || !fs.existsSync(expanded)) fail(`KEYPAIR not found: ${expanded}`);

  const raw = JSON.parse(fs.readFileSync(expanded, "utf8"));
  if (!Array.isArray(raw)) fail(`KEYPAIR must be a Solana secret-key JSON array: ${expanded}`);

  return Keypair.fromSecretKey(new Uint8Array(raw));
}

function parsePoolId(): number {
  const raw = env("POOL_ID") || "0";
  if (!/^\d+$/.test(raw)) fail(`POOL_ID must be a non-negative integer, got ${raw}`);

  const poolId = Number(raw);
  if (!Number.isSafeInteger(poolId) || poolId < 0 || poolId > 0xffffffff) {
    fail(`POOL_ID must fit in u32, got ${raw}`);
  }
  return poolId;
}

function parseNewRate(): bigint {
  const raw = requireEnv("NEW_RATE");
  if (!/^\d+$/.test(raw)) fail(`NEW_RATE must be a non-negative integer, got ${raw}`);

  const rate = BigInt(raw);
  if (rate > MAX_REWARD_RATE_PER_SLOT) {
    fail(`NEW_RATE ${rate} exceeds on-chain max ${MAX_REWARD_RATE_PER_SLOT}`);
  }
  return rate;
}

function poolIdBytes(poolId: number): Buffer {
  const bytes = Buffer.alloc(4);
  bytes.writeUInt32LE(poolId, 0);
  return bytes;
}

function anchorDiscriminator(ixName: string): Buffer {
  return createHash("sha256").update(`global:${ixName}`).digest().subarray(0, 8);
}

function readStakePoolRate(data: Buffer): bigint {
  if (data.length < RATE_OFFSET + 8) {
    fail(`StakePool account is too short: ${data.length} bytes`);
  }
  return data.readBigUInt64LE(RATE_OFFSET);
}

function readConfigAdmin(data: Buffer): PublicKey {
  const adminOffset = 8;
  if (data.length < adminOffset + 32) {
    fail(`Config account is too short: ${data.length} bytes`);
  }
  return new PublicKey(data.subarray(adminOffset, adminOffset + 32));
}

function shouldBroadcast(): boolean {
  return env("BROADCAST") === "1" || process.argv.includes("--send");
}

async function confirmBroadcast(cluster: string, poolId: number, newRate: bigint): Promise<void> {
  const confirmationToken = `${cluster}:${poolId}:${newRate.toString()}`;
  if (env("CONFIRM_BROADCAST") === confirmationToken) {
    return;
  }

  if (cluster === "mainnet-beta" && env("I_UNDERSTAND_MAINNET") !== "1") {
    fail("Refusing mainnet broadcast without I_UNDERSTAND_MAINNET=1");
  }

  if (!process.stdin.isTTY) {
    fail(`Non-interactive broadcast requires CONFIRM_BROADCAST=${confirmationToken}`);
  }

  const phrase = `set ${confirmationToken}`;
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  try {
    const answer = await rl.question(`Type "${phrase}" to broadcast: `);
    if (answer.trim() !== phrase) fail("Confirmation phrase did not match; aborting");
  } finally {
    rl.close();
  }
}

async function main(): Promise<void> {
  const cluster = normalizeCluster(requireEnv("CLUSTER"));
  const rpcUrl = env("RPC_URL") || env("SOLANA_RPC_URL");
  if (!rpcUrl) fail("Missing RPC_URL or SOLANA_RPC_URL");

  const keypairPath = requireEnv("KEYPAIR");
  const poolId = parsePoolId();
  const newRate = parseNewRate();
  const broadcast = shouldBroadcast();

  const connection = new Connection(rpcUrl, "confirmed");
  const admin = loadKeypair(keypairPath);

  const [configPda] = PublicKey.findProgramAddressSync([CONFIG_SEED], RAILS_PROGRAM);
  const [poolPda] = PublicKey.findProgramAddressSync([POOL_SEED, poolIdBytes(poolId)], RAILS_PROGRAM);

  console.log("\nwzrd-rails set_reward_rate");
  console.log(`  Mode    : ${broadcast ? "broadcast" : "dry-run simulation only"}`);
  console.log(`  Cluster : ${cluster}`);
  console.log(`  Program : ${RAILS_PROGRAM.toBase58()}`);
  console.log(`  Admin   : ${admin.publicKey.toBase58()}`);
  console.log(`  Pool ID : ${poolId}`);
  console.log(`  New rate: ${newRate} base units/slot`);
  console.log(`  Config  : ${configPda.toBase58()}`);
  console.log(`  Pool    : ${poolPda.toBase58()}`);

  const [configAcct, poolAcct] = await Promise.all([
    connection.getAccountInfo(configPda, "confirmed"),
    connection.getAccountInfo(poolPda, "confirmed"),
  ]);
  if (!configAcct) fail("Config account not found on-chain");
  if (!poolAcct) fail("Pool account not found on-chain");
  if (!configAcct.owner.equals(RAILS_PROGRAM)) fail(`Config owner mismatch: ${configAcct.owner.toBase58()}`);
  if (!poolAcct.owner.equals(RAILS_PROGRAM)) fail(`Pool owner mismatch: ${poolAcct.owner.toBase58()}`);

  const configuredAdmin = readConfigAdmin(configAcct.data);
  if (!configuredAdmin.equals(admin.publicKey)) {
    fail(`KEYPAIR pubkey is not config admin. Expected ${configuredAdmin.toBase58()}`);
  }

  const currentRate = readStakePoolRate(poolAcct.data);
  console.log(`\n  Current on-chain rate: ${currentRate} base units/slot`);
  if (currentRate === newRate) {
    console.log("  Rate already equals target. No transaction needed.");
    return;
  }

  const rateBytes = Buffer.alloc(8);
  rateBytes.writeBigUInt64LE(newRate, 0);
  const ixData = Buffer.concat([anchorDiscriminator("set_reward_rate"), poolIdBytes(poolId), rateBytes]);

  const ix = new TransactionInstruction({
    programId: RAILS_PROGRAM,
    keys: [
      { pubkey: configPda, isSigner: false, isWritable: false },
      { pubkey: poolPda, isSigner: false, isWritable: true },
      { pubkey: admin.publicKey, isSigner: true, isWritable: false },
    ],
    data: ixData,
  });

  const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash("confirmed");
  const message = new TransactionMessage({
    payerKey: admin.publicKey,
    recentBlockhash: blockhash,
    instructions: [ix],
  }).compileToV0Message();

  const tx = new VersionedTransaction(message);
  tx.sign([admin]);

  console.log("\nSimulating transaction...");
  const simulation = await connection.simulateTransaction(tx, {
    commitment: "confirmed",
    sigVerify: true,
  });
  if (simulation.value.logs?.length) {
    for (const line of simulation.value.logs) console.log(`  ${line}`);
  }
  if (simulation.value.err) {
    console.error("Simulation failed:", JSON.stringify(simulation.value.err));
    process.exit(1);
  }
  console.log("  Simulation passed.");

  if (!broadcast) {
    console.log("\nDRY RUN ONLY. No transaction sent.");
    console.log(`To broadcast, rerun with BROADCAST=1 and confirm token ${cluster}:${poolId}:${newRate}`);
    return;
  }

  await confirmBroadcast(cluster, poolId, newRate);

  console.log("\nSending transaction...");
  const signature = await connection.sendTransaction(tx, { skipPreflight: false });
  console.log(`  Signature: ${signature}`);

  const confirmed = await connection.confirmTransaction(
    { signature, blockhash, lastValidBlockHeight },
    "confirmed",
  );
  if (confirmed.value.err) {
    console.error("Transaction failed:", JSON.stringify(confirmed.value.err));
    process.exit(1);
  }

  const poolAfter = await connection.getAccountInfo(poolPda, "finalized");
  if (!poolAfter) fail("Pool account missing after confirmed transaction");
  const rateAfter = readStakePoolRate(poolAfter.data);
  console.log(`\n  On-chain rate after: ${rateAfter} base units/slot`);
  if (rateAfter !== newRate) {
    console.error(`MISMATCH: expected ${newRate}, got ${rateAfter}`);
    process.exit(1);
  }

  console.log("  SUCCESS: reward rate updated.");
  console.log(`\nExplorer: https://solscan.io/tx/${signature}${cluster === "mainnet-beta" ? "" : `?cluster=${cluster}`}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
