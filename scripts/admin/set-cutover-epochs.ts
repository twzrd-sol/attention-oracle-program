/**
 * set-cutover-epochs.ts
 *
 * Sets cutover_epoch on all active channels via update_channel_cutover_epoch IX.
 * Supports two modes:
 *   1. Direct: admin keypair signs and sends (single-signer authority)
 *   2. Dry-run: prints the IX data for each channel (for Squads proposal building)
 *
 * Usage:
 *   # Dry run (prints instructions only):
 *   RPC_URL=https://... CUTOVER_EPOCH=750 npx ts-node scripts/admin/set-cutover-epochs.ts
 *
 *   # Execute with keypair:
 *   RPC_URL=https://... CUTOVER_EPOCH=750 ADMIN_KEY=~/.config/solana/id.json EXECUTE=1 \
 *     npx ts-node scripts/admin/set-cutover-epochs.ts
 *
 * Environment:
 *   RPC_URL         - Solana RPC endpoint (required)
 *   CUTOVER_EPOCH   - Target cutover epoch (required). Set to 0 to disable V2 sunset.
 *   ADMIN_KEY       - Path to admin keypair JSON (required if EXECUTE=1)
 *   EXECUTE         - Set to "1" to actually send transactions (default: dry run)
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, PublicKey, Keypair, Transaction, sendAndConfirmTransaction } from "@solana/web3.js";
import { CHANNELS, oracleChannelName } from "../keepers/lib/channels.js";
import * as fs from "fs";
import * as path from "path";

const ORACLE_PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");
const PROTOCOL_SEED = Buffer.from("protocol");

async function main() {
  const rpcUrl = process.env.RPC_URL;
  if (!rpcUrl) throw new Error("Set RPC_URL");

  const cutoverEpochStr = process.env.CUTOVER_EPOCH;
  if (!cutoverEpochStr) throw new Error("Set CUTOVER_EPOCH (e.g. 750)");
  const cutoverEpoch = parseInt(cutoverEpochStr, 10);
  if (isNaN(cutoverEpoch) || cutoverEpoch < 0) throw new Error("CUTOVER_EPOCH must be a non-negative integer");

  const execute = process.env.EXECUTE === "1";

  const connection = new Connection(rpcUrl, "confirmed");
  const epochInfo = await connection.getEpochInfo();
  const currentEpoch = epochInfo.epoch;

  // Load admin keypair if executing
  let adminKeypair: Keypair | null = null;
  if (execute) {
    const keyPath = process.env.ADMIN_KEY;
    if (!keyPath) throw new Error("ADMIN_KEY required when EXECUTE=1");
    const resolved = keyPath.startsWith("~")
      ? path.join(process.env.HOME!, keyPath.slice(1))
      : keyPath;
    const secretKey = JSON.parse(fs.readFileSync(resolved, "utf-8"));
    adminKeypair = Keypair.fromSecretKey(Uint8Array.from(secretKey));
  }

  const wallet = adminKeypair
    ? new anchor.Wallet(adminKeypair)
    : new anchor.Wallet(Keypair.generate());
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed" });
  const oracleIdl = await Program.fetchIdl(ORACLE_PROGRAM_ID, provider);
  if (!oracleIdl) throw new Error("Failed to fetch Oracle IDL");
  const oracleProgram = new Program(oracleIdl, provider);

  // Derive protocol state PDA
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, CCM_MINT.toBuffer()],
    ORACLE_PROGRAM_ID
  );

  console.log("=".repeat(70));
  console.log(execute ? "SET CUTOVER EPOCHS (EXECUTE)" : "SET CUTOVER EPOCHS (DRY RUN)");
  console.log("=".repeat(70));
  console.log(`Current epoch:  ${currentEpoch}`);
  console.log(`Target cutover: ${cutoverEpoch}`);
  if (cutoverEpoch > 0 && cutoverEpoch <= currentEpoch) {
    console.log(`  WARNING: target epoch ${cutoverEpoch} <= current epoch ${currentEpoch}`);
    console.log("  V2 claims will be IMMEDIATELY disabled on all channels.");
  }
  if (adminKeypair) {
    console.log(`Admin:          ${adminKeypair.publicKey.toBase58()}`);
  }
  console.log(`Protocol state: ${protocolState.toBase58()}`);
  console.log(`Channels:       ${CHANNELS.length}`);
  console.log();

  let success = 0;
  let skipped = 0;
  let failed = 0;

  for (const channel of CHANNELS) {
    const channelConfig = new PublicKey(channel.channelConfig);

    try {
      // Read current state
      const cfg: any = await oracleProgram.account.channelConfigV2.fetch(channelConfig);
      const current = Number(cfg.cutoverEpoch);

      if (current === cutoverEpoch) {
        console.log(`SKIP ${channel.name}: already set to ${cutoverEpoch}`);
        skipped++;
        continue;
      }

      // The Oracle program derives ChannelConfigV2 PDAs from the original channel string,
      // not the dash-safe registry name.
      const channelName = oracleChannelName(channel);

      if (execute && adminKeypair) {
        const ix = await oracleProgram.methods
          .updateChannelCutoverEpoch(channelName, new anchor.BN(cutoverEpoch))
          .accounts({
            admin: adminKeypair.publicKey,
            protocolState,
            channelConfig,
          })
          .instruction();

        const tx = new Transaction().add(ix);
        const sig = await sendAndConfirmTransaction(connection, tx, [adminKeypair]);
        console.log(`OK   ${channel.name}: ${current} -> ${cutoverEpoch}  tx: ${sig}`);
        success++;
      } else {
        console.log(`PLAN ${channel.name}: ${current} -> ${cutoverEpoch}`);
        console.log(`     config: ${channel.channelConfig}`);
        success++;
      }
    } catch (err: any) {
      console.log(`FAIL ${channel.name}: ${err.message || err}`);
      failed++;
    }
  }

  console.log("\n" + "=".repeat(70));
  console.log("SUMMARY");
  console.log("=".repeat(70));
  console.log(`  ${execute ? "Updated" : "Planned"}: ${success}`);
  console.log(`  Skipped (already set):  ${skipped}`);
  console.log(`  Failed:                 ${failed}`);

  if (!execute && success > 0) {
    console.log("\n  To execute, re-run with EXECUTE=1 and ADMIN_KEY set.");
    console.log("  For Squads multisig, use the PLAN output above to build proposals.");
  }
}

main().catch(console.error);
