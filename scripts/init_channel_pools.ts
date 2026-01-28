/**
 * Initialize Stake Pools for Channels
 *
 * This script creates ChannelStakePool + vault accounts for each channel.
 * Must be run ONCE per channel before staking is enabled.
 *
 * Usage:
 *   RPC_URL=<your-rpc> KEYPAIR=<path-to-keypair> npx ts-node scripts/init_channel_pools.ts
 */

import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  sendAndConfirmTransaction,
  ComputeBudgetProgram,
  SystemProgram,
} from "@solana/web3.js";
import { Program, AnchorProvider, Wallet } from "@coral-xyz/anchor";
import * as fs from "fs";
import * as path from "path";
import { TOKEN_2022_PROGRAM_ID } from "@solana/spl-token";
import { keccak_256 } from "@noble/hashes/sha3";

// =============================================================================
// CONFIG
// =============================================================================

const PROGRAM_ID = new PublicKey(
  "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
);

// Replace with your CCM mint
const CCM_MINT = new PublicKey(
  "Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM"
);

// PDA Seeds (must match on-chain)
const SEEDS = {
  PROTOCOL: Buffer.from("protocol"),
  CHANNEL_CONFIG_V2: Buffer.from("channel_cfg_v2"),
  CHANNEL_STAKE_POOL: Buffer.from("channel_pool"),
  STAKE_VAULT: Buffer.from("stake_vault"),
};

// ============================================================================
// CHANNELS TO INITIALIZE
// ============================================================================
// Add your channel names here. These should already have ChannelConfigV2 accounts.
//
// Format: "<namespace>:<subject>"
//
// Configure this list for your deployment environment.

const CHANNELS_TO_INIT = [
  // Add your channels here
  // "example:channel1",
  // "example:channel2",
];

// =============================================================================
// HELPERS
// =============================================================================

/**
 * Derive subject ID using Keccak256 hash (matches on-chain derivation)
 */
function deriveSubjectId(channelName: string): Buffer {
  const lower = channelName.toLowerCase();
  const input = Buffer.concat([
    Buffer.from("channel:"),
    Buffer.from(lower),
  ]);
  return Buffer.from(keccak_256(input));
}

/**
 * Parse environment for RPC URL and keypair
 */
function getEnv() {
  const rpcUrl = process.env.RPC_URL;
  const keypairPath = process.env.KEYPAIR;

  if (!rpcUrl) {
    console.error(`❌ RPC_URL environment variable not set`);
    process.exit(1);
  }

  if (!keypairPath) {
    console.error(`❌ KEYPAIR environment variable not set`);
    process.exit(1);
  }

  if (!fs.existsSync(keypairPath)) {
    console.error(`❌ Keypair not found at ${keypairPath}`);
    process.exit(1);
  }

  return { rpcUrl, keypairPath };
}

/**
 * Format lamports to SOL
 */
function toSOL(lamports: number): string {
  return (lamports / 1_000_000_000).toFixed(4);
}

// =============================================================================
// MAIN
// =============================================================================

async function main() {
  const { rpcUrl, keypairPath } = getEnv();

  console.log("\n🔗 Connecting to RPC...");
  const connection = new Connection(rpcUrl, "confirmed");

  // Load keypair
  const keypairJson = JSON.parse(fs.readFileSync(keypairPath, "utf-8"));
  const payer = Keypair.fromSecretKey(Uint8Array.from(keypairJson));
  const wallet = new Wallet(payer);
  const provider = new AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });

  // Load IDL
  const idlPath = path.join(__dirname, "../target/idl/token_2022.json");
  if (!fs.existsSync(idlPath)) {
    console.error(
      "❌ IDL not found at",
      idlPath,
      "- Run 'anchor build' first"
    );
    process.exit(1);
  }

  const idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
  const program = new Program(idl, PROGRAM_ID, provider);

  // Get payer balance
  const balance = await connection.getBalance(payer.publicKey);
  console.log(`✓ Connected to ${rpcUrl}`);
  console.log(`✓ Payer: ${payer.publicKey.toBase58()}`);
  console.log(`✓ Balance: ${toSOL(balance)} SOL`);
  console.log(`✓ Program: ${PROGRAM_ID.toBase58()}`);
  console.log(`✓ Mint: ${CCM_MINT.toBase58()}`);

  if (balance < 5_000_000) {
    console.error(
      "❌ Insufficient SOL balance (need ~0.005 SOL per pool + fees)"
    );
    process.exit(1);
  }

  // ==========================================================================
  // INITIALIZE POOLS
  // ==========================================================================

  console.log(
    `\n📦 Initializing ${CHANNELS_TO_INIT.length} stake pool(s)...\n`
  );

  let successCount = 0;
  let skipCount = 0;
  let failureCount = 0;

  for (const channel of CHANNELS_TO_INIT) {
    console.log(`\n${"=".repeat(70)}`);
    console.log(`Channel: ${channel}`);
    console.log("=".repeat(70));

    try {
      // 1. Derive Channel Config PDA
      const subjectId = deriveSubjectId(channel);
      const [channelConfig] = PublicKey.findProgramAddressSync(
        [SEEDS.CHANNEL_CONFIG_V2, CCM_MINT.toBuffer(), subjectId],
        PROGRAM_ID
      );

      console.log(`Channel Config: ${channelConfig.toBase58()}`);

      // Verify Channel Config exists
      const configAccount = await connection.getAccountInfo(channelConfig);
      if (!configAccount) {
        console.error(
          `❌ Channel Config not found. Has this channel been initialized?`
        );
        failureCount++;
        continue;
      }

      // 2. Derive Stake Pool PDA
      const [stakePool] = PublicKey.findProgramAddressSync(
        [SEEDS.CHANNEL_STAKE_POOL, channelConfig.toBuffer()],
        PROGRAM_ID
      );

      console.log(`Stake Pool: ${stakePool.toBase58()}`);

      // Check if already initialized
      const poolAccount = await connection.getAccountInfo(stakePool);
      if (poolAccount) {
        console.log(
          `⚠️  Stake Pool already exists (rent-exempt: ${toSOL(poolAccount.lamports)} SOL)`
        );
        skipCount++;
        continue;
      }

      // 3. Derive Vault PDA
      const [vault] = PublicKey.findProgramAddressSync(
        [SEEDS.STAKE_VAULT, stakePool.toBuffer()],
        PROGRAM_ID
      );

      console.log(`Vault: ${vault.toBase58()}`);

      // 4. Build transaction
      console.log("\n⏳ Building transaction...");
      const tx = await program.methods
        .initializeStakePool()
        .accounts({
          payer: payer.publicKey,
          protocolState: PublicKey.findProgramAddressSync(
            [SEEDS.PROTOCOL, CCM_MINT.toBuffer()],
            PROGRAM_ID
          )[0],
          channelConfig: channelConfig,
          stakePool: stakePool,
          mint: CCM_MINT,
          vault: vault,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .transaction();

      // Add compute budget
      tx.add(
        ComputeBudgetProgram.setComputeUnitLimit({
          units: 200_000,
        })
      );

      // 5. Send and confirm
      console.log("📤 Sending transaction...");
      const sig = await sendAndConfirmTransaction(connection, tx, [payer], {
        commitment: "confirmed",
      });

      console.log(`✅ Success!`);
      console.log(`   Tx: https://solscan.io/tx/${sig}`);
      successCount++;
    } catch (error) {
      console.error(`❌ Failed:`);
      if (error instanceof Error) {
        console.error(`   ${error.message}`);
        if (error.stack) {
          const lines = error.stack.split("\n");
          console.error(`   ${lines[1]}`); // Show first stack line
        }
      }
      failureCount++;
    }
  }

  // ==========================================================================
  // SUMMARY
  // ==========================================================================

  console.log(`\n${"=".repeat(70)}`);
  console.log("SUMMARY");
  console.log("=".repeat(70));
  console.log(`✅ Success:  ${successCount}`);
  console.log(`⚠️  Skipped:  ${skipCount} (already initialized)`);
  console.log(`❌ Failed:   ${failureCount}`);
  console.log("");

  if (successCount > 0) {
    console.log("🎉 Staking is now enabled for the initialized channels!");
    console.log("   Users can now call stake_channel() to begin staking.");
  }

  if (failureCount > 0) {
    console.log(
      "⚠️  Some channels failed to initialize. Check error messages above."
    );
    process.exit(1);
  }
}

main().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(1);
});
