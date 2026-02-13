/**
 * check-cutover-epochs.ts
 *
 * Reads the current Solana epoch and each channel's cutover_epoch from on-chain
 * ChannelConfigV2 accounts. Reports which channels have V2 sunset active,
 * which are unprotected (cutover_epoch = 0), and how many epochs remain.
 *
 * Usage:
 *   RPC_URL=https://... npx ts-node scripts/admin/check-cutover-epochs.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, PublicKey } from "@solana/web3.js";
import { CHANNELS } from "../keepers/lib/channels.js";

const ORACLE_PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");

async function main() {
  const rpcUrl = process.env.RPC_URL;
  if (!rpcUrl) throw new Error("Set RPC_URL");

  const connection = new Connection(rpcUrl, "confirmed");
  const wallet = new anchor.Wallet(anchor.web3.Keypair.generate());
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed" });
  const oracleIdl = await Program.fetchIdl(ORACLE_PROGRAM_ID, provider);
  if (!oracleIdl) throw new Error("Failed to fetch Oracle IDL");
  const oracleProgram = new Program(oracleIdl, provider);

  const epochInfo = await connection.getEpochInfo();
  const currentEpoch = epochInfo.epoch;

  console.log("=".repeat(70));
  console.log("CUTOVER EPOCH STATUS CHECK");
  console.log("=".repeat(70));
  console.log(`\nCurrent Solana epoch: ${currentEpoch}`);
  console.log(`Channels: ${CHANNELS.length}\n`);

  let unprotected = 0;
  let active = 0;
  let expired = 0;

  for (const channel of CHANNELS) {
    const channelConfig = new PublicKey(channel.channelConfig);

    try {
      const cfg: any = await oracleProgram.account.channelConfigV2.fetch(channelConfig);
      const cutoverEpoch = Number(cfg.cutoverEpoch);

      let status: string;
      if (cutoverEpoch === 0) {
        status = "  UNPROTECTED (cutover_epoch = 0, V2 claims allowed indefinitely)";
        unprotected++;
      } else if (currentEpoch >= cutoverEpoch) {
        status = `  V2 DISABLED (cutover_epoch = ${cutoverEpoch}, expired ${currentEpoch - cutoverEpoch} epochs ago)`;
        expired++;
      } else {
        const remaining = cutoverEpoch - currentEpoch;
        status = `  ACTIVE (cutover_epoch = ${cutoverEpoch}, ${remaining} epochs remaining)`;
        active++;
      }

      console.log(`${channel.name}`);
      console.log(`  config: ${channel.channelConfig}`);
      console.log(status);
      console.log();
    } catch (err: any) {
      console.log(`${channel.name}`);
      console.log(`  config: ${channel.channelConfig}`);
      console.log(`  ERROR: ${err.message || "fetch failed"}`);
      console.log();
    }
  }

  console.log("=".repeat(70));
  console.log("SUMMARY");
  console.log("=".repeat(70));
  console.log(`  Unprotected (needs cutover_epoch set): ${unprotected}`);
  console.log(`  Active (V2 sunset scheduled):          ${active}`);
  console.log(`  Expired (V2 disabled):                 ${expired}`);

  if (unprotected > 0) {
    console.log(`\n  ACTION REQUIRED: ${unprotected} channel(s) have cutover_epoch = 0.`);
    console.log("  Run set-cutover-epochs.ts to schedule V2 sunset.");
  } else {
    console.log("\n  All channels have cutover_epoch set.");
  }
}

main().catch(console.error);
