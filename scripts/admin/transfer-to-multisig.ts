/**
 * Transfer admin authority for Oracle + all 16 Channel Vaults → Squads multisig.
 *
 * This is IRREVERSIBLE. Once transferred, only the Squads multisig can
 * modify admin state or transfer admin again.
 *
 * Safety:
 *   --dry-run (default) — derive all accounts, print plan, exit
 *   --execute           — submit 17 update_admin transactions
 *
 * Usage:
 *   CLUSTER=mainnet-beta I_UNDERSTAND_MAINNET=1 \
 *   RPC_URL=https://... KEYPAIR=~/.config/solana/id.json \
 *   npx tsx scripts/admin/transfer-to-multisig.ts --dry-run
 *
 *   CLUSTER=mainnet-beta I_UNDERSTAND_MAINNET=1 \
 *   RPC_URL=https://... KEYPAIR=~/.config/solana/id.json \
 *   npx tsx scripts/admin/transfer-to-multisig.ts --execute
 */

import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { AnchorProvider, Program, Wallet } from "@coral-xyz/anchor";
import { requireScriptEnv } from "../script-guard";
import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const ORACLE_PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const VAULT_PROGRAM_ID = new PublicKey("5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");
const EXPECTED_CURRENT_ADMIN = new PublicKey("2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD");

// Squads vault PDA (index 0) — the new admin authority.
// This is the system-owned PDA that signs via Squads governance.
const SQUADS_VAULT = new PublicKey("2v9jrkuJM99uf4xDFwfyxuzoNmqfggqbuW34mad2n6kW");

const PROTOCOL_SEED = Buffer.from("protocol");
const VAULT_SEED = Buffer.from("vault");

// Channel registry — all 16 channel configs
const CHANNELS = [
  { name: "vault-01", channelConfig: "J3HAT4NbL6REyyNqbW1BDGF9BXXc3FYuQ1fr6NbCQaoW" },
  { name: "vault-02", channelConfig: "dJvatt5bga4ak64ghTLEtxs1jxfLX4TNoZuvfiDCcGy" },
  { name: "vault-03", channelConfig: "2TWM1H1gHWrA6Ta6A9tH3E1TTTRbPpmSL2Xg7KdHwxCM" },
  { name: "vault-04", channelConfig: "GZL7vAo9vxdNbsmrreVueVd1Xm9oWmatkQauFcxhq8qP" },
  { name: "vault-05", channelConfig: "84SxXryEL2dFT5rno9F1SGBAFvvkEDyp3wNQZyxT3hQ9" },
  { name: "vault-06", channelConfig: "7g1qkWgZkbhZNFgbEzxxvYxCJHt4NMb3fwE2RHyrygDL" },
  { name: "vault-07", channelConfig: "DqoM3QcGPbUD2Hic1fxsSLqZY1CaSDkiaNaas2ufZUpb" },
  { name: "vault-08", channelConfig: "EADvLuoe6ZXTfVBpVEKAMSfnFr1oZuHMxiButLVMnHuE" },
  { name: "vault-09", channelConfig: "HEa4KgAyuvRZPyAsUPmVTRXiTRuxVEkkGbmtEeybzGB9" },
  { name: "vault-10", channelConfig: "9G1MvnVq3dX6UwGFvhTC9bDArNt9TyvS5UimffTL1BAJ" },
  { name: "vault-11", channelConfig: "Dg84d5BkSYxKSix9m6YgbLz1L7mEsSH81Svp24watxEC" },
  { name: "vault-12", channelConfig: "GdrV9DjKZFePZadxuQANKEBvVaB7rM8aEhMEzMHWrFJE" },
  { name: "vault-13", channelConfig: "8LCSiL2a4FjTAveMMn8SjLVxrYecWSfFDH48sdhzdbv" },
  { name: "vault-14", channelConfig: "GxzK9iqyFJf3TRJG5XAQJD3eJtgKCivzkQtj7iPKrUsG" },
  { name: "vault-15", channelConfig: "4JawzmsofxVCim7eDtFPCMwiP21NMcAQqsZRPT7k9uL1" },
  { name: "vault-16", channelConfig: "2uGQDJMsGy3undJCT9NazdJXjSoCcXd71vgkvYzMt3eR" },
];

// ---------------------------------------------------------------------------
// Admin byte parsers (match verify-admin-state.ts)
// ---------------------------------------------------------------------------

function parseProtocolAdmin(data: Buffer): PublicKey {
  const offset = 8 + 1 + 1; // discriminator + is_initialized + version
  return new PublicKey(data.subarray(offset, offset + 32));
}

function parseVaultAdmin(data: Buffer): PublicKey {
  const offset = 8 + 1 + 1 + (32 * 4) + (8 * 6); // = 186
  return new PublicKey(data.subarray(offset, offset + 32));
}

// ---------------------------------------------------------------------------
// IDL loader
// ---------------------------------------------------------------------------

function loadIdl(name: string): any {
  const idlPath = path.resolve(__dirname, `../../target/idl/${name}.json`);
  return JSON.parse(fs.readFileSync(idlPath, "utf-8"));
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main() {
  const execute = process.argv.includes("--execute");
  const dryRun = !execute;

  if (dryRun) {
    console.log("=== DRY RUN — No transactions will be sent ===\n");
  } else {
    console.log("=== LIVE EXECUTION — Transactions will be sent ===\n");
  }

  // --- Environment ---
  const env = requireScriptEnv();
  const keypairData = JSON.parse(fs.readFileSync(env.keypairPath, "utf-8"));
  const payer = Keypair.fromSecretKey(new Uint8Array(keypairData));

  console.log(`Cluster:  ${env.cluster}`);
  console.log(`RPC:      ${env.rpcUrl.substring(0, 60)}...`);
  console.log(`Signer:   ${payer.publicKey.toBase58()}`);
  console.log(`Target:   ${SQUADS_VAULT.toBase58()} (Squads vault PDA)\n`);

  // Verify signer is the current admin
  if (!payer.publicKey.equals(EXPECTED_CURRENT_ADMIN)) {
    console.error(`ERROR: Signer ${payer.publicKey.toBase58()} is not the expected admin ${EXPECTED_CURRENT_ADMIN.toBase58()}`);
    process.exit(1);
  }

  const conn = new Connection(env.rpcUrl, "confirmed");
  const wallet = new Wallet(payer);
  const provider = new AnchorProvider(conn, wallet, { commitment: "confirmed" });

  // Load IDLs
  const oracleIdl = loadIdl("token_2022");
  const vaultIdl = loadIdl("channel_vault");

  const oracleProgram = new Program(oracleIdl, provider);
  const vaultProgram = new Program(vaultIdl, provider);

  // --- Pre-flight: verify all current admins ---
  console.log("--- Pre-flight Verification ---\n");

  // Oracle
  const [protocolPda] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, CCM_MINT.toBuffer()],
    ORACLE_PROGRAM_ID,
  );
  const protocolAcct = await conn.getAccountInfo(protocolPda);
  if (!protocolAcct) {
    console.error("ERROR: ProtocolState not found on-chain!");
    process.exit(1);
  }
  const currentOracleAdmin = parseProtocolAdmin(Buffer.from(protocolAcct.data));
  if (currentOracleAdmin.equals(SQUADS_VAULT)) {
    console.log(`Oracle:  Already transferred to Squads — SKIP`);
  } else if (!currentOracleAdmin.equals(EXPECTED_CURRENT_ADMIN)) {
    console.error(`ERROR: Oracle admin is ${currentOracleAdmin.toBase58()}, expected ${EXPECTED_CURRENT_ADMIN.toBase58()}`);
    process.exit(1);
  } else {
    console.log(`Oracle:  ${currentOracleAdmin.toBase58()} — ready`);
  }

  // Vaults
  interface VaultTarget {
    name: string;
    vaultPda: PublicKey;
    channelConfig: PublicKey;
    skip: boolean;
  }
  const vaultTargets: VaultTarget[] = [];

  for (const ch of CHANNELS) {
    const channelConfig = new PublicKey(ch.channelConfig);
    const [vaultPda] = PublicKey.findProgramAddressSync(
      [VAULT_SEED, channelConfig.toBuffer()],
      VAULT_PROGRAM_ID,
    );

    const acct = await conn.getAccountInfo(vaultPda);
    if (!acct) {
      console.error(`ERROR: Vault ${ch.name} (${vaultPda.toBase58()}) not found!`);
      process.exit(1);
    }

    const admin = parseVaultAdmin(Buffer.from(acct.data));
    if (admin.equals(SQUADS_VAULT)) {
      console.log(`${ch.name.padEnd(20)} Already transferred — SKIP`);
      vaultTargets.push({ name: ch.name, vaultPda, channelConfig, skip: true });
    } else if (!admin.equals(EXPECTED_CURRENT_ADMIN)) {
      console.error(`ERROR: ${ch.name} admin is ${admin.toBase58()}, expected ${EXPECTED_CURRENT_ADMIN.toBase58()}`);
      process.exit(1);
    } else {
      console.log(`${ch.name.padEnd(20)} ${admin.toBase58()} — ready`);
      vaultTargets.push({ name: ch.name, vaultPda, channelConfig, skip: false });
    }
  }

  const oracleSkip = currentOracleAdmin.equals(SQUADS_VAULT);
  const pendingVaults = vaultTargets.filter((v) => !v.skip);
  const totalTxs = (oracleSkip ? 0 : 1) + pendingVaults.length;

  console.log(`\n--- Transfer Plan ---`);
  console.log(`Oracle:      ${oracleSkip ? "SKIP (already done)" : "TRANSFER"}`);
  console.log(`Vaults:      ${pendingVaults.length} to transfer, ${vaultTargets.length - pendingVaults.length} already done`);
  console.log(`Total txs:   ${totalTxs}`);
  console.log(`New admin:   ${SQUADS_VAULT.toBase58()}`);

  if (dryRun) {
    console.log("\n=== Dry run complete. Run with --execute to submit transactions. ===");
    return;
  }

  if (totalTxs === 0) {
    console.log("\nAll admins already transferred. Nothing to do.");
    return;
  }

  // --- Execute transfers ---
  console.log(`\n--- Executing ${totalTxs} transfers ---\n`);

  const results: { name: string; sig: string; success: boolean }[] = [];

  // 1. Oracle update_admin
  if (!oracleSkip) {
    try {
      console.log(`[1/${totalTxs}] Oracle ProtocolState...`);
      const sig = await oracleProgram.methods
        .updateAdmin(SQUADS_VAULT)
        .accounts({
          admin: payer.publicKey,
          protocolState: protocolPda,
        })
        .rpc();

      console.log(`  TX: ${sig}`);

      // Verify
      const after = await conn.getAccountInfo(protocolPda);
      const newAdmin = parseProtocolAdmin(Buffer.from(after!.data));
      const ok = newAdmin.equals(SQUADS_VAULT);
      console.log(`  Verify: ${ok ? "CONFIRMED" : "FAILED — admin is " + newAdmin.toBase58()}`);
      results.push({ name: "Oracle", sig, success: ok });
    } catch (err: any) {
      console.error(`  ERROR: ${err.message}`);
      results.push({ name: "Oracle", sig: "FAILED", success: false });
    }
  }

  // 2. Vault update_admin (one at a time for safety)
  let txNum = oracleSkip ? 1 : 2;
  for (const vt of pendingVaults) {
    try {
      console.log(`[${txNum}/${totalTxs}] ${vt.name}...`);
      const sig = await vaultProgram.methods
        .updateAdmin(SQUADS_VAULT)
        .accounts({
          admin: payer.publicKey,
          vault: vt.vaultPda,
        })
        .rpc();

      console.log(`  TX: ${sig}`);

      // Verify
      const after = await conn.getAccountInfo(vt.vaultPda);
      const newAdmin = parseVaultAdmin(Buffer.from(after!.data));
      const ok = newAdmin.equals(SQUADS_VAULT);
      console.log(`  Verify: ${ok ? "CONFIRMED" : "FAILED — admin is " + newAdmin.toBase58()}`);
      results.push({ name: vt.name, sig, success: ok });
    } catch (err: any) {
      console.error(`  ERROR: ${err.message}`);
      results.push({ name: vt.name, sig: "FAILED", success: false });
    }
    txNum++;
  }

  // --- Summary ---
  console.log("\n=== Transfer Summary ===");
  console.log(`${"Name".padEnd(20)} ${"Signature".padEnd(90)} Status`);
  console.log("-".repeat(120));
  for (const r of results) {
    console.log(`${r.name.padEnd(20)} ${r.sig.padEnd(90)} ${r.success ? "OK" : "FAILED"}`);
  }

  const allOk = results.every((r) => r.success);
  console.log(`\nResult: ${allOk ? "ALL TRANSFERS SUCCESSFUL" : "SOME TRANSFERS FAILED"}`);

  if (!allOk) {
    console.log("\nRe-run with --execute to retry failed transfers (already-transferred accounts will be skipped).");
    process.exit(1);
  }

  console.log("\nAdmin authority has been transferred to the Squads multisig vault.");
  console.log("Run verify-admin-state.ts with --target to confirm:");
  console.log(`  RPC_URL=... npx tsx scripts/admin/verify-admin-state.ts --target ${SQUADS_VAULT.toBase58()}`);
}

main().catch((err) => {
  console.error("Fatal:", err);
  process.exit(1);
});
