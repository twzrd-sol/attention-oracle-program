/**
 * Add liquidity to Meteora DLMM pool programmatically.
 *
 * Usage:
 *   RPC_URL="https://api.mainnet-beta.solana.com" \
 *   KEYPAIR=~/.config/solana/id.json \
 *   npx tsx scripts/add-dlmm-liquidity.ts
 */

import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import DLMM from "@meteora-ag/dlmm";
import BN from "bn.js";
import * as fs from "fs";

// ============================================================================
// Configuration
// ============================================================================

const POOL_ADDRESS = new PublicKey("CEt6qy87ozwmoTGeSXyx4eSD1w33LvRrGA645d67yH3M");
const VLOFI_MINT = new PublicKey("E9Kt33axpCy3ve2PCY9BSrbPhcR9wdDsWQECAahzw2dS");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");

// Amounts to add (in token units, not lamports)
const VLOFI_AMOUNT = 75_000;
const CCM_AMOUNT = 66_863; // Balanced at ~0.891 price

const DECIMALS = 9;

// ============================================================================
// Helpers
// ============================================================================

function loadKeypair(path: string): Keypair {
  const expanded = path.replace("~", process.env.HOME || "");
  const raw = JSON.parse(fs.readFileSync(expanded, "utf-8"));
  return Keypair.fromSecretKey(new Uint8Array(raw));
}

function toRawAmount(amount: number): BN {
  return new BN(Math.floor(amount * 10 ** DECIMALS));
}

// ============================================================================
// Main
// ============================================================================

async function main() {
  const rpcUrl = process.env.RPC_URL;
  if (!rpcUrl) {
    console.error("ERROR: RPC_URL required");
    process.exit(1);
  }

  const keypairPath = process.env.KEYPAIR;
  if (!keypairPath) {
    console.error("ERROR: KEYPAIR required");
    process.exit(1);
  }

  const user = loadKeypair(keypairPath);
  const connection = new Connection(rpcUrl, "confirmed");

  console.log("=".repeat(60));
  console.log("  Add DLMM Liquidity");
  console.log("=".repeat(60));
  console.log(`  Pool:    ${POOL_ADDRESS.toBase58()}`);
  console.log(`  User:    ${user.publicKey.toBase58()}`);
  console.log(`  vLOFI:   ${VLOFI_AMOUNT.toLocaleString()}`);
  console.log(`  CCM:     ${CCM_AMOUNT.toLocaleString()}`);
  console.log();

  // Check balances
  const userVlofi = getAssociatedTokenAddressSync(VLOFI_MINT, user.publicKey, false, TOKEN_PROGRAM_ID);
  const userCcm = getAssociatedTokenAddressSync(CCM_MINT, user.publicKey, false, TOKEN_2022_PROGRAM_ID);

  const [vlofiBalance, ccmBalance] = await Promise.all([
    connection.getTokenAccountBalance(userVlofi),
    connection.getTokenAccountBalance(userCcm),
  ]);

  console.log(`  vLOFI Balance: ${vlofiBalance.value.uiAmountString}`);
  console.log(`  CCM Balance:   ${ccmBalance.value.uiAmountString}`);
  console.log();

  // Load DLMM pool
  console.log("  Loading DLMM pool...");
  const dlmmPool = await DLMM.create(connection, POOL_ADDRESS);

  // Get active bin (current price)
  const activeBin = await dlmmPool.getActiveBin();
  console.log(`  Active Bin ID: ${activeBin.binId}`);
  console.log(`  Active Bin Price: ${activeBin.pricePerToken}`);
  console.log();

  // Calculate bin range for spot liquidity (±15 bins around active)
  const binRange = 15;
  const minBinId = activeBin.binId - binRange;
  const maxBinId = activeBin.binId + binRange;

  console.log(`  Adding liquidity from bin ${minBinId} to ${maxBinId}`);
  console.log();

  // Create position with spot distribution
  const totalXAmount = toRawAmount(VLOFI_AMOUNT);
  const totalYAmount = toRawAmount(CCM_AMOUNT);

  // Get the add liquidity transaction
  const addLiquidityTx = await dlmmPool.addLiquidityByStrategy({
    user: user.publicKey,
    totalXAmount,
    totalYAmount,
    strategy: {
      maxBinId,
      minBinId,
      strategyType: 0, // Spot
    },
  });

  console.log("  Sending transaction...");

  // Send transaction
  const txHash = await connection.sendTransaction(addLiquidityTx, [user], {
    skipPreflight: false,
  });

  console.log(`  TX: ${txHash}`);

  // Confirm
  await connection.confirmTransaction(txHash, "confirmed");
  console.log("  Confirmed!");
  console.log();

  // Get updated position
  const positions = await dlmmPool.getPositionsByUserAndLbPair(user.publicKey);
  console.log(`  Total Positions: ${positions.userPositions.length}`);

  if (positions.userPositions.length > 0) {
    const pos = positions.userPositions[0];
    console.log(`  Position Address: ${pos.publicKey.toBase58()}`);
  }

  console.log();
  console.log("=".repeat(60));
  console.log("  Liquidity Added Successfully!");
  console.log("=".repeat(60));
  console.log();
  console.log(`  View pool: https://www.meteora.ag/dlmm/${POOL_ADDRESS.toBase58()}`);
}

main().catch((err) => {
  console.error("\nError:", err.message || err);
  if (err.logs) {
    console.error("\nProgram logs:");
    for (const log of err.logs) {
      console.error("  ", log);
    }
  }
  process.exit(1);
});
