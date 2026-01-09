#!/usr/bin/env ts-node

/**
 * Admin withdraw CCM from treasury for LP seeding
 *
 * Usage:
 *   npx ts-node scripts/admin-withdraw-v3.ts <amount_ccm> [--dry-run]
 *
 * Example:
 *   npx ts-node scripts/admin-withdraw-v3.ts 100000000  # Withdraw 100M CCM
 */

import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  sendAndConfirmTransaction,
  ComputeBudgetProgram,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
} from "@solana/spl-token";
import fs from "fs";

import {
  PROGRAM_ID,
  CCM_V3_MINT,
  DECIMALS,
  PROTOCOL_SEED,
  getRpcUrl,
  getWalletPath,
} from "./config.js";

// admin_withdraw discriminator from IDL
const ADMIN_WITHDRAW_DISCRIMINATOR = Buffer.from([
  160, 166, 147, 222, 46, 220, 75, 224
]);

async function main() {
  const args = process.argv.slice(2).filter(a => !a.startsWith('--'));
  const dryRun = process.argv.includes("--dry-run");

  if (args.length < 1) {
    console.log("Usage: npx ts-node scripts/admin-withdraw-v3.ts <amount_ccm> [--dry-run]");
    console.log("Example: npx ts-node scripts/admin-withdraw-v3.ts 100000000  # 100M CCM");
    process.exit(1);
  }

  const amountCcm = BigInt(args[0]);
  const amountRaw = amountCcm * BigInt(10 ** DECIMALS);

  console.log("=== CCM-v3 Admin Withdraw ===\n");

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

  // Treasury ATA (owned by protocol_state PDA)
  const treasuryAta = getAssociatedTokenAddressSync(
    CCM_V3_MINT,
    protocolStatePda,
    true,
    TOKEN_2022_PROGRAM_ID
  );

  // Recipient ATA (owned by admin wallet)
  const recipientAta = getAssociatedTokenAddressSync(
    CCM_V3_MINT,
    admin.publicKey,
    false,
    TOKEN_2022_PROGRAM_ID
  );

  console.log("\n=== Accounts ===");
  console.log("Protocol State:", protocolStatePda.toBase58());
  console.log("Treasury ATA:", treasuryAta.toBase58());
  console.log("Recipient ATA:", recipientAta.toBase58());

  console.log("\n=== Withdraw ===");
  console.log("Amount:", amountCcm.toLocaleString(), "CCM");
  console.log("Raw:", amountRaw.toString());

  // Check treasury balance
  const treasuryInfo = await connection.getTokenAccountBalance(treasuryAta);
  console.log("Treasury Balance:", (Number(treasuryInfo.value.amount) / 1e9).toLocaleString(), "CCM");

  if (BigInt(treasuryInfo.value.amount) < amountRaw) {
    console.error("\n❌ Error: Insufficient treasury balance");
    process.exit(1);
  }

  // Build transaction
  const transaction = new Transaction();

  transaction.add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 200_000 }),
    ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 50_000 })
  );

  // Check if recipient ATA exists, create if needed
  const recipientInfo = await connection.getAccountInfo(recipientAta);
  if (!recipientInfo) {
    console.log("\nCreating recipient ATA...");
    transaction.add(
      createAssociatedTokenAccountInstruction(
        admin.publicKey,
        recipientAta,
        admin.publicKey,
        CCM_V3_MINT,
        TOKEN_2022_PROGRAM_ID
      )
    );
  }

  // Build instruction data: discriminator (8) + amount (8, little-endian u64)
  const data = Buffer.alloc(8 + 8);
  ADMIN_WITHDRAW_DISCRIMINATOR.copy(data, 0);
  data.writeBigUInt64LE(amountRaw, 8);

  // Build admin_withdraw instruction
  const ix = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: admin.publicKey, isSigner: true, isWritable: true },
      { pubkey: protocolStatePda, isSigner: false, isWritable: false },
      { pubkey: CCM_V3_MINT, isSigner: false, isWritable: false },
      { pubkey: treasuryAta, isSigner: false, isWritable: true },
      { pubkey: recipientAta, isSigner: false, isWritable: true },
      { pubkey: TOKEN_2022_PROGRAM_ID, isSigner: false, isWritable: false },
    ],
    data,
  });

  transaction.add(ix);

  if (dryRun) {
    console.log("\n=== DRY RUN ===");
    console.log("Transaction would:");
    console.log(`  1. Withdraw ${amountCcm.toLocaleString()} CCM from treasury`);
    console.log(`  2. Transfer to ${recipientAta.toBase58()}`);
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

    console.log("\n✅ Withdraw Complete!");
    console.log("Signature:", sig);
    console.log(`Tx: https://solscan.io/tx/${sig}`);

    // Verify balances
    const newTreasuryBalance = await connection.getTokenAccountBalance(treasuryAta);
    const recipientBalance = await connection.getTokenAccountBalance(recipientAta);

    console.log("\n=== Final Balances ===");
    console.log("Treasury:", (Number(newTreasuryBalance.value.amount) / 1e9).toLocaleString(), "CCM");
    console.log("Recipient:", (Number(recipientBalance.value.amount) / 1e9).toLocaleString(), "CCM");

  } catch (err: any) {
    console.error("\n❌ Error:", err.message);
    if (err.logs) {
      console.error("Logs:", err.logs.join("\n"));
    }
    process.exit(1);
  }
}

main().catch(console.error);
