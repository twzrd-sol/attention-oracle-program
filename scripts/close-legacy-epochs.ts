#!/usr/bin/env ts-node
/**
 * close-legacy-epochs.ts
 *
 * Reclaims rent from legacy EpochState accounts.
 *
 * This script enumerates and closes legacy EpochState accounts to reclaim rent.
 *
 * Usage:
 *   # Dry run - enumerate only
 *   ts-node close-legacy-epochs.ts --dry-run
 *
 *   # Close all legacy epochs
 *   ts-node close-legacy-epochs.ts
 *
 *   # Close legacy + open-variant epochs (mint in seeds)
 *   ts-node close-legacy-epochs.ts --include-open
 *
 *   # Close specific epoch/subject
 *   ts-node close-legacy-epochs.ts --epoch 12345 --subject <pubkey>
 *
 * Requirements:
 *   - ANCHOR_WALLET or ~/.config/solana/id.json must be the ADMIN_AUTHORITY
 *   - SYNDICA_RPC or ANCHOR_PROVIDER_URL environment variable
 *   - Program built with --features legacy
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
import { fileURLToPath } from "url";
import crypto from "crypto";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const ADMIN_AUTHORITY = new PublicKey("2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD");
const EPOCH_STATE_SEED = Buffer.from("epoch_state");
// EpochState discriminator (sha256("account:EpochState")[0..8])
const EPOCH_STATE_DISCRIMINATOR = Buffer.from([191, 63, 139, 237, 144, 12, 223, 210]);

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
  const includeOpen = args.includes("--include-open");
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
  if (!walletKeypair.publicKey.equals(ADMIN_AUTHORITY)) {
    throw new Error(`Admin wallet mismatch. Expected ${ADMIN_AUTHORITY.toString()}`);
  }

  // Setup connection
  const rpcUrl = process.env.SYNDICA_RPC || process.env.ANCHOR_PROVIDER_URL || "https://api.mainnet-beta.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");

  const wallet = walletKeypair;

  // If needed, program IDL can be loaded here for future extensions.

  console.log("\n=== Enumerating EpochState Accounts ===\n");

  // Get all accounts owned by the program (filter in-process for safety)
  const accounts = await connection.getProgramAccounts(PROGRAM_ID);

  console.log(`Found ${accounts.length} total program accounts\n`);

  // Parse accounts
  const epochStates: EpochStateAccount[] = [];
  let totalReclaimable = 0;
  let skippedNonEpoch = 0;
  let skippedUnknownVariant = 0;

  for (const { pubkey, account } of accounts) {
    try {
      // Parse basic info from account data
      // Layout: discriminator (8) + epoch (8) + subject_id (32) + claim_count (4) + closed (1) + ...
      const data = account.data;
      if (data.length < 156) {
        skippedNonEpoch += 1;
        continue;
      }

      // Skip if not an EpochState (check discriminator)
      if (!data.subarray(0, 8).equals(EPOCH_STATE_DISCRIMINATOR)) {
        skippedNonEpoch += 1;
        continue;
      }

      const epoch = data.readBigUInt64LE(8);
      // Layout offsets (after 8-byte discriminator):
      // epoch(8) @8, root(32) @16, claim_count(4) @48, mint(32) @52,
      // subject(32) @84, treasury(32) @116, timestamp(8) @148
      const mintBytes = data.slice(52, 84);
      const subjectIdBytes = data.slice(84, 116);
      const subjectId = new PublicKey(subjectIdBytes);
      const timestamp = data.readBigInt64LE(148); // timestamp field

      // Check closed flag (varies by version, typically at a known offset)
      // For legacy accounts, we'll check if data is zeroed or has special marker
      const closed = false; // Will verify by account existence

      // Determine if this is legacy or open variant by trying to derive PDAs
      let mint: PublicKey | null = null;

      // Try legacy derivation first
      const epochBytes = Buffer.alloc(8);
      epochBytes.writeBigUInt64LE(epoch);
      const [legacyPda] = PublicKey.findProgramAddressSync(
        [EPOCH_STATE_SEED, epochBytes, subjectId.toBuffer()],
        PROGRAM_ID
      );

      if (!legacyPda.equals(pubkey)) {
        const accountMint = new PublicKey(mintBytes);
        // Only accept open-variant if mint is in the allowlist
        if (KNOWN_MINTS.some(m => m.equals(accountMint))) {
          const [openPda] = PublicKey.findProgramAddressSync(
            [EPOCH_STATE_SEED, epochBytes, subjectId.toBuffer(), accountMint.toBuffer()],
            PROGRAM_ID
          );
          if (openPda.equals(pubkey)) {
            mint = accountMint;
          }
        }
      }

      if (!legacyPda.equals(pubkey) && !mint) {
        skippedUnknownVariant += 1;
        continue;
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

  const legacyStates = epochStates.filter(e => !e.mint);
  const openStates = epochStates.filter(e => e.mint);
  const reclaimLegacy = legacyStates.reduce((sum, e) => sum + e.lamports, 0);
  const reclaimOpen = openStates.reduce((sum, e) => sum + e.lamports, 0);
  const closableStates = includeOpen ? epochStates : legacyStates;
  const reclaimClosable = includeOpen ? totalReclaimable : reclaimLegacy;

  // Display summary
  console.log("=== EpochState Summary ===\n");
  console.log(`Total accounts: ${epochStates.length}`);
  console.log(`Legacy (no mint): ${legacyStates.length}`);
  console.log(`Open (with mint): ${openStates.length}`);
  console.log(`Skipped non-epoch accounts: ${skippedNonEpoch}`);
  console.log(`Skipped unknown epoch variants: ${skippedUnknownVariant}`);
  console.log(`Reclaimable (legacy only): ${(reclaimLegacy / 1e9).toFixed(4)} SOL`);
  console.log(`Reclaimable (incl. open): ${(totalReclaimable / 1e9).toFixed(4)} SOL`);
  console.log(`Reclaimable (this run): ${(reclaimClosable / 1e9).toFixed(4)} SOL\n`);

  if (closableStates.length === 0) {
    console.log("No EpochState accounts found.");
    return;
  }

  // List accounts
  console.log("=== Accounts to Close ===\n");
  for (const es of closableStates) {
    const mintStr = es.mint ? es.mint.toString().slice(0, 8) + "..." : "LEGACY";
    console.log(
      `  epoch=${es.epoch} subject=${es.subjectId.toString().slice(0, 8)}... ` +
      `mint=${mintStr} lamports=${es.lamports} (${(es.lamports / 1e9).toFixed(4)} SOL)`
    );
  }

  if (dryRun) {
    console.log("\n[DRY RUN] No accounts closed.");
    if (!includeOpen) {
      console.log("Note: open-variant epoch accounts were skipped. Use --include-open to include them.");
    }
    return;
  }

  // Confirm before proceeding
  console.log("\n=== Closing Accounts ===\n");
  console.log("Press Ctrl+C to cancel, or wait 5 seconds to proceed...");
  await new Promise(r => setTimeout(r, 5000));

  let closed = 0;
  let failed = 0;
  let reclaimedLamports = 0;

  const adminKey = wallet.publicKey;
  const ixKeys = (epochState: PublicKey) => ([
    { pubkey: adminKey, isSigner: true, isWritable: true },
    { pubkey: epochState, isSigner: false, isWritable: true },
  ]);

  const u64le = (v: bigint) => {
    const buf = Buffer.alloc(8);
    buf.writeBigUInt64LE(v);
    return buf;
  };

  const ixDiscriminator = (name: string) => {
    return crypto.createHash("sha256").update(`global:${name}`).digest().subarray(0, 8);
  };

  for (const es of closableStates) {
    // Skip if filtering
    if (specificEpoch !== null && es.epoch !== specificEpoch) continue;
    if (specificSubject !== null && !es.subjectId.equals(specificSubject)) continue;

    try {
      if (es.mint) {
        if (!includeOpen) {
          continue;
        }
        // Use force_close_epoch_state_open for accounts with mint in seeds
        console.log(`Closing open epoch ${es.epoch} (mint: ${es.mint.toString().slice(0, 8)}...)...`);
        const data = Buffer.concat([
          ixDiscriminator("force_close_epoch_state_open"),
          u64le(es.epoch),
          es.subjectId.toBuffer(),
          es.mint.toBuffer(),
        ]);
        const ix = new TransactionInstruction({
          programId: PROGRAM_ID,
          keys: ixKeys(es.pubkey),
          data,
        });
        const tx = new Transaction().add(ix);
        const sig = await sendAndConfirmTransaction(connection, tx, [walletKeypair]);
        console.log(`  sig: ${sig}`);
      } else {
        // Use force_close_epoch_state_legacy for legacy accounts
        console.log(`Closing legacy epoch ${es.epoch}...`);
        const data = Buffer.concat([
          ixDiscriminator("force_close_epoch_state_legacy"),
          u64le(es.epoch),
          es.subjectId.toBuffer(),
        ]);
        const ix = new TransactionInstruction({
          programId: PROGRAM_ID,
          keys: ixKeys(es.pubkey),
          data,
        });
        const tx = new Transaction().add(ix);
        const sig = await sendAndConfirmTransaction(connection, tx, [walletKeypair]);
        console.log(`  sig: ${sig}`);
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
