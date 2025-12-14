#!/usr/bin/env ts-node

/**
 * Admin mint CCM-v2 tokens to treasury
 *
 * Usage: npx ts-node scripts/admin-mint-v2.ts <amount>
 * Example: npx ts-node scripts/admin-mint-v2.ts 1000000000000000000  # 1B CCM (9 decimals)
 *
 * Uses hardcoded ADMIN_AUTHORITY - must be signed by admin wallet.
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

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const CCM_V2_MINT = new PublicKey("Bwmh8UfYuUEh31gYuxgBRGct4jCut6TkpfnB6ba5MbF");
const PROTOCOL_SEED = Buffer.from("protocol");

// Admin authority (hardcoded in program)
const ADMIN_AUTHORITY = new PublicKey("AmMftc4zHgR4yYfv29awV9Q46emo2aGPFW8utP81CsBv");

async function main() {
  const args = process.argv.slice(2).filter(a => !a.startsWith("--"));
  if (args.length === 0) {
    console.log("Usage: npx ts-node scripts/admin-mint-v2.ts <amount> [--wallet <path>] [--dry-run]");
    console.log("Example: npx ts-node scripts/admin-mint-v2.ts 1000000000000000000  # 1B CCM");
    console.log("\nNote: Must be signed by ADMIN_AUTHORITY: AmMftc4zHgR4yYfv29awV9Q46emo2aGPFW8utP81CsBv");
    process.exit(1);
  }

  const amount = new BN(args[0]);
  console.log(`\n=== Admin Mint CCM-v2 ===`);
  console.log(`Amount: ${amount.toString()} (${Number(amount) / 1e9} CCM)`);

  // Load wallet (check for --wallet flag)
  const walletFlagIdx = process.argv.indexOf("--wallet");
  const walletPath = walletFlagIdx !== -1
    ? process.argv[walletFlagIdx + 1]
    : process.env.ANCHOR_WALLET || `${process.env.HOME}/.config/solana/id.json`;

  const wallet = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(walletPath, "utf-8")))
  );
  console.log(`Admin: ${wallet.publicKey.toBase58()}`);
  console.log(`Expected: ${ADMIN_AUTHORITY.toBase58()}`);

  // Warn if not admin authority (on-chain constraint will enforce)
  if (!wallet.publicKey.equals(ADMIN_AUTHORITY)) {
    console.warn(`\n⚠️  Warning: Wallet is not ADMIN_AUTHORITY - tx will fail on-chain`);
  }

  // Setup connection
  const rpcUrl = process.env.SYNDICA_RPC || "https://api.mainnet-beta.solana.com";
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
  const treasuryAta = getAssociatedTokenAddressSync(
    CCM_V2_MINT,
    wallet.publicKey,
    false,
    TOKEN_2022_PROGRAM_ID
  );

  const [mintAuthority] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, CCM_V2_MINT.toBuffer()],
    PROGRAM_ID
  );

  console.log(`\n=== Accounts ===`);
  console.log(`CCM-v2 Mint: ${CCM_V2_MINT.toBase58()}`);
  console.log(`Treasury ATA: ${treasuryAta.toBase58()}`);
  console.log(`Mint Authority PDA: ${mintAuthority.toBase58()}`);

  // Check/create treasury ATA
  const transaction = new Transaction();

  try {
    await getAccount(connection, treasuryAta, "confirmed", TOKEN_2022_PROGRAM_ID);
    console.log(`Treasury ATA exists`);
  } catch {
    console.log(`Creating treasury ATA...`);
    transaction.add(
      createAssociatedTokenAccountInstruction(
        wallet.publicKey,
        treasuryAta,
        wallet.publicKey,
        CCM_V2_MINT,
        TOKEN_2022_PROGRAM_ID
      )
    );
  }

  // Build admin_mint_v2 instruction
  const mintIx = await program.methods
    .adminMintV2(amount)
    .accounts({
      admin: wallet.publicKey,
      mint: CCM_V2_MINT,
      treasuryAta,
      mintAuthority,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
    })
    .instruction();

  transaction.add(mintIx);

  // Dry run check
  const dryRun = process.argv.includes("--dry-run");
  if (dryRun) {
    console.log("\nDRY RUN - not sending transaction");
    console.log("To execute for real, run without --dry-run flag");
    process.exit(0);
  }

  // Execute
  console.log(`\n=== Executing Admin Mint ===`);

  try {
    const sig = await sendAndConfirmTransaction(connection, transaction, [wallet], {
      commitment: "confirmed",
    });

    console.log(`\n✅ Admin mint successful!`);
    console.log(`Signature: ${sig}`);
    console.log(`https://solscan.io/tx/${sig}`);

    // Verify balance
    const account = await getAccount(connection, treasuryAta, "confirmed", TOKEN_2022_PROGRAM_ID);
    console.log(`\nTreasury Balance: ${account.amount.toString()} (${Number(account.amount) / 1e9} CCM-v2)`);

  } catch (err: any) {
    console.error(`\n❌ Error: ${err.message}`);
    if (err.logs) {
      console.error("Logs:", err.logs.join("\n"));
    }
    process.exit(1);
  }
}

main().catch(console.error);
