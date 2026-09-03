/**
 * Safely rotate the wzrd-rails Config admin via set_admin.
 *
 * Modeled on scripts/set-reward-rate.ts: same env contract, same dry-run
 * default, same confirmation gate. Default mode derives accounts, verifies the
 * signer really is the current on-chain admin, signs, and simulates. Broadcast
 * requires BROADCAST=1 or --send plus an explicit confirmation phrase, and on
 * mainnet also I_UNDERSTAND_MAINNET=1.
 *
 * set_admin is SINGLE STEP. There is no pending_admin on Config and no accept
 * leg to catch a mistake -- the two-step set_payout_admin / accept_payout_admin
 * pair belongs to PayoutAuthorityConfig, a different account. Once this lands,
 * only NEW_ADMIN can administer the config. Confirm you control NEW_ADMIN
 * before broadcasting. See docs/playbooks/wzrd-rails-config-admin-rotation.md.
 *
 * Usage:
 *   CLUSTER=mainnet-beta RPC_URL="https://..." KEYPAIR=~/.config/solana/id.json \
 *     NEW_ADMIN=<pubkey> npx tsx scripts/set-admin.ts
 *
 *   ... BROADCAST=1 I_UNDERSTAND_MAINNET=1 npx tsx scripts/set-admin.ts
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
const ALLOWED_CLUSTERS = new Set(["localnet", "devnet", "testnet", "mainnet-beta"]);

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

function parseNewAdmin(): PublicKey {
  const raw = requireEnv("NEW_ADMIN");
  let key: PublicKey;
  try {
    key = new PublicKey(raw);
  } catch {
    return fail(`NEW_ADMIN is not a valid pubkey: ${raw}`);
  }
  // Mirrors the on-chain guard (audit M-3 / EZ-7): all-zeros permanently
  // retires the admin role and is recoverable only via program upgrade.
  if (key.equals(PublicKey.default)) fail("NEW_ADMIN is the all-zeros pubkey; refusing");
  return key;
}

function anchorDiscriminator(ixName: string): Buffer {
  return createHash("sha256").update(`global:${ixName}`).digest().subarray(0, 8);
}

function readConfigAdmin(data: Buffer): PublicKey {
  const adminOffset = 8;
  if (data.length < adminOffset + 32) fail(`Config account is too short: ${data.length} bytes`);
  return new PublicKey(data.subarray(adminOffset, adminOffset + 32));
}

function shouldBroadcast(): boolean {
  return env("BROADCAST") === "1" || process.argv.includes("--send");
}

async function confirmBroadcast(cluster: string, newAdmin: PublicKey): Promise<void> {
  const confirmationToken = `${cluster}:${newAdmin.toBase58()}`;
  if (env("CONFIRM_BROADCAST") === confirmationToken) return;

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
  const newAdmin = parseNewAdmin();
  const broadcast = shouldBroadcast();

  const connection = new Connection(rpcUrl, "confirmed");
  const admin = loadKeypair(keypairPath);
  const [configPda] = PublicKey.findProgramAddressSync([CONFIG_SEED], RAILS_PROGRAM);

  console.log("\nwzrd-rails set_admin (SINGLE STEP - no accept leg)");
  console.log(`  Mode      : ${broadcast ? "broadcast" : "dry-run simulation only"}`);
  console.log(`  Cluster   : ${cluster}`);
  console.log(`  Program   : ${RAILS_PROGRAM.toBase58()}`);
  console.log(`  Config    : ${configPda.toBase58()}`);
  console.log(`  Signer    : ${admin.publicKey.toBase58()}`);
  console.log(`  New admin : ${newAdmin.toBase58()}`);

  const configAcct = await connection.getAccountInfo(configPda, "confirmed");
  if (!configAcct) fail("Config account not found on-chain");
  if (!configAcct.owner.equals(RAILS_PROGRAM)) {
    fail(`Config owner mismatch: ${configAcct.owner.toBase58()}`);
  }

  const configuredAdmin = readConfigAdmin(configAcct.data);
  console.log(`\n  Current on-chain admin: ${configuredAdmin.toBase58()}`);
  if (!configuredAdmin.equals(admin.publicKey)) {
    fail(`KEYPAIR pubkey is not config admin. Expected ${configuredAdmin.toBase58()}`);
  }
  if (configuredAdmin.equals(newAdmin)) {
    console.log("  Admin already equals target. No transaction needed.");
    return;
  }

  const ixData = Buffer.concat([anchorDiscriminator("set_admin"), newAdmin.toBuffer()]);
  const ix = new TransactionInstruction({
    programId: RAILS_PROGRAM,
    keys: [
      { pubkey: configPda, isSigner: false, isWritable: true },
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
    console.log(`To broadcast, rerun with BROADCAST=1 and confirm token ${cluster}:${newAdmin.toBase58()}`);
    return;
  }

  await confirmBroadcast(cluster, newAdmin);

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

  const configAfter = await connection.getAccountInfo(configPda, "finalized");
  if (!configAfter) fail("Config account missing after confirmed transaction");
  const adminAfter = readConfigAdmin(configAfter.data);
  console.log(`\n  On-chain admin after: ${adminAfter.toBase58()}`);
  if (!adminAfter.equals(newAdmin)) {
    console.error(`MISMATCH: expected ${newAdmin.toBase58()}, got ${adminAfter.toBase58()}`);
    process.exit(1);
  }

  console.log("  SUCCESS: config admin rotated.");
  console.log(`\nExplorer: https://solscan.io/tx/${signature}${cluster === "mainnet-beta" ? "" : `?cluster=${cluster}`}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
