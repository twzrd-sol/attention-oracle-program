/**
 * Deploy channel vaults to mainnet.
 *
 * Initializes vault state + vLOFI metadata for all 16 channels.
 * Channels and stake pools must already exist on-chain.
 *
 * Safety:
 *   - Requires CLUSTER=mainnet-beta + I_UNDERSTAND_MAINNET=1
 *   - Keypair must not be inside repo, perms must be 600/400
 *   - Manual "Type DEPLOY to continue" prompt (skip with CONFIRM=yes)
 *   - Idempotent: skips already-initialized accounts
 *
 * Usage:
 *   CLUSTER=mainnet-beta I_UNDERSTAND_MAINNET=1 \
 *   RPC_URL=https://... KEYPAIR=/secure/admin.json \
 *     npx tsx scripts/deploy-mainnet-vaults.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import BN from "bn.js";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  Connection,
} from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { readFileSync, statSync } from "fs";
import * as path from "path";
import * as url from "url";
import * as readline from "readline";

import { requireScriptEnv } from "./script-guard.js";
import { CCM_V3_MINT, PROGRAM_ID as ORACLE_PROGRAM_ID } from "./config.js";
import { CHANNELS } from "./keepers/lib/channels.js";
import {
  VAULT_PROGRAM_ID,
  METADATA_PROGRAM_ID,
  deriveProtocolState,
  deriveVault,
  deriveVlofiMint,
  deriveCcmBuffer,
  deriveOraclePosition,
  deriveMetadata,
} from "./keepers/lib/vault-pda.js";

// ESM __dirname
const __filename = url.fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const MIN_DEPOSIT = new BN(1_000_000_000); // 1 CCM

// =========================================================================
// Mainnet safety guards
// =========================================================================

function validateKeypairSafety(keypairPath: string): void {
  const repoRoot = path.resolve(__dirname, "..");
  const resolved = path.resolve(keypairPath);

  if (resolved.startsWith(repoRoot + path.sep) || resolved === repoRoot) {
    console.error(`Refusing keypair inside repo: ${resolved}`);
    process.exit(3);
  }

  const mode = statSync(resolved).mode & 0o777;
  if (mode !== 0o600 && mode !== 0o400) {
    console.error(
      `Keypair perms must be 600 or 400 (got ${mode.toString(8)}): ${resolved}`,
    );
    process.exit(5);
  }
}

async function confirmDeploy(): Promise<void> {
  if (process.env.CONFIRM === "yes") {
    console.log("[guard] Auto-confirmed (CONFIRM=yes)");
    return;
  }

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  return new Promise((resolve, reject) => {
    rl.question(
      "[guard] Mainnet vault deploy. Type DEPLOY to continue: ",
      (answer) => {
        rl.close();
        if (answer.trim() === "DEPLOY") {
          resolve();
        } else {
          reject(new Error("Aborted by user"));
        }
      },
    );
  });
}

async function accountExists(
  connection: Connection,
  pubkey: PublicKey,
): Promise<boolean> {
  const info = await connection.getAccountInfo(pubkey);
  return info !== null;
}

// =========================================================================
// Main
// =========================================================================

async function main() {
  const env = requireScriptEnv();

  console.log("=== DEPLOY MAINNET VAULTS ===");
  console.log("Cluster:", env.cluster);
  console.log("Channels:", CHANNELS.length);
  console.log("");

  // Mainnet-specific guards
  if (env.cluster === "mainnet-beta") {
    validateKeypairSafety(env.keypairPath);
    await confirmDeploy();
  }

  // Setup provider
  const keypairData = JSON.parse(readFileSync(env.keypairPath, "utf-8"));
  const adminKeypair = Keypair.fromSecretKey(Uint8Array.from(keypairData));
  const connection = new Connection(env.rpcUrl, "confirmed");
  const wallet = new anchor.Wallet(adminKeypair);
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
    preflightCommitment: "confirmed",
  });
  anchor.setProvider(provider);

  console.log("Admin:", adminKeypair.publicKey.toBase58());
  console.log("CCM Mint:", CCM_V3_MINT.toBase58());
  console.log("");

  // Load IDLs
  const vaultIdl = await Program.fetchIdl(VAULT_PROGRAM_ID, provider);
  if (!vaultIdl) throw new Error("Vault IDL not found on-chain. Deploy vault program first.");
  const vaultProgram = new Program(vaultIdl, provider);

  const protocolState = deriveProtocolState(CCM_V3_MINT);
  const envLines: string[] = [];
  let initialized = 0;
  let skipped = 0;

  for (const ch of CHANNELS) {
    const channelConfig = new PublicKey(ch.channelConfig);
    const vault = deriveVault(channelConfig);
    const vlofiMint = deriveVlofiMint(vault);
    const ccmBuffer = deriveCcmBuffer(vault);
    const oraclePosition = deriveOraclePosition(vault);
    const metadata = deriveMetadata(vlofiMint);

    console.log(`--- ${ch.name} ---`);

    // Step 1: Initialize vault
    if (await accountExists(connection, vault)) {
      console.log("  [SKIP] Vault exists");
      skipped++;
    } else {
      console.log(
        `  [INIT] Vault (lock=${ch.lockDurationSlots} slots, queue=${ch.withdrawQueueSlots} slots)`,
      );
      const tx = await vaultProgram.methods
        .initializeVault(
          MIN_DEPOSIT,
          new BN(ch.lockDurationSlots),
          new BN(ch.withdrawQueueSlots),
        )
        .accounts({
          admin: adminKeypair.publicKey,
          oracleProtocol: protocolState,
          oracleChannelConfig: channelConfig,
          ccmMint: CCM_V3_MINT,
          vault,
          ccmBuffer,
          vlofiMint,
          vaultOraclePosition: oraclePosition,
          token2022Program: TOKEN_2022_PROGRAM_ID,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          rent: SYSVAR_RENT_PUBKEY,
        })
        .rpc();
      console.log("  tx:", tx);
      initialized++;
    }

    // Step 2: Set metadata
    if (await accountExists(connection, metadata)) {
      console.log("  [SKIP] Metadata exists");
    } else {
      console.log(`  [META] Setting metadata: "${ch.label}"`);
      const tx = await vaultProgram.methods
        .setVlofiMetadata(ch.label, "vLOFI", "")
        .accounts({
          admin: adminKeypair.publicKey,
          vault,
          vlofiMint,
          metadata,
          metadataProgram: METADATA_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
      console.log("  tx:", tx);
    }

    // Collect env output
    const prefix = ch.name.toUpperCase().replace(/-/g, "_");
    envLines.push(`${prefix}_CHANNEL_CONFIG=${channelConfig.toBase58()}`);
    envLines.push(`${prefix}_VAULT=${vault.toBase58()}`);
    envLines.push(`${prefix}_VLOFI_MINT=${vlofiMint.toBase58()}`);
    envLines.push(`${prefix}_CCM_BUFFER=${ccmBuffer.toBase58()}`);
    envLines.push(`${prefix}_ORACLE_POSITION=${oraclePosition.toBase58()}`);

    // Rate-limit between channels
    await new Promise((r) => setTimeout(r, 1000));
  }

  console.log("");
  console.log(`=== SUMMARY: ${initialized} initialized, ${skipped} skipped ===`);
  console.log("");
  console.log("=== BACKEND CONFIG (copy to .env) ===");
  console.log(`VAULT_PROGRAM_ID=${VAULT_PROGRAM_ID.toBase58()}`);
  console.log(`ORACLE_PROGRAM_ID=${ORACLE_PROGRAM_ID.toBase58()}`);
  console.log(`CCM_MINT=${CCM_V3_MINT.toBase58()}`);
  envLines.forEach((l) => console.log(l));
  console.log("");
  console.log("=== DONE ===");
}

main().catch((err) => {
  console.error("FATAL:", err.message || err);
  if (err.logs) err.logs.slice(-10).forEach((l: string) => console.error("  ", l));
  process.exit(1);
});
