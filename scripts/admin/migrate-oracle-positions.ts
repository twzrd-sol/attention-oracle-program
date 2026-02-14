/**
 * Migrate Oracle Positions via Squads Multisig
 *
 * Creates Squads vault transaction proposals to run migrate_oracle_position
 * on all configured channel vaults, initializing the VaultOraclePosition accounts
 * that are required for compound to work.
 *
 * Vault list is loaded from the env-driven channel registry:
 *   - TWZRD_CHANNELS_JSON (preferred)
 *   - TWZRD_CHANNELS_PATH
 *
 * PREREQUISITE: Channel Vault program must be upgraded on-chain to include
 * the migrate_oracle_position instruction (added in commit 7983b67).
 *
 * Creates one proposal per vault (up to 16).
 * Each gets 2 local approvals (2pHj + 87d5).
 * User provides 3rd approval + executes via Squads UI.
 *
 * Usage:
 *   # Create proposals + approve with 2 local keys
 *   RPC_URL="..." npx tsx scripts/admin/migrate-oracle-positions.ts
 *
 *   # Execute all approved proposals (after 3rd approval in Squads UI)
 *   RPC_URL="..." npx tsx scripts/admin/migrate-oracle-positions.ts --execute-all <startIdx>
 */

import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  TransactionMessage,
  TransactionInstruction,
} from "@solana/web3.js";
import * as multisig from "@sqds/multisig";
import { createHash } from "crypto";
import * as fs from "fs";
import { CHANNELS } from "../keepers/lib/channels.js";

// ============================================================================
// Constants
// ============================================================================

const VAULT_PROGRAM_ID = new PublicKey("5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ");
const MULTISIG_PDA = new PublicKey("BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ");

const VAULT_SEED = Buffer.from("vault");
const VAULT_ORACLE_POSITION_SEED = Buffer.from("vault_oracle");

const KEYPAIR_PATHS = [
  `${process.env.HOME}/.config/solana/id.json`,              // 2pHj...
  `${process.env.HOME}/.config/solana/oracle-authority.json`, // 87d5...
];

// ============================================================================
// Helpers
// ============================================================================

function loadKeypair(path: string): Keypair {
  const raw = JSON.parse(fs.readFileSync(path, "utf-8"));
  return Keypair.fromSecretKey(new Uint8Array(raw));
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

/** Anchor discriminator: sha256("global:<name>")[0..8] */
function anchorDiscriminator(instructionName: string): Buffer {
  const hash = createHash("sha256")
    .update(`global:${instructionName}`)
    .digest();
  return Buffer.from(hash.subarray(0, 8));
}

/**
 * Build the migrate_oracle_position instruction.
 *
 * Accounts (from MigrateOraclePosition struct):
 *   0. admin           (signer, writable — payer for init)
 *   1. vault           (writable — seeds verified by program)
 *   2. vault_oracle_position (writable — init)
 *   3. system_program
 */
function migrateOraclePositionIx(
  admin: PublicKey,
  vault: PublicKey,
  vaultOraclePosition: PublicKey,
): TransactionInstruction {
  return new TransactionInstruction({
    programId: VAULT_PROGRAM_ID,
    keys: [
      { pubkey: admin,               isSigner: true,  isWritable: true },
      { pubkey: vault,               isSigner: false, isWritable: true },
      { pubkey: vaultOraclePosition, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: anchorDiscriminator("migrate_oracle_position"),
  });
}

interface VaultTarget {
  name: string;
  channelConfig: PublicKey;
  vault: PublicKey;
  oraclePosition: PublicKey;
  exists: boolean;
}

// ============================================================================
// Create: proposals + approvals for all vaults
// ============================================================================

async function createAndApprove(connection: Connection): Promise<void> {
  console.log("\n" + "=".repeat(60));
  console.log("  Oracle Position Migration - Squads Vault Transactions");
  console.log("=".repeat(60) + "\n");

  // Load keypairs
  const keypairs = KEYPAIR_PATHS.map((p) => {
    const kp = loadKeypair(p);
    console.log(`  Loaded: ${kp.publicKey.toBase58().slice(0, 8)}... (${p.split("/").pop()})`);
    return kp;
  });

  // Derive Squads vault PDA
  const [vaultPda] = multisig.getVaultPda({
    multisigPda: MULTISIG_PDA,
    index: 0,
  });
  console.log(`\n  Squads vault:    ${vaultPda.toBase58()}`);
  console.log(`  Vault program:   ${VAULT_PROGRAM_ID.toBase58()}`);

  // Check vault balance
  const vaultBalance = await connection.getBalance(vaultPda);
  console.log(`  Vault SOL:       ${(vaultBalance / 1e9).toFixed(4)} SOL`);
  if (vaultBalance < 50_000_000) { // 0.05 SOL minimum
    console.error("\n  ERROR: Squads vault needs at least 0.05 SOL for rent");
    console.error("  Send SOL to: " + vaultPda.toBase58());
    process.exit(1);
  }

  // Derive all vault targets and check existence
  console.log("\n--- Checking oracle positions ---\n");

  const targets: VaultTarget[] = [];
  for (const ch of CHANNELS) {
    const channelConfig = new PublicKey(ch.channelConfig);
    const [vault] = PublicKey.findProgramAddressSync(
      [VAULT_SEED, channelConfig.toBuffer()],
      VAULT_PROGRAM_ID,
    );
    const [oraclePosition] = PublicKey.findProgramAddressSync(
      [VAULT_ORACLE_POSITION_SEED, vault.toBuffer()],
      VAULT_PROGRAM_ID,
    );

    const info = await connection.getAccountInfo(oraclePosition);
    const exists = info !== null;

    targets.push({ name: ch.name, channelConfig, vault, oraclePosition, exists });

    if (exists) {
      console.log(`  SKIP  ${ch.name.padEnd(12)} oracle position exists`);
    } else {
      console.log(`  NEED  ${ch.name.padEnd(12)} ${oraclePosition.toBase58()}`);
    }

    await sleep(200); // rate limit
  }

  const pending = targets.filter((t) => !t.exists);
  if (pending.length === 0) {
    console.log("\n  All oracle positions already initialized. Nothing to do.");
    return;
  }

  console.log(`\n  ${pending.length} vaults need migration`);

  // Fetch multisig state
  console.log("\n--- Fetching multisig state ---\n");

  const multisigAccount = await multisig.accounts.Multisig.fromAccountAddress(
    connection,
    MULTISIG_PDA,
  );

  const baseIndex = Number(multisigAccount.transactionIndex);
  console.log(`  Threshold:     ${multisigAccount.threshold}`);
  console.log(`  Members:       ${multisigAccount.members.length}`);
  console.log(`  Last tx index: ${baseIndex}`);
  console.log(`  Will create:   proposals ${baseIndex + 1} through ${baseIndex + pending.length}`);

  // Verify keypairs are members
  for (const kp of keypairs) {
    const isMember = multisigAccount.members.some(
      (m: any) => m.key.toBase58() === kp.publicKey.toBase58(),
    );
    if (!isMember) {
      console.error(`\n  ERROR: ${kp.publicKey.toBase58()} is not a multisig member`);
      process.exit(1);
    }
  }

  // Create proposals
  console.log("\n--- Creating vault transactions ---\n");

  const feePayer = keypairs[0];
  const results: { name: string; txIndex: bigint; success: boolean; sig?: string }[] = [];

  for (let i = 0; i < pending.length; i++) {
    const target = pending[i];
    const txIndex = BigInt(baseIndex + 1 + i);

    console.log(`  [${i + 1}/${pending.length}] ${target.name} (proposal #${txIndex})`);

    try {
      // Build instruction
      const ix = migrateOraclePositionIx(vaultPda, target.vault, target.oraclePosition);

      const { blockhash } = await connection.getLatestBlockhash("confirmed");
      const message = new TransactionMessage({
        payerKey: vaultPda,
        recentBlockhash: blockhash,
        instructions: [ix],
      });

      // 1. Create vault transaction
      const vtSig = await multisig.rpc.vaultTransactionCreate({
        connection,
        feePayer,
        multisigPda: MULTISIG_PDA,
        transactionIndex: txIndex,
        creator: feePayer.publicKey,
        vaultIndex: 0,
        ephemeralSigners: 0,
        transactionMessage: message,
      });
      console.log(`    vault tx: ${vtSig.slice(0, 20)}...`);
      await sleep(500);

      // 2. Create proposal
      const proposalSig = await multisig.rpc.proposalCreate({
        connection,
        feePayer,
        creator: feePayer,
        multisigPda: MULTISIG_PDA,
        transactionIndex: txIndex,
        isDraft: false,
      });
      console.log(`    proposal: ${proposalSig.slice(0, 20)}...`);
      await sleep(500);

      // 3. Approve with both local keypairs
      for (const kp of keypairs) {
        const approveSig = await multisig.rpc.proposalApprove({
          connection,
          feePayer,
          member: kp,
          multisigPda: MULTISIG_PDA,
          transactionIndex: txIndex,
        });
        console.log(`    approved: ${kp.publicKey.toBase58().slice(0, 8)}...`);
        await sleep(500);
      }

      results.push({ name: target.name, txIndex, success: true, sig: vtSig });
    } catch (err: any) {
      console.error(`    ERROR: ${err.message}`);
      results.push({ name: target.name, txIndex, success: false });
    }
  }

  // Summary
  const startIdx = baseIndex + 1;
  const endIdx = baseIndex + pending.length;
  const succeeded = results.filter((r) => r.success).length;

  console.log("\n" + "=".repeat(60));
  console.log("  Migration Proposals Created");
  console.log("=".repeat(60));
  console.log(`\n  Created: ${succeeded} / ${pending.length} proposals`);
  console.log(`  Index range: ${startIdx} - ${endIdx}`);
  console.log(`  Approvals: 2 / ${multisigAccount.threshold} per proposal`);

  console.log("\n  Per-vault status:");
  for (const r of results) {
    console.log(`    ${r.name.padEnd(12)} #${r.txIndex.toString().padEnd(4)} ${r.success ? "OK" : "FAILED"}`);
  }

  console.log("\n  Next steps:");
  console.log("  1. Open app.squads.so -> approve all proposals with 3rd wallet");
  console.log("  2. Execute each proposal in Squads UI");
  console.log("     OR run:");
  console.log(`     RPC_URL="..." npx tsx scripts/admin/migrate-oracle-positions.ts --execute-all ${startIdx}`);
  console.log();
}

// ============================================================================
// Execute: run approved proposals
// ============================================================================

async function executeAll(
  connection: Connection,
  startIdx: number,
): Promise<void> {
  console.log("\n" + "=".repeat(60));
  console.log("  Execute Migration Proposals");
  console.log("=".repeat(60) + "\n");

  const feePayer = loadKeypair(KEYPAIR_PATHS[0]);
  console.log(`  Fee payer: ${feePayer.publicKey.toBase58().slice(0, 8)}...`);

  // Figure out how many proposals to execute
  const multisigAccount = await multisig.accounts.Multisig.fromAccountAddress(
    connection,
    MULTISIG_PDA,
  );
  const maxIndex = Number(multisigAccount.transactionIndex);

  if (startIdx > maxIndex) {
    console.error(`  ERROR: startIdx ${startIdx} > last tx index ${maxIndex}`);
    process.exit(1);
  }

  console.log(`  Executing proposals ${startIdx} through ${maxIndex}`);
  console.log();

  let executed = 0;
  let failed = 0;

  for (let idx = startIdx; idx <= maxIndex; idx++) {
    const txIndex = BigInt(idx);

    // Check proposal status
    const [proposalPda] = multisig.getProposalPda({
      multisigPda: MULTISIG_PDA,
      transactionIndex: txIndex,
    });

    try {
      const proposal = await multisig.accounts.Proposal.fromAccountAddress(
        connection,
        proposalPda,
      );

      const status = ((proposal.status as any).__kind || Object.keys(proposal.status)[0]).toLowerCase();
      if (status === "executed") {
        console.log(`  #${idx} already executed - skip`);
        continue;
      }
      if (status !== "approved") {
        console.log(`  #${idx} status: ${status} - skip (need 'approved')`);
        failed++;
        continue;
      }

      console.log(`  #${idx} executing...`);
      const sig = await multisig.rpc.vaultTransactionExecute({
        connection,
        feePayer,
        multisigPda: MULTISIG_PDA,
        transactionIndex: txIndex,
        member: feePayer.publicKey,
      });
      console.log(`    TX: ${sig}`);
      executed++;
      await sleep(1000);
    } catch (err: any) {
      console.error(`  #${idx} FAILED: ${err.message}`);
      failed++;
    }
  }

  console.log(`\n  Executed: ${executed}, Failed: ${failed}`);

  if (executed > 0) {
    console.log("\n  Verify migration:");
    console.log('  RPC_URL="..." npx tsx scripts/verify-mainnet-vaults.ts');
  }
  console.log();
}

// ============================================================================
// Main
// ============================================================================

async function main() {
  const rpcUrl = process.env.RPC_URL;
  if (!rpcUrl) {
    console.error("ERROR: RPC_URL required");
    process.exit(1);
  }

  const connection = new Connection(rpcUrl, "confirmed");
  const args = process.argv.slice(2);

  if (args[0] === "--execute-all") {
    const startIdx = parseInt(args[1], 10);
    if (isNaN(startIdx)) {
      console.error("Usage: --execute-all <startIdx>");
      process.exit(1);
    }
    await executeAll(connection, startIdx);
  } else {
    await createAndApprove(connection);
  }
}

main().catch((err) => {
  console.error("\nError:", err.message || err);
  if (err.logs) {
    console.error("\nProgram logs:");
    for (const log of err.logs) {
      console.error("  ", log);
    }
  }
  process.exit(1);
});
