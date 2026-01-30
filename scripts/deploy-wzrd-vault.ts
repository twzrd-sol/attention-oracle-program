/**
 * Deploy WZRD Vault to devnet (or any cluster)
 *
 * Prerequisites:
 *   - Both programs deployed: `anchor deploy --provider.cluster devnet`
 *   - .env.ccm-v3 configured with CCM_V3_MINT
 *   - Admin keypair at KEYPAIR path
 *
 * Steps:
 *   1. Initialize "wzrd" channel on Oracle
 *   2. Initialize stake pool
 *   3. Initialize vault (6h lock, 1h queue)
 *   4. Set vLOFI metadata via Metaplex
 *   5. Print all PDAs for backend config
 *
 * Usage:
 *   CLUSTER=devnet RPC_URL=https://api.devnet.solana.com KEYPAIR=~/.config/solana/id.json \
 *     npx tsx scripts/deploy-wzrd-vault.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  Connection,
} from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { readFileSync } from "fs";
import { keccak_256 } from "@noble/hashes/sha3";
import { requireScriptEnv } from "./script-guard.js";
import { CCM_V3_MINT, PROGRAM_ID as ORACLE_PROGRAM_ID } from "./config.js";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const VAULT_PROGRAM_ID = new PublicKey("5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ");
const METADATA_PROGRAM_ID = new PublicKey("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

const CHANNEL_NAME = "wzrd";
const LOCK_DURATION_SLOTS = 54_000; // 6 hours
const WITHDRAW_QUEUE_SLOTS = 9_000; // 1 hour
const MIN_DEPOSIT = new anchor.BN(1_000_000_000); // 1 CCM

// Oracle seeds
const PROTOCOL_SEED = Buffer.from("protocol");
const CHANNEL_CONFIG_V2_SEED = Buffer.from("channel_cfg_v2");
const CHANNEL_STAKE_POOL_SEED = Buffer.from("channel_pool");
const STAKE_VAULT_SEED = Buffer.from("stake_vault");

// Vault seeds
const VAULT_SEED = Buffer.from("vault");
const VAULT_CCM_BUFFER_SEED = Buffer.from("vault_ccm");
const VLOFI_MINT_SEED = Buffer.from("vlofi");
const VAULT_ORACLE_POSITION_SEED = Buffer.from("vault_oracle");

// ---------------------------------------------------------------------------
// PDA Derivation
// ---------------------------------------------------------------------------
function deriveSubjectId(channel: string): Buffer {
  const input = Buffer.concat([Buffer.from("channel:"), Buffer.from(channel.toLowerCase())]);
  return Buffer.from(keccak_256(input));
}

function deriveProtocolState(): PublicKey {
  return PublicKey.findProgramAddressSync([PROTOCOL_SEED, CCM_V3_MINT.toBuffer()], ORACLE_PROGRAM_ID)[0];
}

function deriveFeeConfig(): PublicKey {
  return PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, CCM_V3_MINT.toBuffer(), Buffer.from("fee_config")],
    ORACLE_PROGRAM_ID
  )[0];
}

function deriveChannelConfig(): PublicKey {
  return PublicKey.findProgramAddressSync(
    [CHANNEL_CONFIG_V2_SEED, CCM_V3_MINT.toBuffer(), deriveSubjectId(CHANNEL_NAME)],
    ORACLE_PROGRAM_ID
  )[0];
}

function deriveStakePool(channelConfig: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync([CHANNEL_STAKE_POOL_SEED, channelConfig.toBuffer()], ORACLE_PROGRAM_ID)[0];
}

function deriveStakeVault(stakePool: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync([STAKE_VAULT_SEED, stakePool.toBuffer()], ORACLE_PROGRAM_ID)[0];
}

function deriveVault(channelConfig: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync([VAULT_SEED, channelConfig.toBuffer()], VAULT_PROGRAM_ID)[0];
}

function deriveVlofiMint(vault: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync([VLOFI_MINT_SEED, vault.toBuffer()], VAULT_PROGRAM_ID)[0];
}

function deriveCcmBuffer(vault: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync([VAULT_CCM_BUFFER_SEED, vault.toBuffer()], VAULT_PROGRAM_ID)[0];
}

function deriveOraclePosition(vault: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync([VAULT_ORACLE_POSITION_SEED, vault.toBuffer()], VAULT_PROGRAM_ID)[0];
}

function deriveMetadata(mint: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("metadata"), METADATA_PROGRAM_ID.toBuffer(), mint.toBuffer()],
    METADATA_PROGRAM_ID
  )[0];
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
async function accountExists(connection: Connection, pubkey: PublicKey): Promise<boolean> {
  const info = await connection.getAccountInfo(pubkey);
  return info !== null;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------
async function main() {
  const env = requireScriptEnv();
  console.log("=== DEPLOY WZRD VAULT ===");
  console.log("Cluster:", env.cluster);
  console.log("Channel:", CHANNEL_NAME);
  console.log("Lock:", LOCK_DURATION_SLOTS, "slots (6h)");
  console.log("Queue:", WITHDRAW_QUEUE_SLOTS, "slots (1h)");
  console.log("");

  // Setup provider
  const keypairData = JSON.parse(readFileSync(env.keypairPath, "utf-8"));
  const adminKeypair = Keypair.fromSecretKey(Uint8Array.from(keypairData));
  const connection = new Connection(env.rpcUrl, "confirmed");
  const wallet = new anchor.Wallet(adminKeypair);
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
    preflightCommitment: "confirmed",
  });
  anchor.setProvider(provider);

  console.log("Admin:", adminKeypair.publicKey.toBase58());
  console.log("CCM Mint:", CCM_V3_MINT.toBase58());

  // Load IDLs
  const oracleIdl = await Program.fetchIdl(ORACLE_PROGRAM_ID, provider);
  if (!oracleIdl) throw new Error("Oracle IDL not found on-chain. Deploy Oracle program first.");
  const oracleProgram = new Program(oracleIdl, provider);

  const vaultIdl = await Program.fetchIdl(VAULT_PROGRAM_ID, provider);
  if (!vaultIdl) throw new Error("Vault IDL not found on-chain. Deploy Vault program first.");
  const vaultProgram = new Program(vaultIdl, provider);

  // Derive all PDAs
  const protocolState = deriveProtocolState();
  const feeConfig = deriveFeeConfig();
  const channelConfig = deriveChannelConfig();
  const stakePool = deriveStakePool(channelConfig);
  const stakeVault = deriveStakeVault(stakePool);
  const vault = deriveVault(channelConfig);
  const vlofiMint = deriveVlofiMint(vault);
  const ccmBuffer = deriveCcmBuffer(vault);
  const oraclePosition = deriveOraclePosition(vault);
  const metadata = deriveMetadata(vlofiMint);

  console.log("");
  console.log("--- PDAs ---");
  console.log("Protocol State:", protocolState.toBase58());
  console.log("Channel Config:", channelConfig.toBase58());
  console.log("Stake Pool:    ", stakePool.toBase58());
  console.log("Vault:         ", vault.toBase58());
  console.log("vLOFI Mint:    ", vlofiMint.toBase58());
  console.log("CCM Buffer:    ", ccmBuffer.toBase58());
  console.log("Oracle Pos:    ", oraclePosition.toBase58());
  console.log("Metadata:      ", metadata.toBase58());
  console.log("");

  // -----------------------------------------------------------------------
  // Step 1: Initialize "wzrd" channel (skip if exists)
  // -----------------------------------------------------------------------
  if (await accountExists(connection, channelConfig)) {
    console.log("[SKIP] Channel 'wzrd' already exists");
  } else {
    console.log("[INIT] Creating channel 'wzrd'...");
    const tx = await oracleProgram.methods
      .initializeChannelCumulative(
        CHANNEL_NAME,
        new anchor.BN(0),
        adminKeypair.publicKey,
        0
      )
      .accounts({
        payer: adminKeypair.publicKey,
        protocolState,
        channelConfig,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    console.log("  tx:", tx);
  }

  // -----------------------------------------------------------------------
  // Step 2: Initialize stake pool (skip if exists)
  // -----------------------------------------------------------------------
  if (await accountExists(connection, stakePool)) {
    console.log("[SKIP] Stake pool already exists");
  } else {
    console.log("[INIT] Creating stake pool...");
    const tx = await oracleProgram.methods
      .initializeStakePool()
      .accounts({
        payer: adminKeypair.publicKey,
        protocolState,
        channelConfig,
        mint: CCM_V3_MINT,
        stakePool,
        vault: stakeVault,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    console.log("  tx:", tx);
  }

  // -----------------------------------------------------------------------
  // Step 3: Initialize vault (skip if exists)
  // -----------------------------------------------------------------------
  if (await accountExists(connection, vault)) {
    console.log("[SKIP] Vault already exists");
  } else {
    console.log("[INIT] Creating vault (6h lock, 1h queue)...");
    const tx = await vaultProgram.methods
      .initializeVault(MIN_DEPOSIT, new anchor.BN(LOCK_DURATION_SLOTS), new anchor.BN(WITHDRAW_QUEUE_SLOTS))
      .accounts({
        admin: adminKeypair.publicKey,
        oracleProtocol: protocolState,
        oracleChannelConfig: channelConfig,
        ccmMint: CCM_V3_MINT,
        vault,
        ccmBuffer,
        vlofiMint,
        vaultOraclePosition: oraclePosition,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .rpc();
    console.log("  tx:", tx);
  }

  // -----------------------------------------------------------------------
  // Step 4: Set vLOFI metadata (skip if exists)
  // -----------------------------------------------------------------------
  if (await accountExists(connection, metadata)) {
    console.log("[SKIP] vLOFI metadata already exists");
  } else {
    console.log("[META] Setting vLOFI metadata...");
    const tx = await vaultProgram.methods
      .setVlofiMetadata("vLOFI WZRD", "vLOFI", "")
      .accounts({
        admin: adminKeypair.publicKey,
        vault,
        vlofiMint,
        metadata,
        metadataProgram: METADATA_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    console.log("  tx:", tx);
  }

  // -----------------------------------------------------------------------
  // Output: Backend config
  // -----------------------------------------------------------------------
  console.log("");
  console.log("=== BACKEND CONFIG (copy to .env) ===");
  console.log(`VAULT_PROGRAM_ID=${VAULT_PROGRAM_ID.toBase58()}`);
  console.log(`ORACLE_PROGRAM_ID=${ORACLE_PROGRAM_ID.toBase58()}`);
  console.log(`CCM_MINT=${CCM_V3_MINT.toBase58()}`);
  console.log(`WZRD_CHANNEL_CONFIG=${channelConfig.toBase58()}`);
  console.log(`WZRD_VAULT=${vault.toBase58()}`);
  console.log(`WZRD_VLOFI_MINT=${vlofiMint.toBase58()}`);
  console.log(`WZRD_CCM_BUFFER=${ccmBuffer.toBase58()}`);
  console.log(`WZRD_ORACLE_POSITION=${oraclePosition.toBase58()}`);
  console.log(`WZRD_STAKE_POOL=${stakePool.toBase58()}`);
  console.log(`WZRD_STAKE_VAULT=${stakeVault.toBase58()}`);
  console.log(`WZRD_METADATA=${metadata.toBase58()}`);
  console.log("");
  console.log("=== DONE ===");
}

main().catch((err) => {
  console.error("FATAL:", err.message || err);
  if (err.logs) err.logs.slice(-10).forEach((l: string) => console.error("  ", l));
  process.exit(1);
});
