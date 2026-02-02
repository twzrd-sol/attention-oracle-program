/**
 * Deploy lock-duration variants for audio playlists
 *
 * Creates 14 new pools: 7 playlists x 2 new tiers (3h + 12h).
 * Each pool requires 4 on-chain steps:
 *   1. Oracle: ChannelConfigV2 (new channel name, e.g. "audio:999:3h")
 *   2. Oracle: ChannelStakePool
 *   3. Vault:  ChannelVault (with lock_duration_slots for the tier)
 *   4. Vault:  vLOFI Metadata
 *
 * All steps are idempotent (skip if already exists).
 *
 * Usage:
 *   CLUSTER=mainnet-beta I_UNDERSTAND_MAINNET=1 \
 *   RPC_URL="https://..." KEYPAIR=/secure/admin.json \
 *     npx tsx scripts/admin/deploy-lock-variants.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import BN from "bn.js";
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
} from "@solana/spl-token";
import { readFileSync } from "fs";
import { keccak_256 } from "@noble/hashes/sha3";

import { requireScriptEnv } from "../script-guard.js";
import { CCM_V3_MINT, PROGRAM_ID as ORACLE_PROGRAM_ID } from "../config.js";
import {
  VAULT_PROGRAM_ID,
  METADATA_PROGRAM_ID,
  deriveProtocolState,
  deriveVault,
  deriveVlofiMint,
  deriveCcmBuffer,
  deriveOraclePosition,
  deriveOracleStakePool,
  deriveOracleStakeVault,
  deriveMetadata,
} from "../keepers/lib/vault-pda.js";
import type { ChannelEntry } from "../keepers/lib/channels.js";

// ============================================================================
// Configuration
// ============================================================================

const CHANNEL_CONFIG_V2_SEED = Buffer.from("channel_cfg_v2");
const MIN_DEPOSIT = new BN(1_000_000_000); // 1 CCM

/** Lock tiers to add (existing 7.5h / 54K slots pools are untouched) */
const LOCK_TIERS = [
  { suffix: "3h", lockDurationSlots: 27_000, withdrawQueueSlots: 9_000 },
  { suffix: "12h", lockDurationSlots: 108_000, withdrawQueueSlots: 9_000 },
];

/** Audio playlists to add lock tiers for */
const PLAYLISTS = [
  { id: "999", label: "999" },
  { id: "212", label: "212" },
  { id: "247", label: "247" },
  { id: "1999", label: "1999" },
  { id: "415", label: "415" },
  { id: "3121", label: "3121" },
  { id: "69", label: "69" },
];

// ============================================================================
// PDA Derivation
// ============================================================================

function deriveSubjectId(channel: string): Buffer {
  const input = Buffer.concat([
    Buffer.from("channel:"),
    Buffer.from(channel.toLowerCase()),
  ]);
  return Buffer.from(keccak_256(input));
}

function deriveChannelConfig(channelName: string): PublicKey {
  return PublicKey.findProgramAddressSync(
    [CHANNEL_CONFIG_V2_SEED, CCM_V3_MINT.toBuffer(), deriveSubjectId(channelName)],
    ORACLE_PROGRAM_ID,
  )[0];
}

// ============================================================================
// Helpers
// ============================================================================

async function accountExists(
  connection: Connection,
  pubkey: PublicKey,
): Promise<boolean> {
  const info = await connection.getAccountInfo(pubkey);
  return info !== null;
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

// ============================================================================
// Main
// ============================================================================

async function main() {
  const env = requireScriptEnv();

  const totalVariants = PLAYLISTS.length * LOCK_TIERS.length;
  console.log("=".repeat(70));
  console.log("  DEPLOY AUDIO LOCK-TIER VARIANTS");
  console.log("=".repeat(70));
  console.log(`\n  Cluster:    ${env.cluster}`);
  console.log(`  Playlists:  ${PLAYLISTS.length}`);
  console.log(`  Tiers:      ${LOCK_TIERS.map((t) => t.suffix).join(", ")}`);
  console.log(`  Total new:  ${totalVariants} pools`);
  console.log();

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

  console.log(`  Admin:      ${adminKeypair.publicKey.toBase58()}`);
  console.log(`  CCM Mint:   ${CCM_V3_MINT.toBase58()}`);

  // Load IDLs
  const oracleIdl = await Program.fetchIdl(ORACLE_PROGRAM_ID, provider);
  if (!oracleIdl) throw new Error("Oracle IDL not found on-chain");
  const oracleProgram = new Program(oracleIdl, provider);

  const vaultIdl = await Program.fetchIdl(VAULT_PROGRAM_ID, provider);
  if (!vaultIdl) throw new Error("Vault IDL not found on-chain");
  const vaultProgram = new Program(vaultIdl, provider);

  const protocolState = deriveProtocolState(CCM_V3_MINT);
  console.log(`  Protocol:   ${protocolState.toBase58()}\n`);

  // Track results
  let created = 0;
  let skipped = 0;
  const newChannels: ChannelEntry[] = [];

  for (const playlist of PLAYLISTS) {
    for (const tier of LOCK_TIERS) {
      const channelName = `audio:${playlist.id}:${tier.suffix}`;
      const vaultName = `audio-${playlist.id}-${tier.suffix}`;
      const vlofiLabel = `vLOFI ${playlist.label} ${tier.suffix}`;

      console.log(`--- ${vaultName} ---`);

      // Derive all PDAs
      const channelConfig = deriveChannelConfig(channelName);
      const stakePool = deriveOracleStakePool(channelConfig);
      const stakeVault = deriveOracleStakeVault(stakePool);
      const vault = deriveVault(channelConfig);
      const vlofiMint = deriveVlofiMint(vault);
      const ccmBuffer = deriveCcmBuffer(vault);
      const oraclePosition = deriveOraclePosition(vault);
      const metadata = deriveMetadata(vlofiMint);

      // Step 1: Create ChannelConfigV2
      if (await accountExists(connection, channelConfig)) {
        console.log(`  [SKIP] Channel '${channelName}' exists`);
      } else {
        console.log(`  [INIT] Channel '${channelName}'`);
        const tx = await oracleProgram.methods
          .initializeChannelCumulative(
            channelName,
            new BN(0),
            adminKeypair.publicKey,
            0,
          )
          .accounts({
            payer: adminKeypair.publicKey,
            protocolState,
            channelConfig,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
        console.log(`         tx: ${tx}`);
        created++;
        await sleep(500);
      }

      // Step 2: Create ChannelStakePool
      if (await accountExists(connection, stakePool)) {
        console.log(`  [SKIP] Stake pool exists`);
      } else {
        console.log(`  [INIT] Stake pool`);
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
        console.log(`         tx: ${tx}`);
        created++;
        await sleep(500);
      }

      // Step 3: Create ChannelVault
      if (await accountExists(connection, vault)) {
        console.log(`  [SKIP] Vault exists`);
      } else {
        console.log(
          `  [INIT] Vault (lock=${tier.lockDurationSlots} slots, queue=${tier.withdrawQueueSlots} slots)`,
        );
        const tx = await vaultProgram.methods
          .initializeVault(
            MIN_DEPOSIT,
            new BN(tier.lockDurationSlots),
            new BN(tier.withdrawQueueSlots),
          )
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
        console.log(`         tx: ${tx}`);
        created++;
        await sleep(500);
      }

      // Step 4: Set vLOFI metadata
      if (await accountExists(connection, metadata)) {
        console.log(`  [SKIP] Metadata exists`);
      } else {
        console.log(`  [META] "${vlofiLabel}"`);
        const tx = await vaultProgram.methods
          .setVlofiMetadata(vlofiLabel, "vLOFI", "")
          .accounts({
            admin: adminKeypair.publicKey,
            vault,
            vlofiMint,
            metadata,
            metadataProgram: METADATA_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
        console.log(`         tx: ${tx}`);
        created++;
        await sleep(500);
      }

      // Collect new channel entry for channels.ts
      newChannels.push({
        name: vaultName,
        label: vlofiLabel,
        channelConfig: channelConfig.toBase58(),
        lockDurationSlots: tier.lockDurationSlots,
        withdrawQueueSlots: tier.withdrawQueueSlots,
      });

      skipped += 4 - (created - skipped); // approximate
      console.log();
      await sleep(1000);
    }
  }

  // Summary
  console.log("=".repeat(70));
  console.log("  SUMMARY");
  console.log("=".repeat(70));
  console.log(`\n  Created: ${created} accounts`);
  console.log(`  New pools: ${newChannels.length}`);

  // Output channels.ts entries
  console.log(`\n${"=".repeat(70)}`);
  console.log("  ADD TO scripts/keepers/lib/channels.ts:");
  console.log("=".repeat(70));
  console.log();
  console.log("  // Audio lock-tier variants (3h + 12h)");

  for (const ch of newChannels) {
    console.log(`  {`);
    console.log(`    name: "${ch.name}",`);
    console.log(`    label: "${ch.label}",`);
    console.log(`    channelConfig: "${ch.channelConfig}",`);
    console.log(`    lockDurationSlots: ${ch.lockDurationSlots.toLocaleString().replace(/,/g, "_")},`);
    console.log(`    withdrawQueueSlots: ${ch.withdrawQueueSlots.toLocaleString().replace(/,/g, "_")},`);
    console.log(`  },`);
  }

  console.log(`\n  Next steps:`);
  console.log(`  1. Add the entries above to channels.ts`);
  console.log(`  2. Deploy updated compound-keeper`);
  console.log(`  3. Set reward rates via Squads as deposits arrive`);
  console.log();
}

main().catch((err) => {
  console.error("FATAL:", err.message || err);
  if (err.logs) {
    err.logs.slice(-10).forEach((l: string) => console.error("  ", l));
  }
  process.exit(1);
});
