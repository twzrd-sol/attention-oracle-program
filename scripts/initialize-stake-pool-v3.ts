#!/usr/bin/env ts-node

/**
 * Initialize stake pool for CCM-v3 mint
 *
 * Creates:
 * - stake_pool PDA (holds pool state)
 * - stake_vault ATA (holds staked CCM)
 *
 * Config:
 * - reward_rate = 0 (no inflation, revenue-backed only)
 * - min_stake = 1 CCM (enforced in stake ix)
 * - max_lock = 30 days (enforced in stake ix)
 *
 * Usage:
 *   npx ts-node scripts/initialize-stake-pool-v3.ts [--dry-run]
 */

import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
  ComputeBudgetProgram,
  TransactionInstruction,
} from "@solana/web3.js";
import { TOKEN_2022_PROGRAM_ID } from "@solana/spl-token";
import fs from "fs";

import {
  PROGRAM_ID,
  CCM_V3_MINT,
  PROTOCOL_SEED,
  STAKE_POOL_SEED,
  STAKE_VAULT_SEED,
  getRpcUrl,
  getWalletPath,
} from "./config.js";

// Stake pool config
const REWARD_RATE = BigInt(0); // No inflation - revenue-backed only

// initialize_stake_pool discriminator from IDL
const INIT_STAKE_POOL_DISCRIMINATOR = Buffer.from([48, 189, 243, 73, 19, 67, 36, 83]);

async function main() {
  console.log("=== CCM-v3 Stake Pool Initialization ===\n");

  // Load admin wallet
  const admin = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(getWalletPath(), "utf-8")))
  );
  console.log("Admin Wallet:", admin.publicKey.toBase58());

  // Setup connection
  const rpcUrl = getRpcUrl();
  const connection = new Connection(rpcUrl, "confirmed");
  console.log("RPC:", rpcUrl.substring(0, 50) + "...");
  console.log("Mint:", CCM_V3_MINT.toBase58());

  // Derive PDAs
  const [protocolStatePda] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, CCM_V3_MINT.toBuffer()],
    PROGRAM_ID
  );

  const [stakePoolPda, stakePoolBump] = PublicKey.findProgramAddressSync(
    [STAKE_POOL_SEED, CCM_V3_MINT.toBuffer()],
    PROGRAM_ID
  );

  const [stakeVaultPda, stakeVaultBump] = PublicKey.findProgramAddressSync(
    [STAKE_VAULT_SEED, CCM_V3_MINT.toBuffer()],
    PROGRAM_ID
  );

  console.log("\n=== Derived PDAs ===");
  console.log("Protocol State:", protocolStatePda.toBase58());
  console.log("Stake Pool:", stakePoolPda.toBase58());
  console.log("Stake Pool Bump:", stakePoolBump);
  console.log("Stake Vault:", stakeVaultPda.toBase58());
  console.log("Stake Vault Bump:", stakeVaultBump);

  console.log("\n=== Config ===");
  console.log("Reward Rate:", REWARD_RATE.toString(), "(0 = no inflation)");
  console.log("Min Stake: 1 CCM (enforced in stake ix)");
  console.log("Max Lock: 30 days (enforced in stake ix)");

  // Check if stake_pool already exists
  const poolInfo = await connection.getAccountInfo(stakePoolPda);
  if (poolInfo) {
    console.log("\n⚠️  Stake Pool already initialized!");
    console.log("Account exists at:", stakePoolPda.toBase58());
    process.exit(0);
  }

  // Build instruction data: discriminator (8) + reward_rate (8, little-endian u64)
  const data = Buffer.alloc(8 + 8);
  INIT_STAKE_POOL_DISCRIMINATOR.copy(data, 0);
  data.writeBigUInt64LE(REWARD_RATE, 8);

  // Build instruction
  const ix = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: admin.publicKey, isSigner: true, isWritable: true },
      { pubkey: protocolStatePda, isSigner: false, isWritable: false },
      { pubkey: CCM_V3_MINT, isSigner: false, isWritable: false },
      { pubkey: stakePoolPda, isSigner: false, isWritable: true },
      { pubkey: stakeVaultPda, isSigner: false, isWritable: true },
      { pubkey: TOKEN_2022_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });

  // Build transaction
  const transaction = new Transaction();
  transaction.add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 200_000 }),
    ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 50_000 }),
    ix
  );

  // Dry run check
  const dryRun = process.argv.includes("--dry-run");
  if (dryRun) {
    console.log("\n=== DRY RUN ===");
    console.log("Transaction would:");
    console.log("  1. Create stake_pool PDA");
    console.log("  2. Create stake_vault token account");
    console.log("  3. Set reward_rate = 0");
    console.log("\nTo execute, run without --dry-run flag");
    process.exit(0);
  }

  // Execute
  console.log("\n=== Executing ===");
  console.log("Sending transaction...");

  try {
    const sig = await sendAndConfirmTransaction(
      connection,
      transaction,
      [admin],
      { commitment: "confirmed" }
    );

    console.log("\n✅ Stake Pool Initialized!");
    console.log("Signature:", sig);
    console.log("\n=== Links ===");
    console.log(`Tx: https://solscan.io/tx/${sig}`);
    console.log(`Stake Pool: https://solscan.io/account/${stakePoolPda.toBase58()}`);
    console.log(`Stake Vault: https://solscan.io/account/${stakeVaultPda.toBase58()}`);

    console.log("\n=== Summary ===");
    console.log("Mint:", CCM_V3_MINT.toBase58());
    console.log("Stake Pool:", stakePoolPda.toBase58());
    console.log("Stake Vault:", stakeVaultPda.toBase58());
    console.log("Reward Rate: 0 (revenue-backed only)");

    console.log("\n=== Staking Ready ===");
    console.log("Users can now call:");
    console.log("  - stake(amount, lock_slots)");
    console.log("  - delegate_stake(subject_id)");
    console.log("  - unstake(amount)");
    console.log("  - claim_stake_rewards()");

  } catch (err: any) {
    console.error("\n❌ Error:", err.message);
    if (err.logs) {
      console.error("Logs:", err.logs.join("\n"));
    }
    process.exit(1);
  }
}

main().catch(console.error);
