/**
 * Dry-run utilities for keeper processes.
 *
 * When DRY_RUN=1 is set, keepers simulate transactions via
 * connection.simulateTransaction() instead of calling .rpc().
 * No signing key is needed — simulateTransaction defaults to
 * sigVerify: false.
 */

import { Connection, PublicKey, Transaction } from "@solana/web3.js";
import type { Logger } from "./logger.js";

export const DRY_RUN =
  process.env.DRY_RUN === "1" || process.env.DRY_RUN === "true";

export interface SimulationResult {
  success: boolean;
  error: unknown;
  unitsConsumed: number | undefined;
  logs: string[] | null;
}

/**
 * Build a Transaction from an Anchor MethodsBuilder, simulate it,
 * and log the result. Never sends a real transaction.
 *
 * @param connection - Solana RPC connection
 * @param builder    - Anchor MethodsBuilder (post-.accounts(), pre-.rpc())
 * @param payer      - Fee payer public key (for tx.feePayer)
 * @param log        - Structured logger instance
 * @param label      - Human-readable label for log entries
 */
export async function simulateAndLog(
  connection: Connection,
  builder: { transaction(): Promise<Transaction> },
  payer: PublicKey,
  log: Logger,
  label: string,
): Promise<SimulationResult> {
  const tx = await builder.transaction();
  tx.feePayer = payer;
  tx.recentBlockhash = (
    await connection.getLatestBlockhash("confirmed")
  ).blockhash;

  const sim = await connection.simulateTransaction(tx);

  const result: SimulationResult = {
    success: sim.value.err === null,
    error: sim.value.err,
    unitsConsumed: sim.value.unitsConsumed,
    logs: sim.value.logs,
  };

  const ixSummary = tx.instructions.map((ix, i) => ({
    ix: i,
    programId: ix.programId.toBase58(),
    numKeys: ix.keys.length,
    dataLen: ix.data.length,
  }));

  log.info("DRY RUN simulation", {
    label,
    success: result.success,
    error: result.error,
    unitsConsumed: result.unitsConsumed,
    instructionCount: tx.instructions.length,
    instructions: ixSummary,
    logs: result.logs?.slice(-10),
  });

  return result;
}
