#!/usr/bin/env ts-node

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import BN from "bn.js";
import { TOKEN_2022_PROGRAM_ID } from "@solana/spl-token";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const PROGRAM_ID = "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop";
const CCM_MINT = "ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe";

// Seeds
const PROTOCOL_SEED = Buffer.from("protocol");
const STAKE_POOL_SEED = Buffer.from("stake_pool");
const STAKE_VAULT_SEED = Buffer.from("stake_vault");

async function main() {
  const args = process.argv.slice(2);
  const rewardRate = args[0] ? new BN(args[0]) : new BN(0);

  console.log(`Initializing stake pool with reward_rate=${rewardRate.toString()}`);

  // Load wallet
  const walletPath = process.env.ANCHOR_WALLET || `${process.env.HOME}/.config/solana/id.json`;
  const walletKeypair = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(walletPath, "utf-8")))
  );

  // Setup connection
  const connection = new Connection(
    process.env.SYNDICA_RPC || "https://api.mainnet-beta.solana.com",
    "confirmed"
  );

  const wallet = new anchor.Wallet(walletKeypair);
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });
  anchor.setProvider(provider);

  // Load program
  const idl = JSON.parse(
    fs.readFileSync(`${__dirname}/../target/idl/token_2022.json`, "utf-8")
  );
  // Guard against missing account sizes
  if (idl.accounts) {
    idl.accounts.forEach((acc: any) => {
      if (acc.size === null || acc.size === undefined) {
        acc.size = 8 + 1000; // default
      }
    });
  }

  const program = new Program(idl, provider) as any;
  const mint = new PublicKey(CCM_MINT);

  // Derive PDAs
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, mint.toBuffer()],
    new PublicKey(PROGRAM_ID)
  );

  const [stakePool] = PublicKey.findProgramAddressSync(
    [STAKE_POOL_SEED, mint.toBuffer()],
    new PublicKey(PROGRAM_ID)
  );

  const [stakeVault] = PublicKey.findProgramAddressSync(
    [STAKE_VAULT_SEED, mint.toBuffer()],
    new PublicKey(PROGRAM_ID)
  );

  console.log("PDAs:");
  console.log("  Protocol State:", protocolState.toBase58());
  console.log("  Stake Pool:", stakePool.toBase58());
  console.log("  Stake Vault:", stakeVault.toBase58());
  console.log("  Mint:", mint.toBase58());
  console.log("  Admin:", walletKeypair.publicKey.toBase58());

  // Check if already initialized
  const poolInfo = await connection.getAccountInfo(stakePool);
  if (poolInfo) {
    console.log("\nStake pool already initialized!");
    process.exit(0);
  }

  // Build and send transaction
  try {
    const tx = await program.methods
      .initializeStakePool(rewardRate)
      .accounts({
        admin: walletKeypair.publicKey,
        protocolState,
        mint,
        stakePool,
        stakeVault,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    console.log("\nStake pool initialized!");
    console.log("Signature:", tx);
    console.log(`https://solscan.io/tx/${tx}`);
  } catch (err: any) {
    console.error("Error:", err.message);
    if (err.logs) {
      console.error("Logs:", err.logs.join("\n"));
    }
    process.exit(1);
  }
}

main().catch(console.error);
