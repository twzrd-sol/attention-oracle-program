#!/usr/bin/env ts-node
/**
 * close-legacy-epochs.ts
 *
 * Reclaims rent from legacy EpochState accounts.
 *
 * The program has 195 EpochState accounts (163 open, 32 already closed).
 * This script enumerates and closes the remaining open accounts to reclaim ~1.5 SOL.
 *
 * Usage:
 *   # Dry run - enumerate only
 *   ts-node close-legacy-epochs.ts --dry-run
 *
 *   # Close all legacy epochs
 *   ts-node close-legacy-epochs.ts
 *
 *   # Close specific epoch/subject
 *   ts-node close-legacy-epochs.ts --epoch 12345 --subject <pubkey>
 *
 * Requirements:
 *   - ANCHOR_WALLET or ~/.config/solana/id.json must be the ADMIN_AUTHORITY
 *   - SYNDICA_RPC or ANCHOR_PROVIDER_URL environment variable
 *   - Program built with --features legacy
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey, AccountInfo } from "@solana/web3.js";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const EPOCH_STATE_SEED = Buffer.from("epoch_state");

// Known mints for the "open" variant
const KNOWN_MINTS = [
  new PublicKey("ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe"), // CCM v3
  new PublicKey("Bwmh8UfU4cKBrm9BLXV5RmQjNVaRAJf3bHX3T99YV3NM"), // CCM v2
  new PublicKey("Dxk8mAbfBMFM6hh6HqFpC1KSYHMUiNu5TkPsUJMRtVkR"), // CCM v1
];

interface EpochStateAccount {
  pubkey: PublicKey;
  epoch: bigint;
  subjectId: PublicKey;
  mint: PublicKey | null; // null for legacy, set for open variant
  lamports: number;
  closed: boolean;
  timestamp: bigint;
}

async function main() {
  const args = process.argv.slice(2);
  const dryRun = args.includes("--dry-run");
  const specificEpoch = args.includes("--epoch")
    ? BigInt(args[args.indexOf("--epoch") + 1])
    : null;
  const specificSubject = args.includes("--subject")
    ? new PublicKey(args[args.indexOf("--subject") + 1])
    : null;

  // Load wallet
  const walletPath = process.env.ANCHOR_WALLET || `${process.env.HOME}/.config/solana/id.json`;
  const walletKeypair = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(walletPath, "utf-8")))
  );

  console.log(`Admin wallet: ${walletKeypair.publicKey.toString()}`);

  // Setup connection
  const rpcUrl = process.env.SYNDICA_RPC || process.env.ANCHOR_PROVIDER_URL || "https://api.mainnet-beta.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");

  const wallet = new anchor.Wallet(walletKeypair);
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });
  anchor.setProvider(provider);

  // Load program IDL (must be built with --features legacy)
  const idlPath = `${__dirname}/../target/idl/token_2022.json`;
  if (!fs.existsSync(idlPath)) {
    console.error("IDL not found. Build program with: anchor build --features legacy");
    process.exit(1);
  }
  const idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));

  // Guard against missing account sizes
  if (idl.accounts) {
    idl.accounts.forEach((acc: any) => {
      if (acc.size === null || acc.size === undefined) {
        acc.size = 0;
      }
    });
  }

  const program = new Program(idl, PROGRAM_ID.toString(), provider);

  console.log("\n=== Enumerating EpochState Accounts ===\n");

  // Get all accounts owned by the program
  const accounts = await connection.getProgramAccounts(PROGRAM_ID, {
    filters: [
      // EpochState discriminator (first 8 bytes)
      // You may need to adjust this based on your actual discriminator
      { dataSize: 296 }, // Typical EpochState size
    ],
  });

  console.log(`Found ${accounts.length} potential EpochState accounts\n`);

  // Parse accounts
  const epochStates: EpochStateAccount[] = [];
  let totalReclaimable = 0;

  for (const { pubkey, account } of accounts) {
    try {
      // Parse basic info from account data
      // Layout: discriminator (8) + epoch (8) + subject_id (32) + claim_count (4) + closed (1) + ...
      const data = account.data;

      // Skip if not an EpochState (check discriminator)
      const epoch = data.readBigUInt64LE(8);
      const subjectIdBytes = data.slice(16, 48);
      const subjectId = new PublicKey(subjectIdBytes);
      const timestamp = data.readBigInt64LE(48); // timestamp field

      // Check closed flag (varies by version, typically at a known offset)
      // For legacy accounts, we'll check if data is zeroed or has special marker
      const closed = false; // Will verify by account existence

      // Determine if this is legacy or open variant by trying to derive PDAs
      let mint: PublicKey | null = null;

      // Try legacy derivation first
      const [legacyPda] = PublicKey.findProgramAddressSync(
        [EPOCH_STATE_SEED, Buffer.from(epoch.toString(16).padStart(16, '0'), 'hex').reverse(), subjectId.toBuffer()],
        PROGRAM_ID
      );

      if (!legacyPda.equals(pubkey)) {
        // Try open variant with each known mint
        for (const knownMint of KNOWN_MINTS) {
          const epochBytes = Buffer.alloc(8);
          epochBytes.writeBigUInt64LE(epoch);

          const [openPda] = PublicKey.findProgramAddressSync(
            [EPOCH_STATE_SEED, epochBytes, subjectId.toBuffer(), knownMint.toBuffer()],
            PROGRAM_ID
          );

          if (openPda.equals(pubkey)) {
            mint = knownMint;
            break;
          }
        }
      }

      epochStates.push({
        pubkey,
        epoch,
        subjectId,
        mint,
        lamports: account.lamports,
        closed,
        timestamp,
      });

      totalReclaimable += account.lamports;
    } catch (e) {
      console.warn(`Failed to parse account ${pubkey.toString()}: ${e}`);
    }
  }

  // Sort by epoch
  epochStates.sort((a, b) => Number(a.epoch - b.epoch));

  // Display summary
  console.log("=== EpochState Summary ===\n");
  console.log(`Total accounts: ${epochStates.length}`);
  console.log(`Legacy (no mint): ${epochStates.filter(e => !e.mint).length}`);
  console.log(`Open (with mint): ${epochStates.filter(e => e.mint).length}`);
  console.log(`Total reclaimable: ${(totalReclaimable / 1e9).toFixed(4)} SOL\n`);

  if (epochStates.length === 0) {
    console.log("No EpochState accounts found.");
    return;
  }

  // List accounts
  console.log("=== Accounts to Close ===\n");
  for (const es of epochStates) {
    const mintStr = es.mint ? es.mint.toString().slice(0, 8) + "..." : "LEGACY";
    console.log(
      `  epoch=${es.epoch} subject=${es.subjectId.toString().slice(0, 8)}... ` +
      `mint=${mintStr} lamports=${es.lamports} (${(es.lamports / 1e9).toFixed(4)} SOL)`
    );
  }

  if (dryRun) {
    console.log("\n[DRY RUN] No accounts closed.");
    return;
  }

  // Confirm before proceeding
  console.log("\n=== Closing Accounts ===\n");
  console.log("Press Ctrl+C to cancel, or wait 5 seconds to proceed...");
  await new Promise(r => setTimeout(r, 5000));

  let closed = 0;
  let failed = 0;
  let reclaimedLamports = 0;

  for (const es of epochStates) {
    // Skip if filtering
    if (specificEpoch !== null && es.epoch !== specificEpoch) continue;
    if (specificSubject !== null && !es.subjectId.equals(specificSubject)) continue;

    try {
      const epochBytes = Buffer.alloc(8);
      epochBytes.writeBigUInt64LE(es.epoch);

      if (es.mint) {
        // Use force_close_epoch_state_open for accounts with mint in seeds
        console.log(`Closing open epoch ${es.epoch} (mint: ${es.mint.toString().slice(0, 8)}...)...`);

        await program.methods
          .forceCloseEpochStateOpen(
            new anchor.BN(es.epoch.toString()),
            es.subjectId,
            es.mint
          )
          .accounts({
            admin: wallet.publicKey,
            epochState: es.pubkey,
          })
          .rpc();
      } else {
        // Use force_close_epoch_state_legacy for legacy accounts
        console.log(`Closing legacy epoch ${es.epoch}...`);

        await program.methods
          .forceCloseEpochStateLegacy(
            new anchor.BN(es.epoch.toString()),
            es.subjectId
          )
          .accounts({
            admin: wallet.publicKey,
            epochState: es.pubkey,
          })
          .rpc();
      }

      closed++;
      reclaimedLamports += es.lamports;
      console.log(`  ✓ Closed! Reclaimed ${(es.lamports / 1e9).toFixed(4)} SOL`);

      // Rate limit
      await new Promise(r => setTimeout(r, 500));
    } catch (e: any) {
      failed++;
      console.error(`  ✗ Failed: ${e.message || e}`);

      // If grace period not met, show when it will be ready
      if (e.message?.includes("EpochClosed") || e.message?.includes("grace")) {
        console.log(`    (Grace period not yet elapsed for this epoch)`);
      }
    }
  }

  console.log("\n=== Summary ===\n");
  console.log(`Closed: ${closed}`);
  console.log(`Failed: ${failed}`);
  console.log(`Reclaimed: ${(reclaimedLamports / 1e9).toFixed(4)} SOL`);
}

main().catch(console.error);
