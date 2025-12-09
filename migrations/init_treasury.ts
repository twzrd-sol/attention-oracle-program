#!/usr/bin/env npx ts-node
/**
 * Initialize lofi-bank treasury and create treasury token account
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, Keypair, Connection, SystemProgram } from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Load IDL
const idlPath = path.join(__dirname, "../target/idl/lofi_bank.json");
const idl = JSON.parse(fs.readFileSync(idlPath, "utf8"));

const LOFI_BANK_PROGRAM_ID = new PublicKey("EHsyY7uroV6gRUt8gNB6eMXNtRdy5L9q6GA5um4teYTA");
const TWZRD_MINT = new PublicKey("ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe"); // Token-2022 TWZRD

async function main() {
  // Load wallet
  const walletPath = process.env.ANCHOR_WALLET || path.join(process.env.HOME!, ".config/solana/id.json");
  const walletKeypair = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(walletPath, "utf8")))
  );

  // Setup connection
  const rpcUrl = process.env.RPC_URL || "https://api.mainnet-beta.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");

  // Setup Anchor provider
  const wallet = new anchor.Wallet(walletKeypair);
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });
  anchor.setProvider(provider);

  // Initialize program
  const program = new Program(idl, provider);

  // Derive treasury PDA
  const [treasuryState] = PublicKey.findProgramAddressSync(
    [Buffer.from("treasury_state")],
    LOFI_BANK_PROGRAM_ID
  );

  console.log("Lofi Bank Program ID:", LOFI_BANK_PROGRAM_ID.toBase58());
  console.log("Treasury State PDA:", treasuryState.toBase58());
  console.log("TWZRD Mint:", TWZRD_MINT.toBase58());
  console.log("Payer:", walletKeypair.publicKey.toBase58());

  // Check if treasury already exists
  const treasuryInfo = await connection.getAccountInfo(treasuryState);
  if (treasuryInfo) {
    console.log("\nTreasury already initialized!");
    console.log("Account size:", treasuryInfo.data.length);
    console.log("Lamports:", treasuryInfo.lamports);
  } else {
    console.log("\nInitializing treasury with 5% yield (500 bps)...");

    const yieldBps = 500; // 5%

    const tx = await program.methods
      .initializeTreasury(yieldBps)
      .accounts({
        treasuryState: treasuryState,
        payer: walletKeypair.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([walletKeypair])
      .rpc();

    console.log("Treasury initialized!");
    console.log("Transaction:", tx);
  }

  // Get treasury token ATA
  const treasuryTokenAta = getAssociatedTokenAddressSync(
    TWZRD_MINT,
    treasuryState,
    true, // allowOwnerOffCurve for PDAs
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  console.log("\nTreasury Token ATA:", treasuryTokenAta.toBase58());

  // Check if ATA exists
  const ataInfo = await connection.getAccountInfo(treasuryTokenAta);
  if (ataInfo) {
    console.log("Treasury ATA already exists");
    // Get balance
    const balance = await connection.getTokenAccountBalance(treasuryTokenAta);
    console.log("Treasury balance:", balance.value.uiAmountString, "TWZRD");
  } else {
    console.log("\nCreating treasury ATA...");

    const createAtaIx = createAssociatedTokenAccountInstruction(
      walletKeypair.publicKey, // payer
      treasuryTokenAta,        // ata
      treasuryState,           // owner (PDA)
      TWZRD_MINT,              // mint
      TOKEN_2022_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID
    );

    const tx = new anchor.web3.Transaction().add(createAtaIx);
    const sig = await provider.sendAndConfirm(tx, [walletKeypair]);
    console.log("Treasury ATA created!");
    console.log("Transaction:", sig);
  }

  console.log("\n=== Summary ===");
  console.log("Treasury State:", treasuryState.toBase58());
  console.log("Treasury Token ATA:", treasuryTokenAta.toBase58());
  console.log("\nTo seed treasury, run:");
  console.log(`spl-token transfer ${TWZRD_MINT.toBase58()} <amount> ${treasuryTokenAta.toBase58()} --program-id ${TOKEN_2022_PROGRAM_ID.toBase58()}`);
}

main().catch((err) => {
  console.error("Error:", err);
  process.exit(1);
});
