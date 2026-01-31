/**
 * Compound Keeper — Permissionless crank for vault compounding.
 *
 * Periodically checks all vaults and calls compound() when eligible:
 *   - Pending deposits exist (stakeable > 0), OR
 *   - Active Oracle position with expired lock
 *
 * The payer does NOT need admin authority — compound is permissionless.
 * Any funded wallet can run this keeper (only needs SOL for tx fees).
 *
 * Environment:
 *   CLUSTER=mainnet-beta   (or devnet)
 *   RPC_URL=https://...
 *   KEYPAIR=~/.config/solana/keeper.json
 *   COMPOUND_INTERVAL_MS=300000   (optional, default 5 min)
 *
 * Usage:
 *   CLUSTER=mainnet-beta RPC_URL=... KEYPAIR=... \
 *     npx tsx scripts/keepers/compound-keeper.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  Connection,
} from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { readFileSync } from "fs";

import { requireScriptEnv } from "../script-guard.js";
import { CCM_V3_MINT } from "../config.js";
import { CHANNELS } from "./lib/channels.js";
import {
  VAULT_PROGRAM_ID,
  deriveVault,
  deriveOraclePosition,
  deriveCompoundAccounts,
} from "./lib/vault-pda.js";
import { createLogger } from "./lib/logger.js";
import { runKeeperLoop } from "./lib/keeper-loop.js";
import { DRY_RUN, simulateAndLog } from "./lib/dry-run.js";

const INTERVAL_MS = Number(process.env.COMPOUND_INTERVAL_MS || 300_000);
const log = createLogger("compound");

async function main() {
  const env = requireScriptEnv();

  // Setup provider
  const keypairData = JSON.parse(readFileSync(env.keypairPath, "utf-8"));
  const payerKeypair = Keypair.fromSecretKey(Uint8Array.from(keypairData));
  const connection = new Connection(env.rpcUrl, "confirmed");
  const wallet = new anchor.Wallet(payerKeypair);
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
    preflightCommitment: "confirmed",
  });
  anchor.setProvider(provider);

  // Load vault IDL
  const vaultIdl = await Program.fetchIdl(VAULT_PROGRAM_ID, provider);
  if (!vaultIdl) throw new Error("Vault IDL not found on-chain");
  const vaultProgram = new Program(vaultIdl, provider);

  log.info("Compound keeper initialized", {
    payer: payerKeypair.publicKey.toBase58(),
    cluster: env.cluster,
    channels: CHANNELS.length,
    intervalMs: INTERVAL_MS,
    dryRun: DRY_RUN,
  });

  async function tick() {
    const currentSlot = await connection.getSlot("confirmed");
    let compounded = 0;
    let skipped = 0;
    let errors = 0;

    for (const ch of CHANNELS) {
      const channelConfig = new PublicKey(ch.channelConfig);
      const vaultPda = deriveVault(channelConfig);
      const positionPda = deriveOraclePosition(vaultPda);

      try {
        // Fetch vault state
        const vault: any = await vaultProgram.account.channelVault.fetchNullable(
          vaultPda,
        );
        if (!vault) {
          skipped++;
          continue;
        }
        if (vault.paused) {
          log.debug("Vault paused", { channel: ch.name });
          skipped++;
          continue;
        }

        // Fetch oracle position
        const position: any =
          await vaultProgram.account.vaultOraclePosition.fetchNullable(
            positionPda,
          );
        if (!position) {
          log.warn("Oracle position missing", { channel: ch.name });
          skipped++;
          continue;
        }

        // Check compoundability
        const pendingDeposits = Number(vault.pendingDeposits);
        const pendingWithdrawals = Number(vault.pendingWithdrawals);
        const stakeable = Math.max(0, pendingDeposits - pendingWithdrawals);
        const isActive: boolean = position.isActive;
        const lockEndSlot = Number(position.lockEndSlot);

        // Need stakeable deposits OR an active position to roll over
        if (stakeable === 0 && !isActive) {
          skipped++;
          continue;
        }

        // If active and lock hasn't expired, skip
        if (isActive && lockEndSlot > currentSlot) {
          log.debug("Lock not expired", {
            channel: ch.name,
            slotsRemaining: lockEndSlot - currentSlot,
          });
          skipped++;
          continue;
        }

        // Eligible — build and send compound tx
        log.info("Compounding", {
          channel: ch.name,
          stakeable,
          isActive,
          lockEndSlot,
          currentSlot,
        });

        const accounts = deriveCompoundAccounts(channelConfig, CCM_V3_MINT);

        const builder = vaultProgram.methods
          .compound()
          .accounts({
            payer: payerKeypair.publicKey,
            vault: accounts.vault,
            vaultOraclePosition: accounts.vaultOraclePosition,
            vaultCcmBuffer: accounts.vaultCcmBuffer,
            ccmMint: accounts.ccmMint,
            oracleProgram: accounts.oracleProgram,
            oracleProtocol: accounts.oracleProtocol,
            oracleChannelConfig: accounts.oracleChannelConfig,
            oracleStakePool: accounts.oracleStakePool,
            oracleVault: accounts.oracleVault,
            oracleUserStake: accounts.oracleUserStake,
            oracleNftMint: accounts.oracleNftMint,
            vaultNftAta: accounts.vaultNftAta,
            token2022Program: TOKEN_2022_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
            rent: SYSVAR_RENT_PUBKEY,
          });

        if (DRY_RUN) {
          const sim = await simulateAndLog(
            connection, builder, payerKeypair.publicKey, log, ch.name,
          );
          sim.success ? compounded++ : errors++;
        } else {
          const tx = await builder.rpc({ commitment: "confirmed" });
          log.info("Compounded successfully", { channel: ch.name, tx });
          compounded++;
        }
      } catch (err: any) {
        // NothingToCompound / OracleStakeLocked are expected — log at debug
        const msg: string = err.message || "";
        if (msg.includes("NothingToCompound") || msg.includes("OracleStakeLocked")) {
          log.debug("Expected skip", { channel: ch.name, reason: msg });
          skipped++;
        } else {
          log.error("Failed to compound", {
            channel: ch.name,
            error: msg,
            logs: err.logs?.slice(-5),
          });
          errors++;
        }
      }
    }

    log.info("Tick complete", {
      compounded, skipped, errors, currentSlot,
      mode: DRY_RUN ? "dry-run" : "live",
    });
  }

  await runKeeperLoop(
    {
      name: "compound",
      intervalMs: INTERVAL_MS,
      maxRetries: 3,
      retryBaseMs: 2000,
    },
    tick,
  );
}

main().catch((err) => {
  log.error("Fatal", { error: err.message });
  process.exit(1);
});
