import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, Keypair, Connection, Transaction, TransactionInstruction } from "@solana/web3.js";
import { TOKEN_2022_PROGRAM_ID, getAssociatedTokenAddressSync, createWithdrawWithheldTokensFromMintInstruction } from "@solana/spl-token";
import { readFileSync } from "fs";

import { CCM_MINT, ORACLE_PROGRAM_ID, deriveProtocolState } from "./lib/vault-pda.js";

function loadKeypairFromFile(filePath: string): Keypair {
  const raw = readFileSync(filePath, "utf-8");
  return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(raw)));
}

async function main() {
  const adminKeypair = loadKeypairFromFile(process.env.KEYPAIR!);
  const connection = new Connection(process.env.RPC_URL!, "confirmed");
  const wallet = new anchor.Wallet(adminKeypair);
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed", preflightCommitment: "confirmed" });
  anchor.setProvider(provider);

  const oracleIdl = await Program.fetchIdl(ORACLE_PROGRAM_ID, provider);
  if (!oracleIdl) throw new Error("Oracle IDL not found on-chain");
  const oracleProgram = new Program(oracleIdl, provider);

  const protocolState = deriveProtocolState(CCM_MINT);
  const protocolData: any = await oracleProgram.account.protocolState.fetch(protocolState);
  const treasuryOwner: PublicKey = protocolData.treasury;
  
  const treasuryAta = getAssociatedTokenAddressSync(
    CCM_MINT,
    treasuryOwner,
    true, // allowOwnerOffCurve
    TOKEN_2022_PROGRAM_ID,
  );

  console.log("Admin:", adminKeypair.publicKey.toBase58());
  console.log("Protocol State:", protocolState.toBase58());
  console.log("Treasury Owner:", treasuryOwner.toBase58());
  console.log("Treasury ATA:", treasuryAta.toBase58());
  
  try {
     const tx = await oracleProgram.methods
        .withdrawFeesFromMint()
        .accounts({
            admin: adminKeypair.publicKey,
            protocolState,
            mint: CCM_MINT,
            treasuryAta: treasuryAta,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
        })
        .rpc();
        
      console.log("Withdrawal transaction sent:", tx);
  } catch(e: any) {
      console.error("Failed to withdraw:", e.message);
  }
}

main();
