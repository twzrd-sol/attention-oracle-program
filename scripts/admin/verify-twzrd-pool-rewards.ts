/**
 * Verify that the 5 TWZRD pools now have nonzero reward_per_slot
 *
 * Run this after executing the Squads proposal to activate rewards.
 *
 * Usage:
 *   RPC_URL="..." npx tsx scripts/admin/verify-twzrd-pool-rewards.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, PublicKey } from "@solana/web3.js";
import { CHANNELS } from "../keepers/lib/channels.js";

// ============================================================================
// Constants
// ============================================================================

const ORACLE_PROGRAM_ID = new PublicKey(
  "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop",
);

const CHANNEL_STAKE_POOL_SEED = Buffer.from("channel_pool");

const TWZRD_POOL_NAMES = [
  "twzrd-247-6h",
  "twzrd-1999-6h",
  "twzrd-415-6h",
  "twzrd-3121-6h",
  "twzrd-69-6h",
];

const TWZRD_POOLS = CHANNELS.filter((ch) =>
  TWZRD_POOL_NAMES.includes(ch.name)
);

const EXPECTED_RATE = 1_500;

// ============================================================================
// Main
// ============================================================================

async function main() {
  const rpcUrl = process.env.RPC_URL;
  if (!rpcUrl) {
    console.error("ERROR: Set RPC_URL environment variable");
    console.error(
      '  RPC_URL="https://..." npx tsx scripts/admin/verify-twzrd-pool-rewards.ts',
    );
    process.exit(1);
  }

  const connection = new Connection(rpcUrl, "confirmed");
  const wallet = new anchor.Wallet(anchor.web3.Keypair.generate());
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });
  anchor.setProvider(provider);

  const oracleIdl = await Program.fetchIdl(ORACLE_PROGRAM_ID, provider);
  if (!oracleIdl) throw new Error("Oracle IDL not found on-chain");
  const oracleProgram = new Program(oracleIdl, provider);

  console.log("\n" + "=".repeat(70));
  console.log("  TWZRD POOL REWARD VERIFICATION");
  console.log("=".repeat(70));
  console.log(`\n  Expected rate: ${EXPECTED_RATE} per slot\n`);

  let allPassed = true;

  for (const pool of TWZRD_POOLS) {
    const channelConfig = new PublicKey(pool.channelConfig);
    const [stakePool] = PublicKey.findProgramAddressSync(
      [CHANNEL_STAKE_POOL_SEED, channelConfig.toBuffer()],
      ORACLE_PROGRAM_ID,
    );

    try {
      const poolData: any =
        await oracleProgram.account.channelStakePool.fetch(stakePool);

      const rewardPerSlot = Number(poolData.rewardPerSlot.toString());
      const status = rewardPerSlot === EXPECTED_RATE ? "✅ PASS" : "❌ FAIL";

      console.log(`  ${pool.name.padEnd(18)} ${status}  rate=${rewardPerSlot}`);

      if (rewardPerSlot !== EXPECTED_RATE) {
        allPassed = false;
      }
    } catch (err: any) {
      console.log(`  ${pool.name.padEnd(18)} ❌ ERROR ${err.message}`);
      allPassed = false;
    }
  }

  console.log();
  if (allPassed) {
    console.log("  ✅ All 5 pools have the expected reward rate!");
  } else {
    console.log("  ❌ Some pools failed verification. Check the proposal status.");
    process.exit(1);
  }
  console.log();
}

main().catch((err) => {
  console.error("\nError:", err.message || err);
  process.exit(1);
});
