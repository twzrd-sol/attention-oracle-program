/**
 * Harvest Fees Keeper — Admin crank for Token-2022 transfer fee collection.
 *
 * Periodically discovers CCM token accounts with withheld transfer fees,
 * then calls harvest_fees() to sweep them to the protocol treasury.
 *
 * REQUIRES admin keypair (checked against protocol_state.admin on-chain).
 *
 * Environment:
 *   CLUSTER=mainnet-beta   (or devnet)
 *   I_UNDERSTAND_MAINNET=1 (for mainnet)
 *   RPC_URL=https://...
 *   KEYPAIR=/secure/path/admin.json
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
  CCM_V3_MINT,
  PROGRAM_ID as ORACLE_PROGRAM_ID,
} from "../config.js";
import {
  deriveProtocolState,
  deriveFeeConfig,
} from "./lib/vault-pda.js";
import { createLogger } from "./lib/logger.js";
import { runKeeperLoop } from "./lib/keeper-loop.js";
import { DRY_RUN, simulateAndLog } from "./lib/dry-run.js";

const INTERVAL_MS = Number(process.env.HARVEST_INTERVAL_MS || 3_600_000);
const MAX_SOURCES_PER_TX = 30; // governance.rs cap
const log = createLogger("harvest-fees");

async function main() {
  const env = requireScriptEnv();

  // Setup provider (admin keypair required)
  const keypairData = JSON.parse(readFileSync(env.keypairPath, "utf-8"));
  const adminKeypair = Keypair.fromSecretKey(Uint8Array.from(keypairData));
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
  const protocolState = deriveProtocolState(CCM_V3_MINT);
  const feeConfig = deriveFeeConfig(CCM_V3_MINT);

  // Fetch treasury info from protocol state
  const protocolData: any =
    await oracleProgram.account.protocolState.fetch(protocolState);
  const treasuryOwner: PublicKey = protocolData.treasury;
  const treasuryAta = getAssociatedTokenAddressSync(
    CCM_V3_MINT,
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

  async function tick() {
    // Step 1: Find all CCM token accounts
    log.info("Scanning for withheld fees...");

    const allAccounts = await connection.getProgramAccounts(
      TOKEN_2022_PROGRAM_ID,
      {
        filters: [
          {
            memcmp: {
              offset: 0,
              bytes: CCM_V3_MINT.toBase58(),
            },
          },
        ],
      },
    );

    log.info("Scanned token accounts", { total: allAccounts.length });

    // Step 2: Filter for accounts with withheld fees > 0
    const withWithheld: PublicKey[] = [];

    for (const { pubkey, account } of allAccounts) {
      try {
        const unpacked = unpackAccount(pubkey, account, TOKEN_2022_PROGRAM_ID);
        const feeAmount = getTransferFeeAmount(unpacked);
        if (feeAmount && feeAmount.withheldAmount > 0n) {
          withWithheld.push(pubkey);
        }
      } catch {
        // Skip unparseable accounts
      }
    }

    if (withWithheld.length === 0) {
      log.info("No accounts with withheld fees");
      return;
    }

    log.info("Found accounts with withheld fees", {
      count: withWithheld.length,
    });

    // Step 3: Batch into transactions (max 30 sources per tx)
    const batches: PublicKey[][] = [];
    for (let i = 0; i < withWithheld.length; i += MAX_SOURCES_PER_TX) {
      batches.push(withWithheld.slice(i, i + MAX_SOURCES_PER_TX));
    }

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
            mint: CCM_V3_MINT,
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
            connection, builder, adminKeypair.publicKey, log,
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
          await new Promise((r) => setTimeout(r, 2000));
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
