#!/usr/bin/env npx tsx
/**
 * Harvest Token-2022 withheld transfer fees for CCM
 * Uses the AO program's harvestFees instruction via CPI
 */

import pkg from "@coral-xyz/anchor";
const { Program, AnchorProvider, Wallet } = pkg;
import {
  ComputeBudgetProgram,
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import { getAssociatedTokenAddressSync, TOKEN_2022_PROGRAM_ID } from "@solana/spl-token";
import * as fs from "fs";

const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");
const AO_PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const PROTOCOL_SEED = Buffer.from("protocol");

function chunk<T>(items: T[], size: number): T[][] {
  const out: T[][] = [];
  for (let i = 0; i < items.length; i += size) out.push(items.slice(i, i + size));
  return out;
}

async function main() {
  const heliusKey = process.env.HELIUS_API_KEY;
  const rpcUrl = heliusKey
    ? `https://mainnet.helius-rpc.com/?api-key=${heliusKey}`
    : "https://api.mainnet-beta.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");

  // Load admin keypair
  const adminKeypairPath = process.env.HOME + "/.config/solana/id.json";
  const payer = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(adminKeypairPath, "utf-8")))
  );

  const wallet = new Wallet(payer);
  const provider = new AnchorProvider(connection, wallet, { commitment: "confirmed" });

  // Load IDL (use the working pattern)
  const idlPath = "/home/twzrd/attention-oracle-program/target/idl/token_2022.json";
  const idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
  const program = new Program(idl, provider);

  // Derive PDAs
  const [protocolState] = PublicKey.findProgramAddressSync([PROTOCOL_SEED, CCM_MINT.toBuffer()], AO_PROGRAM_ID);
  const [feeConfig] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, CCM_MINT.toBuffer(), Buffer.from("fee_config")],
    AO_PROGRAM_ID,
  );
  const treasuryAta = getAssociatedTokenAddressSync(CCM_MINT, protocolState, true, TOKEN_2022_PROGRAM_ID);

  console.log("\n=== Harvest Withheld Fees ===");
  console.log("RPC:           ", heliusKey ? "Helius (mainnet)" : rpcUrl);
  console.log("AO Program:    ", AO_PROGRAM_ID.toBase58());
  console.log("Mint:          ", CCM_MINT.toBase58());
  console.log("Payer:         ", payer.publicKey.toBase58());
  console.log("ProtocolState: ", protocolState.toBase58());
  console.log("FeeConfig:     ", feeConfig.toBase58());
  console.log("Treasury ATA:  ", treasuryAta.toBase58());

  // Enumerate token accounts for this mint
  console.log("\nEnumerating token accounts...");
  const tokenAccounts = await connection.getProgramAccounts(TOKEN_2022_PROGRAM_ID, {
    commitment: "confirmed",
    filters: [{ memcmp: { offset: 0, bytes: CCM_MINT.toBase58() } }],
  });

  const sources = tokenAccounts
    .map((x) => x.pubkey)
    .filter((pk) => !pk.equals(treasuryAta));

  console.log("Found " + sources.length + " token accounts (excluding treasury).");
  if (sources.length === 0) {
    console.log("Nothing to harvest.");
    return;
  }

  const batches = chunk(sources, 255);
  console.log("Batches: " + batches.length + " (max 255 accounts each)");

  for (let i = 0; i < batches.length; i++) {
    const batch = batches[i];
    console.log("\n--- Batch " + (i + 1) + "/" + batches.length + " (" + batch.length + " accounts) ---");

    const ix = await (program.methods as any)
      .harvestFees()
      .accounts({
        authority: payer.publicKey,
        protocolState,
        feeConfig,
        mint: CCM_MINT,
        treasury: treasuryAta,
        creatorPool: treasuryAta, // unused in 100% treasury mode
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .remainingAccounts(
        batch.map((pubkey) => ({
          pubkey,
          isWritable: true,
          isSigner: false,
        })),
      )
      .instruction();

    const tx = new Transaction()
      .add(ComputeBudgetProgram.setComputeUnitLimit({ units: 1_200_000 }))
      .add(ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 10_000 }))
      .add(ix);

    const sig = await sendAndConfirmTransaction(connection, tx, [payer], {
      commitment: "confirmed",
      skipPreflight: false,
    });
    console.log("✅ Sent. Signature:", sig);
    console.log("   Explorer: https://solscan.io/tx/" + sig);
  }

  console.log("\n✅ Done");
}

main().catch((err) => {
  console.error("\n❌ Error:", err);
  process.exit(1);
});
