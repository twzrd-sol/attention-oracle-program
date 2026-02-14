/**
 * squads-set-cutover-epochs.ts
 *
 * Batch-creates Squads V4 proposals to call update_channel_cutover_epoch on
 * all configured channels. Auto-approves with the 2 local member keypairs;
 * the 3rd approver batch-approves in the Squads UI (or CLI) to reach 3/5.
 *
 * Follows the same pattern as set-withdraw-queue.ts.
 *
 * Usage:
 *   RPC_URL="..." CUTOVER_EPOCH=750 npx tsx scripts/admin/squads-set-cutover-epochs.ts
 *
 * What this script does:
 *   1. Reads the current on-chain cutover_epoch for each channel
 *   2. Skips channels already set to the target epoch
 *   3. For each remaining channel, builds an update_channel_cutover_epoch IX
 *   4. Wraps each in a Squads vault transaction + proposal
 *   5. Approves with 2 local keypairs (need 3rd for 3/5 threshold)
 *   6. Prints a summary table of tx indices for the 3rd approver
 */

import {
  Connection,
  Keypair,
  PublicKey,
  TransactionMessage,
  TransactionInstruction,
} from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import * as multisig from "@sqds/multisig";
import * as fs from "fs";
import { CHANNELS, oracleChannelName } from "../keepers/lib/channels.js";

// ============================================================================
// Constants
// ============================================================================

const ORACLE_PROGRAM_ID = new PublicKey(
  "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop",
);

const CCM_MINT = new PublicKey(
  "Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM",
);

const MULTISIG_PDA = new PublicKey(
  "BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ",
);

/** Anchor discriminator: SHA-256("global:update_channel_cutover_epoch")[0..8] */
const UPDATE_CUTOVER_EPOCH_DISCRIMINATOR = Buffer.from([
  7, 13, 36, 63, 172, 39, 59, 241,
]);

const PROTOCOL_SEED = Buffer.from("protocol");

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

/**
 * Build the update_channel_cutover_epoch instruction.
 *
 * Accounts (from Anchor IDL):
 *   0. admin          — signer (Squads vault PDA)
 *   1. protocol_state — seeds=[b"protocol", mint]
 *   2. channel_config  — writable (the ChannelConfigV2 PDA)
 *
 * Data: 8-byte discriminator + Borsh String (channel) + u64 LE (new_cutover_epoch)
 */
function updateCutoverEpochIx(
  admin: PublicKey,
  protocolState: PublicKey,
  channelConfig: PublicKey,
  channelName: string,
  newCutoverEpoch: bigint,
): TransactionInstruction {
  // Borsh String = 4-byte LE length + UTF-8 bytes
  const channelBytes = Buffer.from(channelName, "utf-8");
  const data = Buffer.alloc(8 + 4 + channelBytes.length + 8);

  let offset = 0;
  UPDATE_CUTOVER_EPOCH_DISCRIMINATOR.copy(data, offset);
  offset += 8;
  data.writeUInt32LE(channelBytes.length, offset);
  offset += 4;
  channelBytes.copy(data, offset);
  offset += channelBytes.length;
  data.writeBigUInt64LE(newCutoverEpoch, offset);

  return new TransactionInstruction({
    programId: ORACLE_PROGRAM_ID,
    keys: [
      { pubkey: admin, isSigner: true, isWritable: true },
      { pubkey: protocolState, isSigner: false, isWritable: false },
      { pubkey: channelConfig, isSigner: false, isWritable: true },
    ],
    data,
  });
}

// ============================================================================
// Main
// ============================================================================

async function main() {
  const rpcUrl = process.env.RPC_URL;
  if (!rpcUrl) {
    console.error("ERROR: Set RPC_URL environment variable");
    console.error(
      '  RPC_URL="..." CUTOVER_EPOCH=750 npx tsx scripts/admin/squads-set-cutover-epochs.ts',
    );
    process.exit(1);
  }

  const cutoverEpochStr = process.env.CUTOVER_EPOCH;
  if (!cutoverEpochStr) {
    console.error("ERROR: Set CUTOVER_EPOCH (e.g. 750)");
    process.exit(1);
  }
  const cutoverEpoch = BigInt(cutoverEpochStr);

  const connection = new Connection(rpcUrl, "confirmed");

  // Fetch current epoch for context
  const epochInfo = await connection.getEpochInfo();
  const currentEpoch = epochInfo.epoch;

  // --- Load keypairs ---

  const keypairs = KEYPAIR_PATHS.map((p) => {
    const kp = loadKeypair(p);
    console.log(
      `  Loaded: ${kp.publicKey.toBase58().slice(0, 8)}... (${p.split("/").pop()})`,
    );
    return kp;
  });
  const feePayer = keypairs[0];

  // --- Derive Squads vault PDA (the AO protocol admin) ---

  const [squadsVaultPda] = multisig.getVaultPda({
    multisigPda: MULTISIG_PDA,
    index: 0,
  });
  console.log(`\n  Squads vault PDA: ${squadsVaultPda.toBase58()}`);

  // --- Derive protocol state PDA ---

  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, CCM_MINT.toBuffer()],
    ORACLE_PROGRAM_ID,
  );
  console.log(`  Protocol state:   ${protocolState.toBase58()}`);

  // --- Fetch multisig state ---

  const multisigAccount = await multisig.accounts.Multisig.fromAccountAddress(
    connection,
    MULTISIG_PDA,
  );

  let currentIndex = Number(multisigAccount.transactionIndex);
  console.log(`  Threshold:        ${multisigAccount.threshold}`);
  console.log(`  Members:          ${multisigAccount.members.length}`);
  console.log(`  Last tx index:    ${currentIndex}`);
  console.log(`\n  Current epoch:    ${currentEpoch}`);
  console.log(`  Target cutover:   ${cutoverEpoch}`);

  if (cutoverEpoch > 0n && cutoverEpoch <= BigInt(currentEpoch)) {
    console.log(`  WARNING: target <= current epoch, V2 claims disabled immediately`);
  }

  // Verify keypairs are members
  for (const kp of keypairs) {
    const isMember = multisigAccount.members.some(
      (m: any) => m.key.toBase58() === kp.publicKey.toBase58(),
    );
    if (!isMember) {
      console.error(
        `\n  ERROR: ${kp.publicKey.toBase58()} is not a multisig member`,
      );
      process.exit(1);
    }
    console.log(
      `  OK: ${kp.publicKey.toBase58().slice(0, 8)}... is a member`,
    );
  }

  // --- Pre-read current cutover_epoch for each channel ---

  const wallet = new anchor.Wallet(Keypair.generate());
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed" });
  const oracleIdl = await Program.fetchIdl(ORACLE_PROGRAM_ID, provider);
  if (!oracleIdl) throw new Error("Failed to fetch Oracle IDL");
  const oracleProgram = new Program(oracleIdl, provider);

  // --- Process each channel ---

  console.log(
    `\n  Creating proposals (cutover_epoch = ${cutoverEpoch})...\n`,
  );

  const results: { name: string; txIndex: bigint; configPda: string; oldEpoch: number }[] = [];
  let skipped = 0;

  for (const channel of CHANNELS) {
    const channelConfig = new PublicKey(channel.channelConfig);

    // Read current cutover_epoch
    let currentCutover = 0;
    try {
      const cfg: any = await oracleProgram.account.channelConfigV2.fetch(channelConfig);
      currentCutover = Number(cfg.cutoverEpoch);
    } catch (err: any) {
      console.log(`  SKIP ${channel.name}: failed to read config (${err.message})`);
      skipped++;
      continue;
    }

    if (BigInt(currentCutover) === cutoverEpoch) {
      console.log(`  SKIP ${channel.name}: already set to ${cutoverEpoch}`);
      skipped++;
      continue;
    }

    currentIndex += 1;
    const txIndex = BigInt(currentIndex);

    console.log("=".repeat(60));
    console.log(`  ${channel.name}  (tx #${txIndex})`);
    console.log(`  Channel config:   ${channel.channelConfig}`);
    console.log(`  Current cutover:  ${currentCutover}`);
    console.log(`  New cutover:      ${cutoverEpoch}`);
    console.log("=".repeat(60));

    // Build instruction
    const ix = updateCutoverEpochIx(
      squadsVaultPda,
      protocolState,
      channelConfig,
      oracleChannelName(channel),
      cutoverEpoch,
    );

    // Wrap in Squads vault transaction
    const { blockhash } = await connection.getLatestBlockhash("confirmed");
    const message = new TransactionMessage({
      payerKey: squadsVaultPda,
      recentBlockhash: blockhash,
      instructions: [ix],
    });

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
    console.log(`  Vault tx created: ${vtSig}`);
    await sleep(1000);

    // Create proposal
    const proposalSig = await multisig.rpc.proposalCreate({
      connection,
      feePayer,
      creator: feePayer,
      multisigPda: MULTISIG_PDA,
      transactionIndex: txIndex,
      isDraft: false,
    });
    console.log(`  Proposal created: ${proposalSig}`);
    await sleep(1000);

    // Approve with both local keypairs
    for (const kp of keypairs) {
      const approveSig = await multisig.rpc.proposalApprove({
        connection,
        feePayer,
        member: kp,
        multisigPda: MULTISIG_PDA,
        transactionIndex: txIndex,
      });
      console.log(
        `  Approved by ${kp.publicKey.toBase58().slice(0, 8)}...: ${approveSig}`,
      );
      await sleep(1000);
    }

    results.push({
      name: channel.name,
      txIndex,
      configPda: channel.channelConfig,
      oldEpoch: currentCutover,
    });
    console.log();
  }

  // --- Summary ---

  console.log("\n" + "=".repeat(70));
  console.log("  SUMMARY - Cutover epoch proposals");
  console.log("=".repeat(70));
  console.log(`\n  Target:   cutover_epoch = ${cutoverEpoch}`);
  console.log(`  Created:  ${results.length} proposals`);
  console.log(`  Skipped:  ${skipped} (already set or fetch failed)`);
  console.log(`  Approvals: 2 / ${multisigAccount.threshold} (need 3rd)`);
  console.log();

  if (results.length > 0) {
    console.log(
      "  " +
        "Tx#".padEnd(6) +
        "Channel".padEnd(22) +
        "Old".padEnd(8) +
        "New",
    );
    console.log("  " + "-".repeat(50));
    for (const r of results) {
      console.log(
        "  " +
          r.txIndex.toString().padEnd(6) +
          r.name.padEnd(22) +
          r.oldEpoch.toString().padEnd(8) +
          cutoverEpoch.toString(),
      );
    }

    const first = results[0].txIndex;
    const last = results[results.length - 1].txIndex;
    console.log(`\n  Next steps:`);
    console.log(
      `  1. Open app.squads.so -> batch-approve tx #${first} through #${last}`,
    );
    console.log(`  2. Execute each proposal (or batch-execute in Squads UI)`);
    console.log(`  3. Run check-cutover-epochs.ts to verify on-chain state`);
  } else {
    console.log("  No proposals created (all channels already at target epoch).");
  }
  console.log();
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
