/**
 * Set lock_duration_slots = 27,000 (~3h) on all Channel Vaults via Squads Multisig
 *
 * Creates one Squads vault transaction + proposal per vault, then approves each
 * with the 2 local member keypairs (2pHj + 87d5).  The 3rd approver can
 * batch-approve the remaining proposals in the Squads UI.
 *
 * REQUIRES: Program upgrade that adds update_lock_duration_slots instruction.
 *           Run this AFTER the upgraded program is deployed and verified.
 *
 * Usage:
 *   RPC_URL="..." npx tsx scripts/admin/set-lock-duration.ts
 *
 * Override target value:
 *   RPC_URL="..." LOCK_DURATION_SLOTS=9000 npx tsx scripts/admin/set-lock-duration.ts
 *
 * What this script does:
 *   1. Derives all vault PDAs from the channel configs
 *   2. Reads current lock_duration_slots from each vault (skips if already at target)
 *   3. For each vault, builds an update_lock_duration_slots(N) instruction
 *   4. Wraps each in a Squads vault transaction
 *   5. Creates a proposal and approves with 2 local keypairs
 *   6. Prints a summary table of tx indices for the 3rd approver
 */

import {
  Connection,
  Keypair,
  PublicKey,
  TransactionMessage,
  TransactionInstruction,
} from "@solana/web3.js";
import * as multisig from "@sqds/multisig";
import * as fs from "fs";
import { CHANNELS } from "../keepers/lib/channels.js";

// ============================================================================
// Constants
// ============================================================================

const VAULT_PROGRAM_ID = new PublicKey(
  "5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ",
);

const MULTISIG_PDA = new PublicKey(
  "BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ",
);

/** Anchor discriminator for update_lock_duration_slots
 *  SHA-256("global:update_lock_duration_slots")[0..8] */
const UPDATE_LOCK_DURATION_DISCRIMINATOR = Buffer.from([
  177, 182, 244, 233, 199, 49, 44, 67,
]);

/** Default: 27,000 slots ≈ 3 hours at 400ms/slot */
const DEFAULT_LOCK_DURATION_SLOTS = 27_000;

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
 * Derive the ChannelVault PDA from a channel config pubkey.
 * Seeds: ["vault", channel_config_pubkey]
 */
function deriveVaultPda(channelConfig: PublicKey): PublicKey {
  const [pda] = PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), channelConfig.toBuffer()],
    VAULT_PROGRAM_ID,
  );
  return pda;
}

/**
 * Read current lock_duration_slots from the vault account.
 *
 * Layout (from state.rs ChannelVault):
 *   offset 0:   8 bytes discriminator
 *   offset 8:   1 byte  bump
 *   offset 9:   1 byte  version
 *   offset 10:  32 bytes channel_config
 *   offset 42:  32 bytes ccm_mint
 *   offset 74:  32 bytes vlofi_mint
 *   offset 106: 32 bytes ccm_buffer
 *   offset 138: 8 bytes  total_staked
 *   offset 146: 8 bytes  total_shares
 *   offset 154: 8 bytes  pending_deposits
 *   offset 162: 8 bytes  pending_withdrawals
 *   offset 170: 8 bytes  last_compound_slot
 *   offset 178: 8 bytes  compound_count
 *   offset 186: 32 bytes admin
 *   offset 218: 8 bytes  min_deposit
 *   offset 226: 1 byte   paused
 *   offset 227: 8 bytes  emergency_reserve
 *   offset 235: 8 bytes  lock_duration_slots  <-- THIS
 *   offset 243: 8 bytes  withdraw_queue_slots
 */
const LOCK_DURATION_OFFSET = 8 + 1 + 1 + 32 + 32 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8 + 32 + 8 + 1 + 8;
// = 235

async function readLockDurationSlots(
  connection: Connection,
  vaultPda: PublicKey,
): Promise<number | null> {
  const accountInfo = await connection.getAccountInfo(vaultPda);
  if (!accountInfo || !accountInfo.data) return null;

  const data = accountInfo.data;
  if (data.length < LOCK_DURATION_OFFSET + 8) return null;

  return Number(data.readBigUInt64LE(LOCK_DURATION_OFFSET));
}

/**
 * Build the update_lock_duration_slots instruction.
 *
 * Accounts (from AdminAction context):
 *   0. admin  — signer (the Squads vault PDA)
 *   1. vault  — writable (the ChannelVault PDA)
 *
 * Data: 8-byte discriminator + u64 LE new_lock_duration_slots
 */
function updateLockDurationSlotsIx(
  admin: PublicKey,
  vault: PublicKey,
  newSlots: number,
): TransactionInstruction {
  const data = Buffer.alloc(8 + 8);
  UPDATE_LOCK_DURATION_DISCRIMINATOR.copy(data, 0);
  data.writeBigUInt64LE(BigInt(newSlots), 8);

  return new TransactionInstruction({
    programId: VAULT_PROGRAM_ID,
    keys: [
      { pubkey: admin, isSigner: true, isWritable: false },
      { pubkey: vault, isSigner: false, isWritable: true },
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
      '  RPC_URL="..." npx tsx scripts/admin/set-lock-duration.ts',
    );
    process.exit(1);
  }

  const newLockDuration = process.env.LOCK_DURATION_SLOTS
    ? Number(process.env.LOCK_DURATION_SLOTS)
    : DEFAULT_LOCK_DURATION_SLOTS;

  if (!Number.isSafeInteger(newLockDuration) || newLockDuration < 0) {
    console.error("ERROR: LOCK_DURATION_SLOTS must be a non-negative integer");
    process.exit(1);
  }

  const approxHours = ((newLockDuration * 0.4) / 3600).toFixed(1);
  console.log(`\n  Target: lock_duration_slots = ${newLockDuration.toLocaleString()} (~${approxHours}h)\n`);

  const connection = new Connection(rpcUrl, "confirmed");

  // --- Load keypairs ---

  const keypairs = KEYPAIR_PATHS.map((p) => {
    const kp = loadKeypair(p);
    console.log(
      `  Loaded: ${kp.publicKey.toBase58().slice(0, 8)}... (${p.split("/").pop()})`,
    );
    return kp;
  });
  const feePayer = keypairs[0];

  // --- Derive Squads vault PDA (the vault admin) ---

  const [squadsVaultPda] = multisig.getVaultPda({
    multisigPda: MULTISIG_PDA,
    index: 0,
  });
  console.log(`\n  Squads vault PDA: ${squadsVaultPda.toBase58()}`);

  // --- Fetch multisig state ---

  const multisigAccount = await multisig.accounts.Multisig.fromAccountAddress(
    connection,
    MULTISIG_PDA,
  );

  let currentIndex = Number(multisigAccount.transactionIndex);
  console.log(`  Threshold:        ${multisigAccount.threshold}`);
  console.log(`  Members:          ${multisigAccount.members.length}`);
  console.log(`  Last tx index:    ${currentIndex}`);

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

  // --- Process each vault ---

  console.log(
    `\n  Creating proposals (lock_duration_slots = ${newLockDuration.toLocaleString()})...\n`,
  );

  const results: { name: string; txIndex: bigint; vaultPda: string; oldValue: number }[] = [];
  let skipped = 0;

  for (const channel of CHANNELS) {
    const channelConfig = new PublicKey(channel.channelConfig);
    const vaultPda = deriveVaultPda(channelConfig);

    // Read current value (skip if already at target)
    const currentLockDuration = await readLockDurationSlots(connection, vaultPda);
    if (currentLockDuration === null) {
      console.log(`  SKIP ${channel.name}: vault account not found`);
      skipped++;
      continue;
    }
    if (currentLockDuration === newLockDuration) {
      console.log(`  SKIP ${channel.name}: already set to ${newLockDuration}`);
      skipped++;
      continue;
    }

    currentIndex += 1;
    const txIndex = BigInt(currentIndex);

    const oldHours = ((currentLockDuration * 0.4) / 3600).toFixed(1);
    console.log("=".repeat(60));
    console.log(`  ${channel.name}  (tx #${txIndex})`);
    console.log(`  Channel config: ${channel.channelConfig}`);
    console.log(`  Vault PDA:      ${vaultPda.toBase58()}`);
    console.log(`  Current lock:   ${currentLockDuration.toLocaleString()} (~${oldHours}h)`);
    console.log(`  New lock:       ${newLockDuration.toLocaleString()} (~${approxHours}h)`);
    console.log("=".repeat(60));

    // Build instruction
    const ix = updateLockDurationSlotsIx(
      squadsVaultPda,
      vaultPda,
      newLockDuration,
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
      vaultPda: vaultPda.toBase58(),
      oldValue: currentLockDuration,
    });
    console.log();
  }

  // --- Summary ---

  console.log("\n" + "=".repeat(70));
  console.log("  SUMMARY - Lock duration proposals");
  console.log("=".repeat(70));
  console.log(
    `\n  Target:   lock_duration_slots = ${newLockDuration.toLocaleString()} (~${approxHours}h)`,
  );
  console.log(`  Created:  ${results.length} proposals`);
  console.log(`  Skipped:  ${skipped} (already set or not found)`);
  console.log(`  Approvals: 2 / ${multisigAccount.threshold} (need 3rd)`);
  console.log();

  if (results.length > 0) {
    console.log(
      "  " +
        "Tx#".padEnd(6) +
        "Vault".padEnd(22) +
        "Old".padEnd(12) +
        "New",
    );
    console.log("  " + "-".repeat(54));
    for (const r of results) {
      console.log(
        "  " +
          r.txIndex.toString().padEnd(6) +
          r.name.padEnd(22) +
          r.oldValue.toLocaleString().padEnd(12) +
          newLockDuration.toLocaleString(),
      );
    }

    const first = results[0].txIndex;
    const last = results[results.length - 1].txIndex;
    console.log(`\n  Next steps:`);
    console.log(
      `  1. Open app.squads.so -> batch-approve tx #${first} through #${last}`,
    );
    console.log(`  2. Execute each proposal (or batch-execute in Squads UI)`);
  } else {
    console.log("  No proposals created (all vaults already at target).");
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
