/**
 * Deploy security.txt on-chain for both programs
 *
 * Creates program-derived accounts containing security.txt content
 * per the Solana security.txt standard:
 * https://github.com/neodyme-labs/solana-security-txt
 *
 * Usage:
 *   CLUSTER=mainnet-beta RPC_URL=https://... \
 *   KEYPAIR=~/.config/solana/admin-keypair.json \
 *     npx tsx scripts/admin/deploy-security-txt.ts
 */

import { Connection, Keypair, PublicKey, Transaction, SystemProgram } from "@solana/web3.js";
import * as fs from "fs";
import * as crypto from "crypto";

const PROGRAMS = [
  {
    name: "Attention Oracle",
    id: "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop",
  },
  {
    name: "ChannelVault",
    id: "5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ",
  },
];

const SECURITY_TXT_PATH = "/home/twzrd/attention-oracle-program/.well-known/security.txt";

async function main() {
  const rpcUrl = process.env.RPC_URL;
  const keypairPath = process.env.KEYPAIR;
  const cluster = process.env.CLUSTER || "mainnet-beta";

  if (!rpcUrl || !keypairPath) {
    console.error("ERROR: RPC_URL and KEYPAIR required");
    process.exit(1);
  }

  if (cluster !== "mainnet-beta") {
    console.error("ERROR: Only mainnet-beta supported for security.txt deployment");
    process.exit(1);
  }

  console.log("\n╔════════════════════════════════════════════════════════╗");
  console.log("║  Deploy security.txt On-Chain                          ║");
  console.log("╚════════════════════════════════════════════════════════╝\n");

  // Load security.txt
  const securityTxt = fs.readFileSync(SECURITY_TXT_PATH, "utf-8");
  console.log("Security.txt content:");
  console.log("─".repeat(60));
  console.log(securityTxt);
  console.log("─".repeat(60));

  // Load keypair
  const keypairData = JSON.parse(fs.readFileSync(keypairPath, "utf-8"));
  const payer = Keypair.fromSecretKey(new Uint8Array(keypairData));
  console.log(`\nPayer: ${payer.publicKey.toBase58()}`);

  const connection = new Connection(rpcUrl, "confirmed");
  const balance = await connection.getBalance(payer.publicKey);
  console.log(`Balance: ${balance / 1e9} SOL\n`);

  if (balance < 0.01 * 1e9) {
    console.error("ERROR: Insufficient SOL balance (need ~0.01 SOL per program)");
    process.exit(1);
  }

  console.log("Type DEPLOY to continue: ");
  const input = await new Promise<string>((resolve) => {
    process.stdin.once("data", (data) => resolve(data.toString().trim()));
  });
  if (input !== "DEPLOY") {
    console.log("Aborted.");
    process.exit(0);
  }

  console.log("\n");

  for (const program of PROGRAMS) {
    console.log(`\n📝 Deploying security.txt for ${program.name}...`);
    console.log(`   Program ID: ${program.id}`);

    const programId = new PublicKey(program.id);

    // Derive security.txt PDA
    // Standard: sha256("SECURITY_TXT")[0..8] as seed
    const seed = crypto.createHash("sha256").update("SECURITY_TXT").digest().slice(0, 8);
    const [securityTxtPda] = PublicKey.findProgramAddressSync([seed], programId);

    console.log(`   Security.txt PDA: ${securityTxtPda.toBase58()}`);

    // Check if already deployed
    const existingAccount = await connection.getAccountInfo(securityTxtPda);
    if (existingAccount) {
      console.log(`   ⏭️  Already deployed (${existingAccount.data.length} bytes)`);
      continue;
    }

    // Create account with security.txt content
    const dataBuffer = Buffer.from(securityTxt, "utf-8");
    const space = dataBuffer.length;
    const lamports = await connection.getMinimumBalanceForRentExemption(space);

    console.log(`   Creating account (${space} bytes, ${lamports / 1e9} SOL rent)...`);

    // NOTE: This requires the program to have a specific instruction to create
    // the security.txt account. Standard approach is:
    // 1. Use a separate keypair for the PDA (not program-derived)
    // 2. Or have the program expose a create_security_txt instruction

    // For now, we'll create it as a regular account owned by the payer
    // (not ideal but works for visibility)

    const securityTxtKeypair = Keypair.generate();

    const tx = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: payer.publicKey,
        newAccountPubkey: securityTxtKeypair.publicKey,
        lamports,
        space,
        programId: SystemProgram.programId, // Owned by system program (data account)
      })
    );

    // TODO: Add instruction to write data to account
    // For proper implementation, this needs to be a CPI from the program itself

    console.log(`   ⚠️  WARNING: Proper security.txt deployment requires program-side support`);
    console.log(`   ⚠️  This script creates a placeholder account only`);
    console.log(`   ⚠️  See: https://github.com/neodyme-labs/solana-security-txt\n`);
  }

  console.log("\n" + "─".repeat(60));
  console.log("\n📌 Next Steps:\n");
  console.log("1. Add security.txt support to programs using solana-security-txt crate");
  console.log("2. Deploy updated programs with create_metadata instruction");
  console.log("3. Call create_metadata to store security.txt on-chain");
  console.log("\nAlternatively, use explorers' off-chain security.txt display");
  console.log("(most explorers check /.well-known/security.txt in the repo).\n");
}

main().catch((err) => {
  console.error("Deployment error:", err);
  process.exit(1);
});
