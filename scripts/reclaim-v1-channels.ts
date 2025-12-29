#!/usr/bin/env ts-node
/**
 * reclaim-v1-channels.ts
 *
 * Closes legacy ChannelState accounts to reclaim rent (approx 14 SOL total).
 * Uses the `force_close_channel_state_legacy` instruction.
 *
 * Usage:
 *   ts-node scripts/reclaim-v1-channels.ts --dry-run
 *   ts-node scripts/reclaim-v1-channels.ts
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
import { requireScriptEnv } from "./script-guard.ts";
import pkg from "js-sha3";
const { keccak256 } = pkg;
import { createHash } from "crypto";
import bs58 from "bs58";

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const ADMIN_AUTHORITY = new PublicKey("2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD");

function getDiscriminator(name: string): Buffer {
  return createHash("sha256").update(`account:${name}`).digest().slice(0, 8);
}
const CHANNEL_STATE_DISC = getDiscriminator("ChannelState");

function getIxDiscriminator(name: string): Buffer {
  return createHash("sha256").update(`global:${name}`).digest().slice(0, 8);
}
const FORCE_CLOSE_IX_DISC = getIxDiscriminator("force_close_channel_state_legacy");

async function main() {
  const args = process.argv.slice(2);
  const dryRun = args.includes("--dry-run");

  const { rpcUrl, keypairPath } = requireScriptEnv();
  const connection = new Connection(rpcUrl, "confirmed");

  // Load wallet (Admin)
  const walletKeypair = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(keypairPath, "utf-8")))
  );
  console.log(`Admin/Payer: ${walletKeypair.publicKey.toString()}`);

  if (!walletKeypair.publicKey.equals(ADMIN_AUTHORITY)) {
    console.error(`❌ Wallet mismatch! Must be admin: ${ADMIN_AUTHORITY.toString()}`);
    process.exit(1);
  }


  console.log("Scanning for legacy channels...");
  // Use filter to reduce RPC load (prevents 429 errors)
  const accounts = await connection.getProgramAccounts(PROGRAM_ID, {
    filters: [
      {
        memcmp: {
          offset: 0,
          bytes: bs58.encode(CHANNEL_STATE_DISC),
        },
      },
    ],
  });

  const legacyAccounts: { pubkey: PublicKey; lamports: number; subject: PublicKey, mint: PublicKey }[] = [];

  for (const { pubkey, account } of accounts) {
    const data = account.data;
    if (data.length < 88) continue; // Header size

    // Check discriminator
    if (data.slice(0, 8).equals(CHANNEL_STATE_DISC)) {
      // It's a ChannelState. 
      const mint = new PublicKey(data.slice(10, 42));
      const subject = new PublicKey(data.slice(42, 74));

      legacyAccounts.push({
        pubkey,
        lamports: account.lamports,
        subject,
        mint
      });
    }
  }

  console.log(`Found ${legacyAccounts.length} legacy ChannelState accounts.`);
  const totalRent = legacyAccounts.reduce((sum, a) => sum + a.lamports, 0) / 1e9;
  console.log(`Total reclaimable rent: ${totalRent.toFixed(4)} SOL`);

  if (dryRun) {
    console.log("[Dry Run] Exiting.");
    return;
  }

  console.log("Closing accounts...");
  let closed = 0;
  let reclaimed = 0;

  for (const acc of legacyAccounts) {
    console.log(`Closing ${acc.pubkey.toString()} (Subject: ${acc.subject.toString().slice(0, 8)}...)...`);

    // Build instruction data: Disc + Mint + Subject
    const ixData = Buffer.concat([
      FORCE_CLOSE_IX_DISC,
      acc.mint.toBuffer(),
      acc.subject.toBuffer()
    ]);

    const ix = new TransactionInstruction({
      programId: PROGRAM_ID,
      keys: [
        { pubkey: walletKeypair.publicKey, isSigner: true, isWritable: true }, // admin
        { pubkey: acc.pubkey, isSigner: false, isWritable: true }  // channel_state
      ],
      data: ixData
    });

    try {
      const tx = new Transaction().add(ix);
      const sig = await sendAndConfirmTransaction(connection, tx, [walletKeypair], {
        commitment: "confirmed",
        skipPreflight: true
      });
      console.log(`  ✓ Closed! Sig: ${sig}`);
      closed++;
      reclaimed += acc.lamports;

      // Rate limit
      await new Promise(r => setTimeout(r, 200));
    } catch (e: any) {
      console.error(`  ✗ Failed: ${e.message}`);
      if (e.getLogs) {
        console.error("  Logs:", await e.getLogs());
      } else if (e.logs) {
        console.error("  Logs:", e.logs);
      }
    }
  }

  console.log(`\nDone! Closed ${closed} accounts. Reclaimed ${(reclaimed / 1e9).toFixed(4)} SOL.`);
}

main().catch(console.error);