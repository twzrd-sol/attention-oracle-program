/**
 * Test deposit into a trial vault
 * Usage: VAULT=lofi-vault-3h AMOUNT=100 npx ts-node scripts/test-deposit.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import BN from "bn.js";
import { PublicKey, SystemProgram } from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";

// Program IDs
const PROGRAM_ID = new PublicKey("5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");

// Trial vault configs
const TRIAL_VAULTS: Record<string, { vault: string; vlofi: string; buffer: string }> = {
  "lofi-vault-3h": {
    vault: "7tjCgZcsK4sgV65wsNajUVRuGHQ7GZELWfTaWYbCBDTw",
    vlofi: "E9Kt33axpCy3ve2PCY9BSrbPhcR9wdDsWQECAahzw2dS",
    buffer: "", // Will derive
  },
  "lofi-vault-6h": {
    vault: "3BumiGZYw96eiyHEjy3wkjnrBTgcUspYmFHHptMpHof9",
    vlofi: "pZ5RyPEB9CS9SBjtidHARtQHqaqFT9qWKLLzohJSn4H",
    buffer: "",
  },
  "lofi-vault-9h": {
    vault: "BnN5JfewvFZ93RFsduKyYbBc3NYvVc4xuYRDsMptEWu8",
    vlofi: "HUhqcKzaYabscWm31YsJYLn4kRxsNrKYgLmJu69fRdCp",
    buffer: "",
  },
  "lofi-vault-12h": {
    vault: "8j7M2aQg7FdaN6dTW33km2zfJX5USVqQwSZ2WPA4kaPz",
    vlofi: "FWKim8StacRqPQ5Cq9QhMwbqHciCC4M1jj56B2FKq63p",
    buffer: "",
  },
};

// Buffer seed
const VAULT_CCM_BUFFER_SEED = Buffer.from("vault_ccm");

async function main() {
  const vaultName = process.env.VAULT || "lofi-vault-3h";
  const amountCcm = parseInt(process.env.AMOUNT || "100", 10);

  const config = TRIAL_VAULTS[vaultName];
  if (!config) {
    console.error(`Unknown vault: ${vaultName}`);
    console.error("Available:", Object.keys(TRIAL_VAULTS).join(", "));
    process.exit(1);
  }

  // Setup
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const idl = await Program.fetchIdl(PROGRAM_ID, provider);
  if (!idl) throw new Error("IDL not found");
  const program = new Program(idl, provider);

  const user = provider.wallet.publicKey;
  const vault = new PublicKey(config.vault);
  const vlofiMint = new PublicKey(config.vlofi);

  // Derive CCM buffer
  const [ccmBuffer] = PublicKey.findProgramAddressSync(
    [VAULT_CCM_BUFFER_SEED, vault.toBuffer()],
    PROGRAM_ID
  );

  console.log("💰 DEPOSIT TEST");
  console.log("================");
  console.log("User:", user.toBase58());
  console.log("Vault:", vaultName);
  console.log("Amount:", amountCcm, "CCM");
  console.log("");

  // Get user's CCM ATA (Token-2022)
  const userCcm = getAssociatedTokenAddressSync(
    CCM_MINT,
    user,
    false,
    TOKEN_2022_PROGRAM_ID
  );

  // Get user's vLOFI ATA (standard SPL)
  const userVlofi = getAssociatedTokenAddressSync(
    vlofiMint,
    user,
    false,
    TOKEN_PROGRAM_ID
  );

  console.log("Addresses:");
  console.log("  Vault:", vault.toBase58());
  console.log("  vLOFI Mint:", vlofiMint.toBase58());
  console.log("  CCM Buffer:", ccmBuffer.toBase58());
  console.log("  User CCM:", userCcm.toBase58());
  console.log("  User vLOFI:", userVlofi.toBase58());
  console.log("");

  // Check user's CCM balance
  try {
    const balance = await provider.connection.getTokenAccountBalance(userCcm);
    console.log("User CCM Balance:", balance.value.uiAmount, "CCM");
  } catch {
    console.error("User has no CCM token account!");
    process.exit(1);
  }

  // Deposit
  const depositAmount = new BN(amountCcm * 1e9); // CCM has 9 decimals
  const minShares = new BN(1); // Accept any shares

  console.log(`\nDepositing ${amountCcm} CCM...`);

  try {
    const tx = await program.methods
      .deposit(depositAmount, minShares)
      .accounts({
        user: user,
        vault: vault,
        ccmMint: CCM_MINT,
        vlofiMint: vlofiMint,
        userCcm: userCcm,
        vaultCcmBuffer: ccmBuffer,
        userVlofi: userVlofi,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    console.log("");
    console.log("✅ DEPOSIT SUCCESSFUL!");
    console.log("Signature:", tx);
    console.log("View: https://solscan.io/tx/" + tx);

    // Check new balances
    console.log("");
    console.log("New balances:");

    const newCcmBalance = await provider.connection.getTokenAccountBalance(userCcm);
    console.log("  CCM:", newCcmBalance.value.uiAmount);

    try {
      const newVlofiBalance = await provider.connection.getTokenAccountBalance(userVlofi);
      console.log("  vLOFI:", newVlofiBalance.value.uiAmount);
    } catch {
      console.log("  vLOFI: (checking...)");
    }

  } catch (e: any) {
    console.error("❌ Deposit failed:", e.message || e);
    if (e.logs) {
      console.log("\nLogs:");
      e.logs.forEach((log: string) => console.log("  ", log));
    }
    process.exit(1);
  }
}

main();
