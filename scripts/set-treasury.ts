/**
 * Set the treasury wallet (fee destination owner)
 *
 * Usage: TREASURY=<wallet_pubkey> npx ts-node scripts/set-treasury.ts
 *
 * This sets the owner wallet whose ATA will receive harvested transfer fees.
 * The treasury field stores the OWNER, not the token account.
 * harvest_fees will send to ATA(treasury, mint).
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";

const ORACLE_PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");
const PROTOCOL_SEED = Buffer.from("protocol");

async function main() {
  const treasuryStr = process.env.TREASURY;

  if (!treasuryStr) {
    console.error("Usage: TREASURY=<wallet_pubkey> npx ts-node scripts/set-treasury.ts");
    console.error("");
    console.error("Example: TREASURY=2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD npx ts-node scripts/set-treasury.ts");
    process.exit(1);
  }

  const newTreasury = new PublicKey(treasuryStr);

  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const idl = await Program.fetchIdl(ORACLE_PROGRAM_ID, provider);
  if (!idl) throw new Error("Oracle IDL not found");
  const program = new Program(idl, provider);

  const admin = provider.wallet.publicKey;

  // Derive protocol state PDA
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, CCM_MINT.toBuffer()],
    ORACLE_PROGRAM_ID
  );

  console.log("\n🏦 SET TREASURY");
  console.log("===============");
  console.log("Protocol State:", protocolState.toBase58());
  console.log("Admin:", admin.toBase58());
  console.log("New Treasury (owner):", newTreasury.toBase58());

  try {
    const tx = await program.methods
      .setTreasury(newTreasury)
      .accounts({
        admin: admin,
        protocolState: protocolState,
      })
      .rpc();

    console.log("\n✅ TREASURY SET!");
    console.log("Signature:", tx);
    console.log("View: https://solscan.io/tx/" + tx);
    console.log("");
    console.log("Fees will now be harvested to ATA(" + newTreasury.toBase58().slice(0, 8) + "..., CCM)");
  } catch (e: any) {
    console.error("\n❌ Failed:", e.message || e);
    if (e.logs) {
      console.log("\nLogs:");
      e.logs.slice(-10).forEach((log: string) => console.log("  ", log));
    }
  }
}

main().catch(console.error);
