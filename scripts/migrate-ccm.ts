#!/usr/bin/env ts-node

/**
 * Migrate CCM-v1 to CCM-v2 (1:1)
 *
 * Usage: npx ts-node scripts/migrate-ccm.ts <amount>
 * Example: npx ts-node scripts/migrate-ccm.ts 1000000000  # 1 CCM (9 decimals)
 *
 * Burns CCM-v1 from user, mints CCM-v2 to user.
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
  getAccount,
} from "@solana/spl-token";
import BN from "bn.js";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import { requireScriptEnv } from "./script-guard.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const OLD_MINT = new PublicKey("ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe");
const NEW_MINT = new PublicKey("Bwmh8UfYuUEh31gYuxgBRGct4jCut6TkpfnB6ba5MbF");
const PROTOCOL_SEED = Buffer.from("protocol");

async function main() {
  const args = process.argv.slice(2);
  if (args.length === 0) {
    console.log("Usage: npx ts-node scripts/migrate-ccm.ts <amount>");
    console.log("Example: npx ts-node scripts/migrate-ccm.ts 1000000000  # 1 CCM");
    process.exit(1);
  }

  const amount = new BN(args[0]);
  console.log(`\n=== CCM Migration ===`);
  console.log(`Amount: ${amount.toString()} (${Number(amount) / 1e9} CCM)`);

  const { rpcUrl, keypairPath } = requireScriptEnv();

  // Load wallet
  const walletPath = keypairPath;
  const wallet = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(walletPath, "utf-8")))
  );
  console.log(`User: ${wallet.publicKey.toBase58()}`);

  // Setup connection
  const connection = new Connection(rpcUrl, "confirmed");

  // Setup provider
  const anchorWallet = new anchor.Wallet(wallet);
  const provider = new anchor.AnchorProvider(connection, anchorWallet, {
    commitment: "confirmed",
  });
  anchor.setProvider(provider);

  // Load IDL
  const idl = JSON.parse(
    fs.readFileSync(path.join(__dirname, "../target/idl/token_2022.json"), "utf-8")
  );

  // Patch account sizes if missing
  if (idl.accounts) {
    idl.accounts.forEach((acc: any) => {
      if (acc.size === null || acc.size === undefined) {
        acc.size = 8 + 1000;
      }
    });
  }

  const program = new Program(idl, provider) as any;

  // Derive addresses
  const userOldAta = getAssociatedTokenAddressSync(
    OLD_MINT,
    wallet.publicKey,
    false,
    TOKEN_2022_PROGRAM_ID
  );

  const userNewAta = getAssociatedTokenAddressSync(
    NEW_MINT,
    wallet.publicKey,
    false,
    TOKEN_2022_PROGRAM_ID
  );

  const [mintAuthority, mintAuthorityBump] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, NEW_MINT.toBuffer()],
    PROGRAM_ID
  );

  console.log(`\n=== Accounts ===`);
  console.log(`Old Mint: ${OLD_MINT.toBase58()}`);
  console.log(`New Mint: ${NEW_MINT.toBase58()}`);
  console.log(`User Old ATA: ${userOldAta.toBase58()}`);
  console.log(`User New ATA: ${userNewAta.toBase58()}`);
  console.log(`Mint Authority PDA: ${mintAuthority.toBase58()}`);

  // Check old ATA balance
  try {
    const oldAccount = await getAccount(connection, userOldAta, "confirmed", TOKEN_2022_PROGRAM_ID);
    console.log(`\nOld ATA Balance: ${oldAccount.amount.toString()} (${Number(oldAccount.amount) / 1e9} CCM)`);

    if (BigInt(oldAccount.amount.toString()) < BigInt(amount.toString())) {
      console.error(`\nError: Insufficient balance. Have ${oldAccount.amount}, need ${amount}`);
      process.exit(1);
    }
  } catch (err) {
    console.error(`\nError: Old ATA not found or empty. Cannot migrate.`);
    process.exit(1);
  }

  // Check if new ATA exists, create if not
  const transaction = new Transaction();

  try {
    await getAccount(connection, userNewAta, "confirmed", TOKEN_2022_PROGRAM_ID);
    console.log(`New ATA exists`);
  } catch {
    console.log(`Creating new ATA...`);
    transaction.add(
      createAssociatedTokenAccountInstruction(
        wallet.publicKey,
        userNewAta,
        wallet.publicKey,
        NEW_MINT,
        TOKEN_2022_PROGRAM_ID
      )
    );
  }

  // Build migrate instruction
  const migrateIx = await program.methods
    .migrate(amount)
    .accounts({
      user: wallet.publicKey,
      oldMint: OLD_MINT,
      newMint: NEW_MINT,
      userOldAta,
      userNewAta,
      mintAuthority,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
    })
    .instruction();

  transaction.add(migrateIx);

  // Dry run check
  const dryRun = process.argv.includes("--dry-run");
  if (dryRun) {
    console.log("\nDRY RUN - not sending transaction");
    console.log("To execute for real, run without --dry-run flag");
    process.exit(0);
  }

  // Execute
  console.log(`\n=== Executing Migration ===`);

  try {
    const sig = await sendAndConfirmTransaction(connection, transaction, [wallet], {
      commitment: "confirmed",
    });

    console.log(`\n✅ Migration successful!`);
    console.log(`Signature: ${sig}`);
    console.log(`https://solscan.io/tx/${sig}`);

    // Verify new balance
    const newAccount = await getAccount(connection, userNewAta, "confirmed", TOKEN_2022_PROGRAM_ID);
    console.log(`\nNew ATA Balance: ${newAccount.amount.toString()} (${Number(newAccount.amount) / 1e9} CCM-v2)`);

  } catch (err: any) {
    console.error(`\n❌ Error: ${err.message}`);
    if (err.logs) {
      console.error("Logs:", err.logs.join("\n"));
    }
    process.exit(1);
  }
}

main().catch(console.error);
