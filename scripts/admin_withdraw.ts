/**
 * Admin Withdraw Script
 * Withdraws CCM from treasury to a destination wallet
 *
 * Usage: npx ts-node scripts/admin_withdraw.ts <amount_ccm>
 */

import pkg from "@coral-xyz/anchor";
const { Program, AnchorProvider, Wallet, BN } = pkg;
import { Connection, Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Constants
const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");
const DESTINATION_WALLET = new PublicKey("AmMftc4zHgR4yYfv29awV9Q46emo2aGPFW8utP81CsBv");
const PROTOCOL_SEED = Buffer.from("protocol");
const WITHDRAW_TRACKER_SEED = Buffer.from("withdraw_tracker");

async function main() {
  const amountArg = process.argv[2];
  if (!amountArg) {
    console.error("Usage: npx ts-node scripts/admin_withdraw.ts <amount_ccm>");
    console.error("Example: npx ts-node scripts/admin_withdraw.ts 1000000");
    process.exit(1);
  }

  const amountCcm = parseInt(amountArg, 10);
  const amountLamports = BigInt(amountCcm) * BigInt(1_000_000_000); // 9 decimals

  console.log(`Withdrawing ${amountCcm} CCM (${amountLamports} lamports)...`);

  // Load admin keypair
  const adminKeypairPath = path.join(process.env.HOME!, ".config/solana/id.json");
  const adminKeypair = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(adminKeypairPath, "utf-8")))
  );
  console.log(`Admin: ${adminKeypair.publicKey.toBase58()}`);

  // Setup connection and provider
  const connection = new Connection("https://api.mainnet-beta.solana.com", "confirmed");
  const wallet = new Wallet(adminKeypair);
  const provider = new AnchorProvider(connection, wallet, { commitment: "confirmed" });

  // Load IDL
  const idlPath = path.join(__dirname, "../target/idl/token_2022.json");
  const idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
  const program = new Program(idl, provider);

  // Derive PDAs
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, CCM_MINT.toBuffer()],
    PROGRAM_ID
  );
  console.log(`Protocol State: ${protocolState.toBase58()}`);

  const [withdrawTracker] = PublicKey.findProgramAddressSync(
    [WITHDRAW_TRACKER_SEED, CCM_MINT.toBuffer()],
    PROGRAM_ID
  );
  console.log(`Withdraw Tracker: ${withdrawTracker.toBase58()}`);

  // Get ATAs
  const treasuryAta = getAssociatedTokenAddressSync(
    CCM_MINT,
    protocolState,
    true, // allowOwnerOffCurve (PDA)
    TOKEN_2022_PROGRAM_ID
  );
  console.log(`Treasury ATA: ${treasuryAta.toBase58()}`);

  const destinationAta = getAssociatedTokenAddressSync(
    CCM_MINT,
    DESTINATION_WALLET,
    false,
    TOKEN_2022_PROGRAM_ID
  );
  console.log(`Destination ATA: ${destinationAta.toBase58()}`);

  // Get hook program and extra account metas for Token-2022 transfer
  // CCM has a transfer hook, need to pass remaining accounts
  const hookProgram = new PublicKey("8VE2Wt5JNusGUCwkrpVmdwgdgyD6vYPEHop2g2CAArzS");

  // Extra accounts for transfer hook (from EAML)
  const extraAccountMetas = await getExtraAccountMetas(connection, CCM_MINT, hookProgram, protocolState);

  console.log("\nCalling admin_withdraw...");

  try {
    const tx = await program.methods
      .adminWithdraw(new BN(amountLamports.toString()))
      .accounts({
        admin: adminKeypair.publicKey,
        protocolState,
        withdrawTracker,
        mint: CCM_MINT,
        treasuryAta,
        destinationAta,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(extraAccountMetas)
      .rpc();

    console.log(`\n✅ Success! Tx: ${tx}`);
    console.log(`   Explorer: https://solscan.io/tx/${tx}`);
  } catch (e: any) {
    console.error("\n❌ Error:", e.message || e);
    if (e.logs) {
      console.error("\nProgram logs:");
      e.logs.forEach((log: string) => console.error("  ", log));
    }
    process.exit(1);
  }
}

async function getExtraAccountMetas(
  connection: Connection,
  mint: PublicKey,
  hookProgram: PublicKey,
  protocolState: PublicKey
): Promise<{ pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[]> {
  // The Extra Account Meta List (EAML) PDA
  const [eaml] = PublicKey.findProgramAddressSync(
    [Buffer.from("extra-account-metas"), mint.toBuffer()],
    hookProgram
  );

  // For CCM's transfer hook, we need based on the hook implementation:
  // - Hook program
  // - EAML account
  // - Oracle program (the AO program itself)
  // - Node score account (PDA)
  // - Treasury ATA

  // Derive node_score PDA (placeholder - may not be needed for admin transfers)
  const [nodeScore] = PublicKey.findProgramAddressSync(
    [Buffer.from("node_score"), protocolState.toBuffer()],
    hookProgram
  );

  return [
    { pubkey: hookProgram, isSigner: false, isWritable: false },
    { pubkey: eaml, isSigner: false, isWritable: false },
    { pubkey: new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"), isSigner: false, isWritable: false }, // AO program
  ];
}

main().catch(console.error);
