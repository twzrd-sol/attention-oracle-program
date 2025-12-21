#!/usr/bin/env ts-node

/**
 * Create CCM-v2 mint with TransferFeeConfig extension
 *
 * Features:
 * - 50 bps (0.5%) transfer fee
 * - Max fee: 5000 CCM (allows high-value transfers)
 * - withdraw_withheld_authority: protocol_state PDA (immutable harvest)
 * - mint_authority: protocol_state PDA (for migration minting)
 * - fee_authority: admin (can adjust rate via governance)
 */

import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  ExtensionType,
  createInitializeMintInstruction,
  createInitializeTransferFeeConfigInstruction,
  getMintLen,
} from "@solana/spl-token";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import { requireScriptEnv } from "./script-guard.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const PROTOCOL_SEED = Buffer.from("protocol");

// Transfer fee config
const TRANSFER_FEE_BASIS_POINTS = 50; // 0.5%
const MAX_FEE = BigInt(5_000_000_000_000); // 5000 CCM (9 decimals)
const DECIMALS = 9;

async function main() {
  console.log("=== CCM-v2 Mint Creation ===\n");

  const { rpcUrl, keypairPath } = requireScriptEnv();

  // Load wallet
  const walletPath = keypairPath;
  const wallet = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(walletPath, "utf-8")))
  );
  console.log("Wallet:", wallet.publicKey.toBase58());

  // Setup connection
  const connection = new Connection(rpcUrl, "confirmed");

  // Check for existing mint keypair or generate new
  const mintKeypairPath = path.join(__dirname, "../keys/ccm-v2-mint.json");
  let mintKeypair: Keypair;

  if (fs.existsSync(mintKeypairPath)) {
    mintKeypair = Keypair.fromSecretKey(
      new Uint8Array(JSON.parse(fs.readFileSync(mintKeypairPath, "utf-8")))
    );
    console.log("Loaded existing mint keypair:", mintKeypair.publicKey.toBase58());
  } else {
    mintKeypair = Keypair.generate();
    // Save for reproducibility
    fs.mkdirSync(path.dirname(mintKeypairPath), { recursive: true });
    fs.writeFileSync(mintKeypairPath, JSON.stringify(Array.from(mintKeypair.secretKey)));
    console.log("Generated new mint keypair:", mintKeypair.publicKey.toBase58());
    console.log("Saved to:", mintKeypairPath);
  }

  const mint = mintKeypair.publicKey;

  // Derive protocol_state PDA for new mint
  const [protocolState, protocolBump] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, mint.toBuffer()],
    PROGRAM_ID
  );

  console.log("\n=== Derived Addresses ===");
  console.log("CCM-v2 Mint:", mint.toBase58());
  console.log("Protocol State PDA:", protocolState.toBase58());
  console.log("Protocol State Bump:", protocolBump);

  // Authorities
  const feeAuthority = wallet.publicKey; // Admin can adjust fee rate
  const withdrawWithheldAuthority = protocolState; // PDA controls harvest
  const mintAuthority = protocolState; // PDA controls minting (for migration)
  const freezeAuthority = null; // No freeze

  console.log("\n=== Authorities ===");
  console.log("Fee Authority:", feeAuthority.toBase58(), "(admin - can adjust rate)");
  console.log("Withdraw Withheld Authority:", withdrawWithheldAuthority.toBase58(), "(PDA - immutable)");
  console.log("Mint Authority:", mintAuthority.toBase58(), "(PDA - for migration)");
  console.log("Freeze Authority: None");

  console.log("\n=== Transfer Fee Config ===");
  console.log("Fee:", TRANSFER_FEE_BASIS_POINTS, "bps (0.5%)");
  console.log("Max Fee:", MAX_FEE.toString(), "lamports (5000 CCM)");

  // Check if mint already exists
  const mintInfo = await connection.getAccountInfo(mint);
  if (mintInfo) {
    console.log("\n⚠️  Mint already exists!");
    console.log("Account size:", mintInfo.data.length);
    process.exit(0);
  }

  // Calculate space needed
  const extensions = [ExtensionType.TransferFeeConfig];
  const mintLen = getMintLen(extensions);
  const lamports = await connection.getMinimumBalanceForRentExemption(mintLen);

  console.log("\n=== Account Setup ===");
  console.log("Mint length:", mintLen, "bytes");
  console.log("Rent:", lamports / 1e9, "SOL");

  // Build transaction
  const transaction = new Transaction().add(
    // Create account
    SystemProgram.createAccount({
      fromPubkey: wallet.publicKey,
      newAccountPubkey: mint,
      space: mintLen,
      lamports,
      programId: TOKEN_2022_PROGRAM_ID,
    }),
    // Initialize transfer fee config (must be before mint init)
    createInitializeTransferFeeConfigInstruction(
      mint,
      feeAuthority,
      withdrawWithheldAuthority,
      TRANSFER_FEE_BASIS_POINTS,
      MAX_FEE,
      TOKEN_2022_PROGRAM_ID
    ),
    // Initialize mint
    createInitializeMintInstruction(
      mint,
      DECIMALS,
      mintAuthority,
      freezeAuthority,
      TOKEN_2022_PROGRAM_ID
    )
  );

  console.log("\n=== Executing ===");

  // Dry run check
  const dryRun = process.argv.includes("--dry-run");
  if (dryRun) {
    console.log("DRY RUN - not sending transaction");
    console.log("\nTo execute for real, run without --dry-run flag");
    process.exit(0);
  }

  try {
    const sig = await sendAndConfirmTransaction(
      connection,
      transaction,
      [wallet, mintKeypair],
      { commitment: "confirmed" }
    );

    console.log("\n✅ CCM-v2 Mint Created!");
    console.log("Signature:", sig);
    console.log("Mint:", mint.toBase58());
    console.log(`https://solscan.io/token/${mint.toBase58()}`);
    console.log(`https://solscan.io/tx/${sig}`);

    // Output for next steps
    console.log("\n=== Next Steps ===");
    console.log("1. Initialize protocol state for new mint:");
    console.log(`   CCM_V2_MINT=${mint.toBase58()}`);
    console.log("2. Create migration instruction (burn v1 → mint v2)");
    console.log("3. Seed Raydium CLMM + Meteora DLMM pools");

  } catch (err: any) {
    console.error("\n❌ Error:", err.message);
    if (err.logs) {
      console.error("Logs:", err.logs.join("\n"));
    }
    process.exit(1);
  }
}

main().catch(console.error);
