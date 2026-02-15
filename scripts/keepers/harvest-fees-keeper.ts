/**
 * Harvest Fees Keeper — Permissionless crank for Token-2022 transfer fee collection.
 *
 * Periodically discovers CCM token accounts with withheld transfer fees,
 * then calls harvest_fees() to sweep them to the protocol treasury.
 *
 * harvest_fees is permissionless (since proposal #24) — any funded wallet
 * can call it. Treasury destination is enforced by on-chain constraints.
 *
 * Discovery strategy (avoids getProgramAccounts on Token-2022):
 *   1. getTokenLargestAccounts(CCM_MINT) — top 20 holders
 *   2. Derive all 16 vault CCM buffer PDAs — deterministic
 *   3. Dedupe, check each for withheld fees, batch harvest
 *
 * Note: getProgramAccounts is rejected by most RPC providers for Token-2022
 * due to the program's massive account set. The discovery strategy above
 * avoids GPA entirely by combining largest-accounts RPC + deterministic
 * PDA derivation for all known vault buffers.
 *
 * Environment:
 *   CLUSTER=mainnet-beta   (or devnet)
 *   I_UNDERSTAND_MAINNET=1 (for mainnet)
 *   RPC_URL=https://...
 *   KEYPAIR=/secure/path/keeper.json
 *   HARVEST_INTERVAL_MS=3600000   (optional, default 1 hour)
 *
 * Usage:
 *   CLUSTER=mainnet-beta I_UNDERSTAND_MAINNET=1 RPC_URL=... KEYPAIR=... \
 *     npx tsx scripts/keepers/harvest-fees-keeper.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  PublicKey,
  Keypair,
  Connection,
} from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  unpackAccount,
  getTransferFeeAmount,
} from "@solana/spl-token";
import { readFileSync } from "fs";

import { requireScriptEnv } from "../script-guard.js";
import {
  CCM_MINT,
  ORACLE_PROGRAM_ID,
  deriveProtocolState,
  deriveFeeConfig,
  deriveVault,
  deriveCcmBuffer,
} from "./lib/vault-pda.js";
import { CHANNELS } from "./lib/channels.js";
import { createLogger } from "./lib/logger.js";
import { runKeeperLoop } from "./lib/keeper-loop.js";
import { DRY_RUN, simulateAndLog } from "./lib/dry-run.js";

const INTERVAL_MS = Number(process.env.HARVEST_INTERVAL_MS || 3_600_000);
const MAX_SOURCES_PER_TX = 20; // limited by 1232-byte tx size, not governance.rs cap (30)
const RATE_LIMIT_DELAY_MS = 2_000;
const log = createLogger("harvest-fees");

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Load a Solana keypair from a JSON file path. */
function loadKeypairFromFile(filePath: string): Keypair {
  const raw = readFileSync(filePath, "utf-8");
  return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(raw)));
}

/** Discover candidate token accounts that may hold withheld transfer fees. */
async function discoverCandidates(
  connection: Connection,
  vaultBuffers: PublicKey[],
  treasuryAta: PublicKey,
): Promise<Set<string>> {
  const candidates = new Set<string>();

  // Source 1: Top holders via RPC (lightweight, ~20 accounts)
  try {
    const largest = await connection.getTokenLargestAccounts(CCM_MINT, "confirmed");
    for (const entry of largest.value) {
      candidates.add(entry.address.toBase58());
    }
  } catch (err: any) {
    log.warn("getTokenLargestAccounts failed, continuing with known accounts", {
      error: err.message,
    });
  }

  // Source 2: All vault CCM buffers (deterministic PDA derivation)
  for (const buf of vaultBuffers) {
    candidates.add(buf.toBase58());
  }

  // Source 3: Treasury ATA
  candidates.add(treasuryAta.toBase58());

  return candidates;
}

/** Filter candidate accounts to only those with withheld fees > 0. */
async function filterWithheldAccounts(
  connection: Connection,
  candidateKeys: PublicKey[],
): Promise<PublicKey[]> {
  const infos = await connection.getMultipleAccountsInfo(candidateKeys, "confirmed");
  const result: PublicKey[] = [];

  for (let i = 0; i < candidateKeys.length; i++) {
    const info = infos[i];
    if (!info) continue;
    try {
      const unpacked = unpackAccount(candidateKeys[i], info, TOKEN_2022_PROGRAM_ID);
      const feeAmount = getTransferFeeAmount(unpacked);
      if (feeAmount && feeAmount.withheldAmount > 0n) {
        result.push(candidateKeys[i]);
      }
    } catch {
      // Skip non-token or unparseable accounts
    }
  }

  return result;
}

/** Split an array into batches of at most `size` elements. */
function batchArray<T>(arr: T[], size: number): T[][] {
  const batches: T[][] = [];
  for (let i = 0; i < arr.length; i += size) {
    batches.push(arr.slice(i, i + size));
  }
  return batches;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main() {
  const env = requireScriptEnv();

  // Setup provider
  const adminKeypair = loadKeypairFromFile(env.keypairPath);
  const connection = new Connection(env.rpcUrl, "confirmed");
  const wallet = new anchor.Wallet(adminKeypair);
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
    preflightCommitment: "confirmed",
  });
  anchor.setProvider(provider);

  // Load Oracle IDL
  const oracleIdl = await Program.fetchIdl(ORACLE_PROGRAM_ID, provider);
  if (!oracleIdl) throw new Error("Oracle IDL not found on-chain");
  const oracleProgram = new Program(oracleIdl, provider);

  // Static accounts
  const protocolState = deriveProtocolState(CCM_MINT);
  const feeConfig = deriveFeeConfig(CCM_MINT);

  // Fetch treasury info from protocol state
  const protocolData: any =
    await oracleProgram.account.protocolState.fetch(protocolState);
  const treasuryOwner: PublicKey = protocolData.treasury;
  const treasuryAta = getAssociatedTokenAddressSync(
    CCM_MINT,
    treasuryOwner,
    true, // allowOwnerOffCurve
    TOKEN_2022_PROGRAM_ID,
  );

  log.info("Harvest fees keeper initialized", {
    admin: adminKeypair.publicKey.toBase58(),
    protocolState: protocolState.toBase58(),
    treasuryOwner: treasuryOwner.toBase58(),
    treasuryAta: treasuryAta.toBase58(),
    intervalMs: INTERVAL_MS,
    dryRun: DRY_RUN,
  });

  // Pre-derive all vault CCM buffer addresses (deterministic, no RPC needed)
  const vaultBuffers: PublicKey[] = CHANNELS.map((ch) => {
    const vault = deriveVault(new PublicKey(ch.channelConfig));
    return deriveCcmBuffer(vault);
  });

  async function tick() {
    log.info("Scanning for withheld fees...");

    // Step 1: Build candidate list
    const candidates = await discoverCandidates(connection, vaultBuffers, treasuryAta);
    log.info("Candidate accounts", { count: candidates.size });

    // Step 2: Filter for withheld fees > 0
    const candidateKeys = [...candidates].map((s) => new PublicKey(s));
    const withWithheld = await filterWithheldAccounts(connection, candidateKeys);

    if (withWithheld.length === 0) {
      log.info("No accounts with withheld fees");
      return;
    }

    log.info("Found accounts with withheld fees", {
      count: withWithheld.length,
    });

    // Step 3: Batch into transactions
    const batches = batchArray(withWithheld, MAX_SOURCES_PER_TX);
    let totalHarvested = 0;
    let txCount = 0;

    for (const batch of batches) {
      try {
        const builder = oracleProgram.methods
          .harvestFees()
          .accounts({
            authority: adminKeypair.publicKey,
            protocolState,
            feeConfig,
            mint: CCM_MINT,
            treasury: treasuryAta,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
          })
          .remainingAccounts(
            batch.map((pubkey) => ({
              pubkey,
              isSigner: false,
              isWritable: true,
            })),
          );

        if (DRY_RUN) {
          const sim = await simulateAndLog(
            connection,
            builder,
            adminKeypair.publicKey,
            log,
            `harvest-batch-${txCount}`,
          );
          if (sim.success) totalHarvested += batch.length;
          txCount++;
        } else {
          const tx = await builder.rpc({ commitment: "confirmed" });
          log.info("Harvest batch sent", {
            tx,
            batchSize: batch.length,
            batchIndex: txCount,
            totalBatches: batches.length,
          });
          totalHarvested += batch.length;
          txCount++;
        }

        // Rate-limit between batches
        if (txCount < batches.length) {
          await new Promise((r) => setTimeout(r, RATE_LIMIT_DELAY_MS));
        }
      } catch (err: any) {
        log.error("Harvest batch failed", {
          batchIndex: txCount,
          batchSize: batch.length,
          error: err.message,
          logs: err.logs?.slice(-5),
        });
      }
    }

    log.info("Harvest cycle complete", {
      accountsHarvested: totalHarvested,
      transactionsSent: txCount,
      totalBatches: batches.length,
      mode: DRY_RUN ? "dry-run" : "live",
    });
  }

  await runKeeperLoop(
    {
      name: "harvest-fees",
      intervalMs: INTERVAL_MS,
      maxRetries: 2,
      retryBaseMs: 5000,
    },
    tick,
  );
}

main().catch((err) => {
  log.error("Fatal", { error: err.message });
  process.exit(1);
});
