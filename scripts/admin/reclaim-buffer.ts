/**
 * Reclaim Stale Oracle Buffer via Squads Multisig
 *
 * Closes the stale Oracle buffer AYUjSU9Q... and reclaims ~4.37 SOL
 * through a Squads v4 vault transaction (3-of-5 threshold).
 *
 * This script:
 *   1. Verifies the buffer exists and authority == Squads vault
 *   2. Creates a vault transaction wrapping BPF Loader Close
 *   3. Creates a proposal for the transaction
 *   4. Approves with 2 local member keypairs (2pHj + 87d5)
 *   -> User provides 3rd approval in Squads UI, then executes
 *
 * Usage:
 *   # Create proposal + approve with 2 local keys
 *   RPC_URL="..." npx ts-node scripts/admin/reclaim-buffer.ts
 *
 *   # Execute after 3rd approval in Squads UI
 *   RPC_URL="..." npx ts-node scripts/admin/reclaim-buffer.ts --execute <txIndex>
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

// ============================================================================
// Constants
// ============================================================================

const MULTISIG_PDA = new PublicKey("BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ");
const STALE_BUFFER = new PublicKey("AYUjSU9Qir4rmHWG55ZvFKfSNXb5LfJzrVdDewovVb1c");
const RECIPIENT = new PublicKey("2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD");
const BPF_LOADER_UPGRADEABLE = new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111");

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

/**
 * BPF Upgradeable Loader Close instruction (enum index 5).
 * Closes a buffer account and sends lamports to recipient.
 * Authority must be the buffer's upgrade authority.
 */
function closeBufferIx(
  buffer: PublicKey,
  recipient: PublicKey,
  authority: PublicKey,
): TransactionInstruction {
  return new TransactionInstruction({
    programId: BPF_LOADER_UPGRADEABLE,
    keys: [
      { pubkey: buffer,    isSigner: false, isWritable: true },
      { pubkey: recipient, isSigner: false, isWritable: true },
      { pubkey: authority, isSigner: true,  isWritable: false },
    ],
    // UpgradeableLoaderInstruction::Close = variant 5, bincode u32 LE
    data: Buffer.from([5, 0, 0, 0]),
  });
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

// ============================================================================
// Create: vault transaction + proposal + 2 approvals
// ============================================================================

async function createAndApprove(connection: Connection): Promise<void> {
  // Load keypairs
  console.log("\n" + "=".repeat(60));
  console.log("  Stale Buffer Reclaim - Squads Vault Transaction");
  console.log("=".repeat(60) + "\n");

  const keypairs = KEYPAIR_PATHS.map((p) => {
    const kp = loadKeypair(p);
    console.log(`  Loaded: ${kp.publicKey.toBase58().slice(0, 8)}... (${p.split("/").pop()})`);
    return kp;
  });

  // Derive vault PDA and verify it matches expected
  const [vaultPda] = multisig.getVaultPda({
    multisigPda: MULTISIG_PDA,
    index: 0,
  });
  console.log(`\n  Squads vault:  ${vaultPda.toBase58()}`);
  console.log(`  Buffer:        ${STALE_BUFFER.toBase58()}`);
  console.log(`  Recipient:     ${RECIPIENT.toBase58()}`);

  // -- Step 1: Verify buffer ------------------------------------------------
  console.log("\n--- Verifying buffer ---\n");

  const bufferInfo = await connection.getAccountInfo(STALE_BUFFER);
  if (!bufferInfo) {
    console.error("  Buffer account not found - already closed?");
    process.exit(1);
  }

  const lamports = bufferInfo.lamports;
  const owner = bufferInfo.owner.toBase58();
  console.log(`  Owner:    ${owner}`);
  console.log(`  Lamports: ${lamports} (${(lamports / 1e9).toFixed(4)} SOL)`);

  if (owner !== BPF_LOADER_UPGRADEABLE.toBase58()) {
    console.error(`  ERROR: Not owned by BPF Loader (owner: ${owner})`);
    process.exit(1);
  }

  // Parse buffer authority: [0..4] enum tag (1=Buffer), [4] Option<Pubkey> tag, [5..37] authority
  if (bufferInfo.data.length < 37) {
    console.error("  ERROR: Buffer data too short");
    process.exit(1);
  }
  const accountType = bufferInfo.data.readUInt32LE(0);
  if (accountType !== 1) {
    console.error(`  ERROR: Not a buffer account (type=${accountType})`);
    process.exit(1);
  }
  const hasAuthority = bufferInfo.data[4];
  if (hasAuthority !== 1) {
    console.error("  ERROR: Buffer has no authority set");
    process.exit(1);
  }
  const bufferAuthority = new PublicKey(bufferInfo.data.slice(5, 37));
  console.log(`  Authority: ${bufferAuthority.toBase58()}`);

  if (!bufferAuthority.equals(vaultPda)) {
    console.error(`  ERROR: Authority mismatch`);
    console.error(`    Buffer authority: ${bufferAuthority.toBase58()}`);
    console.error(`    Squads vault:    ${vaultPda.toBase58()}`);
    process.exit(1);
  }
  console.log("  OK: Buffer authority matches Squads vault\n");

  // -- Step 2: Fetch multisig state -----------------------------------------
  console.log("--- Fetching multisig state ---\n");

  const multisigAccount = await multisig.accounts.Multisig.fromAccountAddress(
    connection,
    MULTISIG_PDA,
  );

  const currentIndex = Number(multisigAccount.transactionIndex);
  const txIndex = BigInt(currentIndex + 1);

  console.log(`  Threshold:     ${multisigAccount.threshold}`);
  console.log(`  Members:       ${multisigAccount.members.length}`);
  console.log(`  Last tx index: ${currentIndex}`);
  console.log(`  New tx index:  ${txIndex}`);

  // Verify keypairs are members
  for (const kp of keypairs) {
    const isMember = multisigAccount.members.some(
      (m: any) => m.key.toBase58() === kp.publicKey.toBase58(),
    );
    if (!isMember) {
      console.error(`\n  ERROR: ${kp.publicKey.toBase58()} is not a multisig member`);
      process.exit(1);
    }
    console.log(`  OK: ${kp.publicKey.toBase58().slice(0, 8)}... is a member`);
  }

  // -- Step 3: Create vault transaction --------------------------------------
  console.log("\n--- Creating vault transaction ---\n");

  const ix = closeBufferIx(STALE_BUFFER, RECIPIENT, vaultPda);

  const { blockhash } = await connection.getLatestBlockhash("confirmed");
  const message = new TransactionMessage({
    payerKey: vaultPda,
    recentBlockhash: blockhash,
    instructions: [ix],
  });

  const feePayer = keypairs[0];

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

  // -- Step 4: Create proposal -----------------------------------------------
  console.log("\n--- Creating proposal ---\n");

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

  // -- Step 5: Approve with both local keypairs ------------------------------
  console.log("\n--- Approving with local keypairs ---\n");

  for (const kp of keypairs) {
    const approveSig = await multisig.rpc.proposalApprove({
      connection,
      feePayer,
      member: kp,
      multisigPda: MULTISIG_PDA,
      transactionIndex: txIndex,
    });

    console.log(`  Approved by ${kp.publicKey.toBase58().slice(0, 8)}...: ${approveSig}`);
    await sleep(1000);
  }

  // -- Summary ---------------------------------------------------------------
  const [txPda] = multisig.getTransactionPda({
    multisigPda: MULTISIG_PDA,
    index: txIndex,
  });
  const [proposalPda] = multisig.getProposalPda({
    multisigPda: MULTISIG_PDA,
    transactionIndex: txIndex,
  });

  console.log("\n" + "=".repeat(60));
  console.log("  Done - 2 of 3 approvals submitted");
  console.log("=".repeat(60));
  console.log(`\n  Transaction PDA: ${txPda.toBase58()}`);
  console.log(`  Proposal PDA:    ${proposalPda.toBase58()}`);
  console.log(`  Tx Index:        ${txIndex}`);
  console.log(`  Approvals:       2 / ${multisigAccount.threshold}`);
  console.log(`  SOL to reclaim:  ${(lamports / 1e9).toFixed(4)} SOL`);
  console.log(`  Refund to:       ${RECIPIENT.toBase58()}`);
  console.log(`\n  Next steps:`);
  console.log(`  1. Open app.squads.so -> this multisig`);
  console.log(`  2. Find the proposal and approve with your 3rd wallet`);
  console.log(`  3. Execute the transaction`);
  console.log(`\n  Or execute here after 3rd approval:`);
  console.log(`  RPC_URL="..." npx ts-node scripts/admin/reclaim-buffer.ts --execute ${txIndex}`);
  console.log();
}

// ============================================================================
// Execute: run an already-approved vault transaction
// ============================================================================

async function executeTransaction(
  connection: Connection,
  txIndex: bigint,
): Promise<void> {
  console.log("\n" + "=".repeat(60));
  console.log("  Execute Vault Transaction");
  console.log("=".repeat(60) + "\n");

  const feePayer = loadKeypair(KEYPAIR_PATHS[0]);
  console.log(`  Fee payer: ${feePayer.publicKey.toBase58().slice(0, 8)}...`);
  console.log(`  Tx Index:  ${txIndex}\n`);

  // Check proposal status
  const [proposalPda] = multisig.getProposalPda({
    multisigPda: MULTISIG_PDA,
    transactionIndex: txIndex,
  });

  const proposal = await multisig.accounts.Proposal.fromAccountAddress(
    connection,
    proposalPda,
  );

  const status = Object.keys(proposal.status)[0];
  console.log(`  Proposal status: ${status}`);
  console.log(`  Approved by:     ${proposal.approved.length} members`);

  if (status !== "approved") {
    console.error(`\n  ERROR: Proposal is '${status}', need 'approved' to execute`);
    process.exit(1);
  }

  console.log("\n--- Executing ---\n");

  const execSig = await multisig.rpc.vaultTransactionExecute({
    connection,
    feePayer,
    multisigPda: MULTISIG_PDA,
    transactionIndex: txIndex,
    member: feePayer.publicKey,
  });

  console.log(`  Executed: ${execSig}`);

  // Verify buffer is closed
  await sleep(2000);
  const bufferInfo = await connection.getAccountInfo(STALE_BUFFER);
  if (bufferInfo === null) {
    console.log("\n  Buffer closed successfully!");
  } else {
    console.log(`\n  WARNING: Buffer still exists (${bufferInfo.lamports} lamports)`);
  }

  // Check recipient balance
  const recipientBalance = await connection.getBalance(RECIPIENT);
  console.log(`  Recipient balance: ${(recipientBalance / 1e9).toFixed(4)} SOL`);
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

  if (args[0] === "--execute") {
    const idx = args[1];
    if (!idx) {
      console.error("Usage: --execute <txIndex>");
      process.exit(1);
    }
    await executeTransaction(connection, BigInt(idx));
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
