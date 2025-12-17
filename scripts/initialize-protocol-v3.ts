#!/usr/bin/env ts-node

/**
 * Initialize protocol_state PDA for CCM-v3 mint
 *
 * This creates the protocol_state and fee_config PDAs which enable:
 * - Claims from merkle roots (claim_channel_open)
 * - Fee harvesting (harvest_fees)
 * - Staking (initialize_stake_pool)
 *
 * ADMIN_AUTHORITY required: 2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD
 *
 * Usage:
 *   ANCHOR_WALLET=~/.config/solana/id.json npx ts-node scripts/initialize-protocol-v3.ts [--dry-run]
 *
 * Env:
 *   ANCHOR_WALLET - path to admin keypair (must be ADMIN_AUTHORITY)
 *   SYNDICA_RPC - RPC endpoint
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
import fs from "fs";

import {
  PROGRAM_ID,
  CCM_V3_MINT,
  ADMIN_AUTHORITY,
  PROTOCOL_SEED,
  FEE_CONFIG_SEED,
  getRpcUrl,
  getWalletPath,
  printConfig,
} from "./config.js";

// Fee config params
const FEE_BASIS_POINTS = 300; // 3% claim skim (matches CLAIM_SKIM_BPS)
const MAX_FEE = BigInt(5_000) * BigInt(10 ** 9); // 5000 CCM max

// initialize_mint discriminator from IDL
const INITIALIZE_MINT_DISCRIMINATOR = Buffer.from([209, 42, 195, 4, 129, 85, 209, 44]);

async function main() {
  console.log("=== CCM-v3 Protocol State Initialization ===\n");

  // Print config for debugging
  printConfig();

  // Load admin wallet
  const admin = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(getWalletPath(), "utf-8")))
  );
  console.log("\nAdmin Wallet:", admin.publicKey.toBase58());

  // Verify admin is ADMIN_AUTHORITY
  if (!admin.publicKey.equals(ADMIN_AUTHORITY)) {
    console.error("\n❌ Error: Wallet is not ADMIN_AUTHORITY");
    console.error("Expected:", ADMIN_AUTHORITY.toBase58());
    console.error("Got:", admin.publicKey.toBase58());
    process.exit(1);
  }
  console.log("✅ Admin is ADMIN_AUTHORITY");

  // Setup connection
  const rpcUrl = getRpcUrl();
  const connection = new Connection(rpcUrl, "confirmed");

  // Derive PDAs
  const [protocolStatePda, protocolBump] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, CCM_V3_MINT.toBuffer()],
    PROGRAM_ID
  );

  const [feeConfigPda, feeConfigBump] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, CCM_V3_MINT.toBuffer(), FEE_CONFIG_SEED],
    PROGRAM_ID
  );

  console.log("\n=== Derived PDAs ===");
  console.log("Protocol State PDA:", protocolStatePda.toBase58());
  console.log("Protocol Bump:", protocolBump);
  console.log("Fee Config PDA:", feeConfigPda.toBase58());
  console.log("Fee Config Bump:", feeConfigBump);

  console.log("\n=== Fee Config ===");
  console.log("Basis Points:", FEE_BASIS_POINTS, "(3%)");
  console.log("Max Fee:", (Number(MAX_FEE) / 1e9).toLocaleString(), "CCM");

  // Check if protocol_state already exists
  const protocolInfo = await connection.getAccountInfo(protocolStatePda);
  if (protocolInfo) {
    console.log("\n⚠️  Protocol State already initialized!");
    console.log("Account exists at:", protocolStatePda.toBase58());
    console.log("Size:", protocolInfo.data.length, "bytes");
    process.exit(0);
  }

  // Build instruction data
  // Format: discriminator (8) + fee_basis_points (2, little-endian) + max_fee (8, little-endian)
  const data = Buffer.alloc(8 + 2 + 8);
  INITIALIZE_MINT_DISCRIMINATOR.copy(data, 0);
  data.writeUInt16LE(FEE_BASIS_POINTS, 8);
  data.writeBigUInt64LE(MAX_FEE, 10);

  // Build instruction
  const ix = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: admin.publicKey, isSigner: true, isWritable: true },
      { pubkey: CCM_V3_MINT, isSigner: false, isWritable: false },
      { pubkey: protocolStatePda, isSigner: false, isWritable: true },
      { pubkey: feeConfigPda, isSigner: false, isWritable: true },
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
    console.log("  1. Create protocol_state PDA");
    console.log("  2. Create fee_config PDA");
    console.log("  3. Set fee config (300 bps, 5000 CCM max)");
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

    console.log("\n✅ Protocol State Initialized!");
    console.log("Signature:", sig);
    console.log("\n=== Links ===");
    console.log(`Tx: https://solscan.io/tx/${sig}`);
    console.log(`Protocol State: https://solscan.io/account/${protocolStatePda.toBase58()}`);

    console.log("\n=== Summary ===");
    console.log("Mint:", CCM_V3_MINT.toBase58());
    console.log("Protocol State:", protocolStatePda.toBase58());
    console.log("Fee Config:", feeConfigPda.toBase58());

    console.log("\n=== Next Steps ===");
    console.log("1. Initialize stake pool: initialize_stake_pool");
    console.log("2. Create channels: create_channel");
    console.log("3. Deploy Meteora DAMM v2 pool (CCM-USDC)");

  } catch (err: any) {
    console.error("\n❌ Error:", err.message);
    if (err.logs) {
      console.error("Logs:", err.logs.join("\n"));
    }
    process.exit(1);
  }
}

main().catch(console.error);
