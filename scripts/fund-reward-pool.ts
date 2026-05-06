/**
 * Safely fund a wzrd-rails reward vault with CCM.
 *
 * Default mode is dry-run: derive accounts, verify ownership/balances, sign, and simulate.
 * Broadcast requires BROADCAST=1 or --send plus an explicit confirmation phrase.
 *
 * Usage:
 *   CLUSTER=devnet RPC_URL="https://..." KEYPAIR=/path/funder.json POOL_ID=0 \
 *     AMOUNT_BASE_UNITS=1000000000 npx tsx scripts/fund-reward-pool.ts
 *
 *   CLUSTER=mainnet-beta RPC_URL="https://..." KEYPAIR=/path/funder.json POOL_ID=0 \
 *     AMOUNT_BASE_UNITS=100000000000 BROADCAST=1 I_UNDERSTAND_MAINNET=1 \
 *     CONFIRM_BROADCAST=fund:mainnet-beta:0:100000000000 \
 *     npx tsx scripts/fund-reward-pool.ts
 *
 * Optional:
 *   FUNDER_CCM_ACCOUNT=<token-account-pubkey>
 *
 * If FUNDER_CCM_ACCOUNT is omitted, the script derives the funder's Token-2022 ATA.
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
const EXPECTED_CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");
const TOKEN_2022_PROGRAM = new PublicKey("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
const ASSOCIATED_TOKEN_PROGRAM = new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

const CONFIG_SEED = Buffer.from("config");
const POOL_SEED = Buffer.from("pool");
const REWARD_VAULT_SEED = Buffer.from("reward_vault");
const ALLOWED_CLUSTERS = new Set(["localnet", "devnet", "testnet", "mainnet-beta"]);
const U64_MAX = (1n << 64n) - 1n;

const CONFIG_ADMIN_OFFSET = 8;
const CONFIG_CCM_MINT_OFFSET = CONFIG_ADMIN_OFFSET + 32;
const TOKEN_ACCOUNT_MINT_OFFSET = 0;
const TOKEN_ACCOUNT_OWNER_OFFSET = 32;
const TOKEN_ACCOUNT_AMOUNT_OFFSET = 64;
const TOKEN_ACCOUNT_MIN_LEN = TOKEN_ACCOUNT_AMOUNT_OFFSET + 8;

type TokenAccountView = {
  mint: PublicKey;
  owner: PublicKey;
  amount: bigint;
};

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

function parseAmount(): bigint {
  const raw = requireEnv("AMOUNT_BASE_UNITS");
  if (!/^\d+$/.test(raw)) fail(`AMOUNT_BASE_UNITS must be a positive integer, got ${raw}`);

  const amount = BigInt(raw);
  if (amount <= 0n) fail("AMOUNT_BASE_UNITS must be greater than 0");
  if (amount > U64_MAX) fail(`AMOUNT_BASE_UNITS exceeds u64 max: ${raw}`);
  return amount;
}

function parseOptionalPublicKey(name: string): PublicKey | null {
  const raw = env(name);
  if (!raw) return null;
  try {
    return new PublicKey(raw);
  } catch {
    fail(`${name} is not a valid public key: ${raw}`);
  }
}

function poolIdBytes(poolId: number): Buffer {
  const bytes = Buffer.alloc(4);
  bytes.writeUInt32LE(poolId, 0);
  return bytes;
}

function u64Bytes(value: bigint): Buffer {
  const bytes = Buffer.alloc(8);
  bytes.writeBigUInt64LE(value, 0);
  return bytes;
}

function anchorDiscriminator(ixName: string): Buffer {
  return createHash("sha256").update(`global:${ixName}`).digest().subarray(0, 8);
}

function readConfigAdmin(data: Buffer): PublicKey {
  if (data.length < CONFIG_ADMIN_OFFSET + 32) {
    fail(`Config account is too short: ${data.length} bytes`);
  }
  return new PublicKey(data.subarray(CONFIG_ADMIN_OFFSET, CONFIG_ADMIN_OFFSET + 32));
}

function readConfigCcmMint(data: Buffer): PublicKey {
  if (data.length < CONFIG_CCM_MINT_OFFSET + 32) {
    fail(`Config account is too short: ${data.length} bytes`);
  }
  return new PublicKey(data.subarray(CONFIG_CCM_MINT_OFFSET, CONFIG_CCM_MINT_OFFSET + 32));
}

function readTokenAccount(data: Buffer, label: string): TokenAccountView {
  if (data.length < TOKEN_ACCOUNT_MIN_LEN) {
    fail(`${label} token account is too short: ${data.length} bytes`);
  }

  return {
    mint: new PublicKey(data.subarray(TOKEN_ACCOUNT_MINT_OFFSET, TOKEN_ACCOUNT_MINT_OFFSET + 32)),
    owner: new PublicKey(data.subarray(TOKEN_ACCOUNT_OWNER_OFFSET, TOKEN_ACCOUNT_OWNER_OFFSET + 32)),
    amount: data.readBigUInt64LE(TOKEN_ACCOUNT_AMOUNT_OFFSET),
  };
}

function shouldBroadcast(): boolean {
  return env("BROADCAST") === "1" || process.argv.includes("--send");
}

async function confirmBroadcast(cluster: string, poolId: number, amount: bigint): Promise<void> {
  const confirmationToken = `fund:${cluster}:${poolId}:${amount.toString()}`;
  if (env("CONFIRM_BROADCAST") === confirmationToken) {
    return;
  }

  if (cluster === "mainnet-beta" && env("I_UNDERSTAND_MAINNET") !== "1") {
    fail("Refusing mainnet broadcast without I_UNDERSTAND_MAINNET=1");
  }

  if (!process.stdin.isTTY) {
    fail(`Non-interactive broadcast requires CONFIRM_BROADCAST=${confirmationToken}`);
  }

  const phrase = `fund ${confirmationToken}`;
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
  const amount = parseAmount();
  const broadcast = shouldBroadcast();

  const connection = new Connection(rpcUrl, "confirmed");
  const funder = loadKeypair(keypairPath);

  const [configPda] = PublicKey.findProgramAddressSync([CONFIG_SEED], RAILS_PROGRAM);
  const [poolPda] = PublicKey.findProgramAddressSync([POOL_SEED, poolIdBytes(poolId)], RAILS_PROGRAM);
  const [rewardVaultPda] = PublicKey.findProgramAddressSync(
    [REWARD_VAULT_SEED, poolPda.toBuffer()],
    RAILS_PROGRAM,
  );
  const derivedFunderAta = PublicKey.findProgramAddressSync(
    [funder.publicKey.toBuffer(), TOKEN_2022_PROGRAM.toBuffer(), EXPECTED_CCM_MINT.toBuffer()],
    ASSOCIATED_TOKEN_PROGRAM,
  )[0];
  const funderCcm = parseOptionalPublicKey("FUNDER_CCM_ACCOUNT") ?? derivedFunderAta;

  console.log("\nwzrd-rails fund_reward_pool");
  console.log(`  Mode       : ${broadcast ? "broadcast" : "dry-run simulation only"}`);
  console.log(`  Cluster    : ${cluster}`);
  console.log(`  Program    : ${RAILS_PROGRAM.toBase58()}`);
  console.log(`  Funder     : ${funder.publicKey.toBase58()}`);
  console.log(`  Pool ID    : ${poolId}`);
  console.log(`  Amount     : ${amount} base units`);
  console.log(`  Config     : ${configPda.toBase58()}`);
  console.log(`  Pool       : ${poolPda.toBase58()}`);
  console.log(`  RewardVault: ${rewardVaultPda.toBase58()}`);
  console.log(`  Funder CCM : ${funderCcm.toBase58()}${funderCcm.equals(derivedFunderAta) ? " (derived ATA)" : ""}`);

  const [configAcct, poolAcct, funderCcmAcct, rewardVaultAcct] = await Promise.all([
    connection.getAccountInfo(configPda, "confirmed"),
    connection.getAccountInfo(poolPda, "confirmed"),
    connection.getAccountInfo(funderCcm, "confirmed"),
    connection.getAccountInfo(rewardVaultPda, "confirmed"),
  ]);

  if (!configAcct) fail("Config account not found on-chain");
  if (!poolAcct) fail("Pool account not found on-chain");
  if (!funderCcmAcct) fail("Funder CCM token account not found on-chain");
  if (!rewardVaultAcct) fail("Reward vault token account not found on-chain");
  if (!configAcct.owner.equals(RAILS_PROGRAM)) fail(`Config owner mismatch: ${configAcct.owner.toBase58()}`);
  if (!poolAcct.owner.equals(RAILS_PROGRAM)) fail(`Pool owner mismatch: ${poolAcct.owner.toBase58()}`);
  if (!funderCcmAcct.owner.equals(TOKEN_2022_PROGRAM)) {
    fail(`Funder CCM account owner is not Token-2022: ${funderCcmAcct.owner.toBase58()}`);
  }
  if (!rewardVaultAcct.owner.equals(TOKEN_2022_PROGRAM)) {
    fail(`Reward vault owner is not Token-2022: ${rewardVaultAcct.owner.toBase58()}`);
  }

  const configuredAdmin = readConfigAdmin(configAcct.data);
  const ccmMint = readConfigCcmMint(configAcct.data);
  if (!ccmMint.equals(EXPECTED_CCM_MINT)) {
    fail(`Config CCM mint mismatch: expected ${EXPECTED_CCM_MINT.toBase58()}, got ${ccmMint.toBase58()}`);
  }

  const funderToken = readTokenAccount(funderCcmAcct.data, "Funder CCM");
  const rewardToken = readTokenAccount(rewardVaultAcct.data, "Reward vault");
  if (!funderToken.mint.equals(ccmMint)) fail(`Funder CCM mint mismatch: ${funderToken.mint.toBase58()}`);
  if (!funderToken.owner.equals(funder.publicKey)) {
    fail(`Funder CCM owner mismatch: expected ${funder.publicKey.toBase58()}, got ${funderToken.owner.toBase58()}`);
  }
  if (!rewardToken.mint.equals(ccmMint)) fail(`Reward vault mint mismatch: ${rewardToken.mint.toBase58()}`);
  if (!rewardToken.owner.equals(poolPda)) {
    fail(`Reward vault authority mismatch: expected pool ${poolPda.toBase58()}, got ${rewardToken.owner.toBase58()}`);
  }
  if (funderToken.amount < amount) {
    fail(`Insufficient CCM: funder has ${funderToken.amount}, requested ${amount} base units`);
  }

  console.log(`\n  Config admin           : ${configuredAdmin.toBase58()}`);
  console.log(`  CCM mint               : ${ccmMint.toBase58()}`);
  console.log(`  Funder CCM balance     : ${funderToken.amount} base units`);
  console.log(`  Reward vault prebalance: ${rewardToken.amount} base units`);

  const ixData = Buffer.concat([
    anchorDiscriminator("fund_reward_pool"),
    poolIdBytes(poolId),
    u64Bytes(amount),
  ]);

  const ix = new TransactionInstruction({
    programId: RAILS_PROGRAM,
    keys: [
      { pubkey: configPda, isSigner: false, isWritable: false },
      { pubkey: poolPda, isSigner: false, isWritable: false },
      { pubkey: funder.publicKey, isSigner: true, isWritable: true },
      { pubkey: ccmMint, isSigner: false, isWritable: false },
      { pubkey: funderCcm, isSigner: false, isWritable: true },
      { pubkey: rewardVaultPda, isSigner: false, isWritable: true },
      { pubkey: TOKEN_2022_PROGRAM, isSigner: false, isWritable: false },
    ],
    data: ixData,
  });

  const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash("confirmed");
  const message = new TransactionMessage({
    payerKey: funder.publicKey,
    recentBlockhash: blockhash,
    instructions: [ix],
  }).compileToV0Message();

  const tx = new VersionedTransaction(message);
  tx.sign([funder]);

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
    console.log(`To broadcast, rerun with BROADCAST=1 and confirm token fund:${cluster}:${poolId}:${amount}`);
    return;
  }

  await confirmBroadcast(cluster, poolId, amount);

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

  const [funderAfterAcct, rewardAfterAcct] = await Promise.all([
    connection.getAccountInfo(funderCcm, "finalized"),
    connection.getAccountInfo(rewardVaultPda, "finalized"),
  ]);
  if (!funderAfterAcct) fail("Funder CCM account missing after confirmed transaction");
  if (!rewardAfterAcct) fail("Reward vault missing after confirmed transaction");

  const funderAfter = readTokenAccount(funderAfterAcct.data, "Funder CCM after");
  const rewardAfter = readTokenAccount(rewardAfterAcct.data, "Reward vault after");
  const debited = funderToken.amount - funderAfter.amount;
  const credited = rewardAfter.amount - rewardToken.amount;

  console.log(`\n  Funder debited       : ${debited} base units`);
  console.log(`  Reward vault credited: ${credited} base units`);
  if (credited <= 0n) fail("Reward vault credited amount was 0");

  console.log("  SUCCESS: reward pool funded.");
  console.log(`\nExplorer: https://solscan.io/tx/${signature}${cluster === "mainnet-beta" ? "" : `?cluster=${cluster}`}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
