#!/usr/bin/env ts-node

/**
 * Create CCM-v3 mint with 2B supply and TransferFeeConfig
 *
 * Features:
 * - 2B total supply (2,000,000,000 CCM with 9 decimals)
 * - 50 bps (0.5%) transfer fee
 * - Max fee: 5000 CCM per transfer
 * - withdraw_withheld_authority: protocol_state PDA (immutable harvest)
 * - Mint 2B directly to treasury ATA (owned by protocol_state PDA)
 * - Close mint authority after minting (no future mints possible)
 *
 * Usage:
 *   npx ts-node scripts/create-ccm-v3-mint.ts [--dry-run]
 *
 * Env:
 *   ANCHOR_WALLET - path to admin keypair
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
} from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  ExtensionType,
  createInitializeMintInstruction,
  createInitializeTransferFeeConfigInstruction,
  createMintToInstruction,
  createSetAuthorityInstruction,
  AuthorityType,
  getMintLen,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
} from "@solana/spl-token";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Program constants
const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const PROTOCOL_SEED = Buffer.from("protocol");

// CCM-v3 Token Config
const DECIMALS = 9;
const TOTAL_SUPPLY = BigInt(2_000_000_000) * BigInt(10 ** DECIMALS); // 2B with 9 decimals
const TRANSFER_FEE_BASIS_POINTS = 50; // 0.5%
const MAX_FEE = BigInt(5_000) * BigInt(10 ** DECIMALS); // 5000 CCM max fee

async function main() {
  console.log("=== CCM-v3 Mint Creation (2B Supply) ===\n");

  // Load admin wallet
  const walletPath = process.env.ANCHOR_WALLET || `${process.env.HOME}/.config/solana/id.json`;
  const admin = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(walletPath, "utf-8")))
  );
  console.log("Admin Wallet:", admin.publicKey.toBase58());

  // Setup connection
  const rpcUrl = process.env.SYNDICA_RPC || "https://api.mainnet-beta.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");
  console.log("RPC:", rpcUrl.substring(0, 50) + "...");

  // Generate or load mint keypair
  const mintKeypairPath = path.join(__dirname, "../.keys/ccm-v3-mint.json");
  let mintKeypair: Keypair;

  if (fs.existsSync(mintKeypairPath)) {
    mintKeypair = Keypair.fromSecretKey(
      new Uint8Array(JSON.parse(fs.readFileSync(mintKeypairPath, "utf-8")))
    );
    console.log("Loaded existing mint keypair:", mintKeypair.publicKey.toBase58());
  } else {
    mintKeypair = Keypair.generate();
    fs.mkdirSync(path.dirname(mintKeypairPath), { recursive: true });
    fs.writeFileSync(mintKeypairPath, JSON.stringify(Array.from(mintKeypair.secretKey)));
    console.log("Generated new mint keypair:", mintKeypair.publicKey.toBase58());
    console.log("Saved to:", mintKeypairPath);
  }

  const mint = mintKeypair.publicKey;

  // Derive protocol_state PDA (will be created later via initialize_mint)
  const [protocolStatePda, protocolBump] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, mint.toBuffer()],
    PROGRAM_ID
  );

  // Derive treasury ATA (owned by protocol_state PDA)
  const treasuryAta = getAssociatedTokenAddressSync(
    mint,
    protocolStatePda,
    true, // allowOwnerOffCurve = true for PDA
    TOKEN_2022_PROGRAM_ID
  );

  console.log("\n=== Derived Addresses ===");
  console.log("CCM-v3 Mint:", mint.toBase58());
  console.log("Protocol State PDA:", protocolStatePda.toBase58());
  console.log("Protocol State Bump:", protocolBump);
  console.log("Treasury ATA:", treasuryAta.toBase58());

  console.log("\n=== Token Config ===");
  console.log("Total Supply:", (Number(TOTAL_SUPPLY) / 1e9).toLocaleString(), "CCM");
  console.log("Decimals:", DECIMALS);
  console.log("Transfer Fee:", TRANSFER_FEE_BASIS_POINTS, "bps (0.5%)");
  console.log("Max Fee:", (Number(MAX_FEE) / 1e9).toLocaleString(), "CCM");

  console.log("\n=== Authorities ===");
  console.log("Mint Authority (temp):", admin.publicKey.toBase58(), "→ will be CLOSED");
  console.log("Freeze Authority:", "None");
  console.log("Fee Config Authority:", admin.publicKey.toBase58());
  console.log("Withdraw Withheld Authority:", protocolStatePda.toBase58(), "(PDA - immutable)");

  // Check if mint already exists
  const mintInfo = await connection.getAccountInfo(mint);
  if (mintInfo) {
    console.log("\n⚠️  Mint already exists!");
    console.log("To create a new mint, delete", mintKeypairPath);
    process.exit(0);
  }

  // Calculate space and rent
  const extensions = [ExtensionType.TransferFeeConfig];
  const mintLen = getMintLen(extensions);
  const lamports = await connection.getMinimumBalanceForRentExemption(mintLen);

  console.log("\n=== Account Setup ===");
  console.log("Mint Account Size:", mintLen, "bytes");
  console.log("Rent:", (lamports / 1e9).toFixed(6), "SOL");

  // Build transaction
  const transaction = new Transaction();

  // 0. Set compute budget (minting 2B in one tx needs headroom)
  transaction.add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 }),
    ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 50_000 })
  );

  // 1. Create mint account
  transaction.add(
    SystemProgram.createAccount({
      fromPubkey: admin.publicKey,
      newAccountPubkey: mint,
      space: mintLen,
      lamports,
      programId: TOKEN_2022_PROGRAM_ID,
    })
  );

  // 2. Initialize TransferFeeConfig (MUST be before mint init)
  transaction.add(
    createInitializeTransferFeeConfigInstruction(
      mint,
      admin.publicKey,        // transferFeeConfigAuthority (can adjust rate)
      protocolStatePda,       // withdrawWithheldAuthority (PDA - immutable)
      TRANSFER_FEE_BASIS_POINTS,
      MAX_FEE,
      TOKEN_2022_PROGRAM_ID
    )
  );

  // 3. Initialize mint (admin is temp mint authority)
  transaction.add(
    createInitializeMintInstruction(
      mint,
      DECIMALS,
      admin.publicKey,  // mintAuthority (temporary)
      null,             // freezeAuthority (none)
      TOKEN_2022_PROGRAM_ID
    )
  );

  // 4. Create treasury ATA (owned by protocol_state PDA)
  transaction.add(
    createAssociatedTokenAccountInstruction(
      admin.publicKey,    // payer
      treasuryAta,        // ata
      protocolStatePda,   // owner (PDA)
      mint,
      TOKEN_2022_PROGRAM_ID
    )
  );

  // 5. Mint 2B to treasury
  transaction.add(
    createMintToInstruction(
      mint,
      treasuryAta,
      admin.publicKey,  // mintAuthority
      TOTAL_SUPPLY,
      [],
      TOKEN_2022_PROGRAM_ID
    )
  );

  // 6. Close mint authority (no more minting ever)
  transaction.add(
    createSetAuthorityInstruction(
      mint,
      admin.publicKey,        // current authority
      AuthorityType.MintTokens,
      null,                   // new authority = null (closed)
      [],
      TOKEN_2022_PROGRAM_ID
    )
  );

  // Dry run check
  const dryRun = process.argv.includes("--dry-run");
  if (dryRun) {
    console.log("\n=== DRY RUN ===");
    console.log("Transaction would:");
    console.log("  1. Create mint account");
    console.log("  2. Initialize TransferFeeConfig (50 bps)");
    console.log("  3. Initialize mint (9 decimals)");
    console.log("  4. Create treasury ATA for protocol_state PDA");
    console.log("  5. Mint 2,000,000,000 CCM to treasury");
    console.log("  6. Close mint authority (PERMANENT)");
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
      [admin, mintKeypair],
      { commitment: "confirmed" }
    );

    console.log("\n✅ CCM-v3 Mint Created!");
    console.log("Signature:", sig);
    console.log("\n=== Links ===");
    console.log(`Token: https://solscan.io/token/${mint.toBase58()}`);
    console.log(`Tx: https://solscan.io/tx/${sig}`);

    console.log("\n=== Summary ===");
    console.log("Mint:", mint.toBase58());
    console.log("Treasury ATA:", treasuryAta.toBase58());
    console.log("Treasury Balance: 2,000,000,000 CCM");
    console.log("Mint Authority: CLOSED (no future mints)");

    console.log("\n=== Next Steps ===");
    console.log("1. Initialize protocol_state for v3 mint:");
    console.log(`   CCM_V3_MINT=${mint.toBase58()}`);
    console.log("2. Deploy Meteora DAMM v2 pool (CCM-USDC)");
    console.log("3. Seed initial LP");

    // Save mint address to env file for easy reference
    const envPath = path.join(__dirname, "../.env.ccm-v3");
    fs.writeFileSync(envPath, `CCM_V3_MINT=${mint.toBase58()}\nTREASURY_ATA=${treasuryAta.toBase58()}\nPROTOCOL_STATE_PDA=${protocolStatePda.toBase58()}\n`);
    console.log(`\nSaved addresses to ${envPath}`);

  } catch (err: any) {
    console.error("\n❌ Error:", err.message);
    if (err.logs) {
      console.error("Logs:", err.logs.join("\n"));
    }
    process.exit(1);
  }
}

main().catch(console.error);
