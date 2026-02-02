/**
 * Set withdraw_queue_slots = 9,000 on all 16 Channel Vaults via Squads Multisig
 *
 * Creates one Squads vault transaction + proposal per vault, then approves each
 * with the 2 local member keypairs (2pHj + 87d5).  The 3rd approver can
 * batch-approve the remaining proposals in the Squads UI.
 *
 * Usage:
 *   RPC_URL="..." npx tsx scripts/admin/set-withdraw-queue.ts
 *
 * What this script does:
 *   1. Derives all 16 vault PDAs from the channel configs
 *   2. For each vault, builds an update_withdraw_queue_slots(9_000) instruction
 *   3. Wraps each in a Squads vault transaction
 *   4. Creates a proposal and approves with 2 local keypairs
 *   5. Prints a summary table of tx indices for the 3rd approver
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

/** Anchor discriminator for update_withdraw_queue_slots (first 8 bytes of SHA-256 of the IDL name) */
const UPDATE_WITHDRAW_QUEUE_DISCRIMINATOR = Buffer.from([
  87, 246, 25, 31, 33, 41, 70, 254,
]);

const NEW_WITHDRAW_QUEUE_SLOTS = 9_000;

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
 * Build the update_withdraw_queue_slots instruction.
 *
 * Accounts (from IDL):
 *   0. admin  — signer (the Squads vault PDA)
 *   1. vault  — writable (the ChannelVault PDA)
 *
 * Data: 8-byte discriminator + u64 LE new_withdraw_queue_slots
 */
function updateWithdrawQueueSlotsIx(
  admin: PublicKey,
  vault: PublicKey,
  newSlots: number,
): TransactionInstruction {
  const data = Buffer.alloc(8 + 8);
  UPDATE_WITHDRAW_QUEUE_DISCRIMINATOR.copy(data, 0);
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
      '  RPC_URL="https://..." npx tsx scripts/admin/set-withdraw-queue.ts',
    );
    process.exit(1);
  }

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
    `\n  Creating ${CHANNELS.length} proposals (withdraw_queue_slots = ${NEW_WITHDRAW_QUEUE_SLOTS.toLocaleString()})...\n`,
  );

  const results: { name: string; txIndex: bigint; vaultPda: string }[] = [];

  for (const channel of CHANNELS) {
    const channelConfig = new PublicKey(channel.channelConfig);
    const vaultPda = deriveVaultPda(channelConfig);

    currentIndex += 1;
    const txIndex = BigInt(currentIndex);

    console.log("=".repeat(60));
    console.log(`  ${channel.name}  (tx #${txIndex})`);
    console.log(`  Channel config: ${channel.channelConfig}`);
    console.log(`  Vault PDA:      ${vaultPda.toBase58()}`);
    console.log("=".repeat(60));

    // Build instruction
    const ix = updateWithdrawQueueSlotsIx(
      squadsVaultPda,
      vaultPda,
      NEW_WITHDRAW_QUEUE_SLOTS,
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
    });
    console.log();
  }

  // --- Summary ---

  console.log("\n" + "=".repeat(70));
  console.log("  SUMMARY - All proposals created");
  console.log("=".repeat(70));
  console.log(
    `\n  New value:  withdraw_queue_slots = ${NEW_WITHDRAW_QUEUE_SLOTS.toLocaleString()}`,
  );
  console.log(`  Approvals: 2 / ${multisigAccount.threshold} (need 3rd)`);
  console.log();

  console.log(
    "  " +
      "Tx#".padEnd(6) +
      "Vault".padEnd(22) +
      "Vault PDA",
  );
  console.log("  " + "-".repeat(66));
  for (const r of results) {
    console.log(
      "  " +
        r.txIndex.toString().padEnd(6) +
        r.name.padEnd(22) +
        r.vaultPda,
    );
  }

  const first = results[0].txIndex;
  const last = results[results.length - 1].txIndex;
  console.log(`\n  Next steps:`);
  console.log(
    `  1. Open app.squads.so -> batch-approve tx #${first} through #${last}`,
  );
  console.log(`  2. Execute each proposal (or batch-execute in Squads UI)`);
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
