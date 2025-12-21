#!/usr/bin/env ts-node
/**
 * enumerate-epoch-states.ts
 *
 * Enumerates all EpochState accounts owned by the attention oracle program.
 * Outputs a JSON report showing:
 * - Total accounts
 * - Open vs closed
 * - Total reclaimable rent
 *
 * Usage:
 *   ts-node enumerate-epoch-states.ts
 *   ts-node enumerate-epoch-states.ts --json > report.json
 */

import { Connection, PublicKey } from "@solana/web3.js";
import { requireScriptEnv } from "./script-guard.js";

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");

// EpochState discriminator (sha256("account:EpochState")[0..8])
// May need to adjust if different
const EPOCH_STATE_DISCRIMINATOR = Buffer.from([0x23, 0x2f, 0xc3, 0x33, 0x94, 0x7b, 0xf2, 0x89]);

interface EpochStateInfo {
  pubkey: string;
  epoch: string;
  lamports: number;
  solAmount: number;
  dataLength: number;
}

async function main() {
  const jsonOutput = process.argv.includes("--json");

  const { rpcUrl } = requireScriptEnv();
  const connection = new Connection(rpcUrl, "confirmed");

  if (!jsonOutput) {
    console.log(`Program ID: ${PROGRAM_ID.toString()}`);
    console.log(`RPC: ${rpcUrl}\n`);
    console.log("Fetching program accounts...\n");
  }

  // Get ALL accounts owned by program
  const allAccounts = await connection.getProgramAccounts(PROGRAM_ID);

  if (!jsonOutput) {
    console.log(`Total program accounts: ${allAccounts.length}\n`);
  }

  // Filter for EpochState accounts by size
  // EpochState is typically 296 bytes or similar
  // We'll look at accounts that aren't ChannelState (which is ~5.7KB or ~1.1MB)
  const epochStates: EpochStateInfo[] = [];
  const channelStates: { pubkey: string; size: number }[] = [];
  const otherAccounts: { pubkey: string; size: number }[] = [];

  for (const { pubkey, account } of allAccounts) {
    const size = account.data.length;

    // ChannelState is large (728 bytes old, 5688 bytes new, or ~1.1MB after resize)
    if (size >= 700) {
      channelStates.push({
        pubkey: pubkey.toString(),
        size,
      });
      continue;
    }

    // EpochState is typically 136-296 bytes depending on bitmap size
    if (size >= 100 && size < 700) {
      try {
        // Try to extract epoch from the data
        // Layout varies but epoch is usually at offset 8 (after discriminator)
        const epoch = account.data.readBigUInt64LE(8);

        epochStates.push({
          pubkey: pubkey.toString(),
          epoch: epoch.toString(),
          lamports: account.lamports,
          solAmount: account.lamports / 1e9,
          dataLength: size,
        });
      } catch {
        otherAccounts.push({
          pubkey: pubkey.toString(),
          size,
        });
      }
      continue;
    }

    // Everything else
    otherAccounts.push({
      pubkey: pubkey.toString(),
      size,
    });
  }

  // Sort epoch states by epoch
  epochStates.sort((a, b) => Number(BigInt(a.epoch) - BigInt(b.epoch)));

  // Calculate totals
  const totalEpochLamports = epochStates.reduce((sum, e) => sum + e.lamports, 0);
  const totalEpochSol = totalEpochLamports / 1e9;

  const report = {
    summary: {
      totalProgramAccounts: allAccounts.length,
      epochStateAccounts: epochStates.length,
      channelStateAccounts: channelStates.length,
      otherAccounts: otherAccounts.length,
      totalReclaimableLamports: totalEpochLamports,
      totalReclaimableSol: totalEpochSol.toFixed(6),
    },
    epochStates,
    channelStates,
  };

  if (jsonOutput) {
    console.log(JSON.stringify(report, null, 2));
  } else {
    console.log("=== Account Summary ===\n");
    console.log(`EpochState accounts:   ${epochStates.length}`);
    console.log(`ChannelState accounts: ${channelStates.length}`);
    console.log(`Other accounts:        ${otherAccounts.length}`);
    console.log(`\nTotal reclaimable from EpochState: ${totalEpochSol.toFixed(6)} SOL\n`);

    if (epochStates.length > 0) {
      console.log("=== EpochState Accounts ===\n");
      for (const es of epochStates.slice(0, 20)) {
        console.log(`  epoch=${es.epoch.padStart(10)} | ${es.solAmount.toFixed(6)} SOL | ${es.pubkey.slice(0, 20)}...`);
      }
      if (epochStates.length > 20) {
        console.log(`  ... and ${epochStates.length - 20} more`);
      }
    }

    if (channelStates.length > 0) {
      console.log("\n=== ChannelState Accounts ===\n");
      for (const cs of channelStates.slice(0, 10)) {
        console.log(`  ${cs.pubkey.slice(0, 20)}... | ${cs.size} bytes`);
      }
      if (channelStates.length > 10) {
        console.log(`  ... and ${channelStates.length - 10} more`);
      }
    }
  }
}

main().catch(console.error);
