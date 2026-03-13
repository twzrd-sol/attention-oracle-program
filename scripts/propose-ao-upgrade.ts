/**
 * Create a Squads V4 proposal to upgrade the attention-oracle program (GnGz...)
 * with the smaller Phase-1-only binary.
 *
 * Auto-approves with 2 local keypairs (2pHj + 87d5). Needs 1 more in Squads UI.
 *
 * Usage:
 *   RPC_URL="https://..." npx ts-node scripts/propose-ao-upgrade.ts
 */

import * as multisig from "@sqds/multisig";
import {
  Connection,
  Keypair,
  PublicKey,
  SYSVAR_CLOCK_PUBKEY,
  SYSVAR_RENT_PUBKEY,
  TransactionInstruction,
  TransactionMessage,
} from "@solana/web3.js";
import * as fs from "fs";

// ============================================================================
// Constants
// ============================================================================

const MULTISIG_PDA = new PublicKey("BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ");
const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const PROGRAM_DATA = new PublicKey("5GyaaVmzRr2r9KcUuzt9SxBVq9ubTT5m3pH9Lzy3Kh4L");
const BUFFER = new PublicKey("8T5qmmVAtUMEd7aFgV9DmtPKWVeZ32pZJzUFudXfHh6i");
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

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
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

  console.log("\n" + "=".repeat(60));
  console.log("  AO Program Upgrade - Squads Vault Transaction");
  console.log("=".repeat(60) + "\n");

  // Load keypairs
  const keypairs = KEYPAIR_PATHS.map((p) => {
    const kp = loadKeypair(p);
    console.log(`  Loaded: ${kp.publicKey.toBase58().slice(0, 8)}... (${p.split("/").pop()})`);
    return kp;
  });

  // Derive vault PDA
  const [vaultPda] = multisig.getVaultPda({
    multisigPda: MULTISIG_PDA,
    index: 0,
  });

  console.log(`\n  Squads vault: ${vaultPda.toBase58()}`);
  console.log(`  Program:      ${PROGRAM_ID.toBase58()}`);
  console.log(`  ProgramData:  ${PROGRAM_DATA.toBase58()}`);
  console.log(`  Buffer:       ${BUFFER.toBase58()}`);

  // Verify buffer exists
  const bufferInfo = await connection.getAccountInfo(BUFFER);
  if (!bufferInfo) {
    console.error("\n  ERROR: Buffer account not found");
    process.exit(1);
  }
  console.log(`  Buffer size:  ${bufferInfo.data.length} bytes`);
  console.log(`  Buffer rent:  ${(bufferInfo.lamports / 1e9).toFixed(4)} SOL`);

  // Verify buffer authority matches vault
  if (bufferInfo.data.length >= 37) {
    const hasAuth = bufferInfo.data[4];
    if (hasAuth === 1) {
      const auth = new PublicKey(bufferInfo.data.slice(5, 37));
      console.log(`  Buffer auth:  ${auth.toBase58()}`);
      if (!auth.equals(vaultPda)) {
        console.error(`\n  ERROR: Buffer authority mismatch (expected vault ${vaultPda.toBase58()})`);
        process.exit(1);
      }
      console.log("  OK: Buffer authority matches vault");
    }
  }

  // Get multisig state
  const multisigAccount = await multisig.accounts.Multisig.fromAccountAddress(
    connection,
    MULTISIG_PDA,
  );

  const currentIndex = Number(multisigAccount.transactionIndex);
  const txIndex = BigInt(currentIndex + 1);

  console.log(`\n  Threshold:     ${multisigAccount.threshold}`);
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
  }

  // Build BPF Loader Upgrade instruction
  const feePayer = keypairs[0];

  const upgradeIx = new TransactionInstruction({
    programId: BPF_LOADER_UPGRADEABLE,
    keys: [
      { pubkey: PROGRAM_DATA, isSigner: false, isWritable: true },
      { pubkey: PROGRAM_ID, isSigner: false, isWritable: true },
      { pubkey: BUFFER, isSigner: false, isWritable: true },
      { pubkey: feePayer.publicKey, isSigner: false, isWritable: true }, // spill (receives leftover SOL)
      { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: vaultPda, isSigner: true, isWritable: false }, // upgrade authority
    ],
    data: Buffer.from([3, 0, 0, 0]), // Upgrade = variant 3 (bincode u32 LE)
  });

  // Create vault transaction
  console.log("\n--- Creating vault transaction ---\n");

  const { blockhash } = await connection.getLatestBlockhash("confirmed");
  const message = new TransactionMessage({
    payerKey: vaultPda,
    recentBlockhash: blockhash,
    instructions: [upgradeIx],
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
  await sleep(1500);

  // Create proposal
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
  await sleep(1500);

  // Approve with both local keypairs
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
    await sleep(1500);
  }

  // Summary
  const [txPda] = multisig.getTransactionPda({
    multisigPda: MULTISIG_PDA,
    index: txIndex,
  });

  console.log("\n" + "=".repeat(60));
  console.log("  Done - 2 of 3 approvals submitted");
  console.log("=".repeat(60));
  console.log(`\n  Transaction PDA: ${txPda.toBase58()}`);
  console.log(`  Tx Index:        ${txIndex}`);
  console.log(`  Program:         ${PROGRAM_ID.toBase58()}`);
  console.log(`  Buffer:          ${BUFFER.toBase58()}`);
  console.log(`\n  Next steps:`);
  console.log(`  1. Open app.squads.so -> multisig ${MULTISIG_PDA.toBase58()}`);
  console.log(`  2. Find the upgrade proposal and approve with your 3rd wallet`);
  console.log(`  3. Execute the transaction`);
  console.log(`\n  After execution, verify:`);
  console.log(`  solana program show ${PROGRAM_ID.toBase58()} --url mainnet-beta`);
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
