/**
 * Initialize Oracle stake pools for trial vault channels
 * Must be run by protocol admin (relayer.json)
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { TOKEN_2022_PROGRAM_ID } from "@solana/spl-token";
import { readFileSync } from "fs";

// Program IDs
const ORACLE_PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");
const PROTOCOL_STATE = new PublicKey("596VBoVvzASAhe38CcBSJnv1LdVFPu4EdB8gw1Ko2nx3");

// Seeds
const CHANNEL_STAKE_POOL_SEED = Buffer.from("channel_pool");
const STAKE_VAULT_SEED = Buffer.from("stake_vault");

// Trial vault channel configs
const CHANNELS: Record<string, string> = {
  "lofi-vault-3h": "J3HAT4NbL6REyyNqbW1BDGF9BXXc3FYuQ1fr6NbCQaoW",
  "lofi-vault-6h": "dJvatt5bga4ak64ghTLEtxs1jxfLX4TNoZuvfiDCcGy",
  "lofi-vault-9h": "2TWM1H1gHWrA6Ta6A9tH3E1TTTRbPpmSL2Xg7KdHwxCM",
  "lofi-vault-12h": "GZL7vAo9vxdNbsmrreVueVd1Xm9oWmatkQauFcxhq8qP",
};

async function main() {
  // Load admin wallet (protocol admin = id.json)
  const relayerPath = process.env.HOME + "/.config/solana/id.json";
  const secretKey = JSON.parse(readFileSync(relayerPath, "utf-8"));
  const admin = Keypair.fromSecretKey(Uint8Array.from(secretKey));

  const connection = new Connection("https://api.mainnet-beta.solana.com", "confirmed");
  const wallet = new anchor.Wallet(admin);
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed" });
  anchor.setProvider(provider);

  // Fetch IDL from chain
  const idl = await Program.fetchIdl(ORACLE_PROGRAM_ID, provider);
  if (!idl) throw new Error("Oracle IDL not found");
  const program = new Program(idl, provider);

  console.log("🏦 INITIALIZING ORACLE STAKE POOLS");
  console.log("===================================");
  console.log("Admin:", admin.publicKey.toBase58());
  console.log("");

  for (const [name, configAddr] of Object.entries(CHANNELS)) {
    console.log(`\n--- ${name} ---`);

    const channelConfig = new PublicKey(configAddr);

    // Derive PDAs
    const [stakePool] = PublicKey.findProgramAddressSync(
      [CHANNEL_STAKE_POOL_SEED, channelConfig.toBuffer()],
      ORACLE_PROGRAM_ID
    );

    const [vault] = PublicKey.findProgramAddressSync(
      [STAKE_VAULT_SEED, stakePool.toBuffer()],
      ORACLE_PROGRAM_ID
    );

    console.log("Channel Config:", configAddr);
    console.log("Stake Pool:", stakePool.toBase58());
    console.log("Vault:", vault.toBase58());

    // Check if already exists
    const info = await connection.getAccountInfo(stakePool);
    if (info) {
      console.log("✅ Already initialized, skipping");
      continue;
    }

    try {
      const tx = await program.methods
        .initializeStakePool()
        .accounts({
          payer: admin.publicKey,
          protocolState: PROTOCOL_STATE,
          channelConfig: channelConfig,
          mint: CCM_MINT,
          stakePool: stakePool,
          vault: vault,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      console.log("✅ Initialized:", tx);
    } catch (err: any) {
      console.error("❌ Failed:", err.message || err);
      if (err.logs) {
        err.logs.slice(-5).forEach((log: string) => console.log("  ", log));
      }
    }

    // Small delay to avoid rate limits
    await new Promise(r => setTimeout(r, 1000));
  }

  console.log("\n\n✅ Done! Stake pools initialized.");
}

main().catch(console.error);
