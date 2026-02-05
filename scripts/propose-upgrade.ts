/**
 * Create a Squads V4 proposal to upgrade the attention-oracle program
 *
 * Usage:
 *   npx ts-node scripts/propose-upgrade.ts
 */

import * as multisig from "@sqds/multisig";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import * as fs from "fs";

// Configuration
const RPC_URL = "https://api.mainnet-beta.solana.com";
const MULTISIG_PUBKEY = new PublicKey("BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ");
const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const BUFFER_PUBKEY = new PublicKey("4XP54FJrgabhvNxE8bTpxqcJre5PSRzfniTPr2aBM6g8");
const PROGRAM_DATA_PUBKEY = new PublicKey("5GyaaVmzRr2r9KcUuzt9SxBVq9ubTT5m3pH9Lzy3Kh4L");
const BPF_LOADER_UPGRADEABLE = new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111");

async function main() {
  // Load keypair
  const keypairPath = process.env.SOLANA_KEYPAIR || `${process.env.HOME}/.config/solana/id.json`;
  const keypairData = JSON.parse(fs.readFileSync(keypairPath, "utf-8"));
  const payer = Keypair.fromSecretKey(new Uint8Array(keypairData));

  console.log("=== Squads V4 Program Upgrade Proposal ===");
  console.log("Proposer:", payer.publicKey.toBase58());
  console.log("Multisig:", MULTISIG_PUBKEY.toBase58());
  console.log("Program:", PROGRAM_ID.toBase58());
  console.log("Buffer:", BUFFER_PUBKEY.toBase58());

  const connection = new Connection(RPC_URL, "confirmed");

  // Get vault PDA (authority for program upgrades)
  const [vaultPda] = multisig.getVaultPda({
    multisigPda: MULTISIG_PUBKEY,
    index: 0,
  });
  console.log("Vault PDA:", vaultPda.toBase58());

  // Get multisig account to find next transaction index
  const multisigAccount = await multisig.accounts.Multisig.fromAccountAddress(
    connection,
    MULTISIG_PUBKEY
  );
  const transactionIndex = BigInt(multisigAccount.transactionIndex) + 1n;
  console.log("Transaction Index:", transactionIndex.toString());

  // Create BPF Loader Upgrade instruction
  // Instruction layout:
  // - [0]: u32 = 3 (Upgrade instruction discriminator)
  const upgradeIx = new TransactionInstruction({
    programId: BPF_LOADER_UPGRADEABLE,
    keys: [
      { pubkey: PROGRAM_DATA_PUBKEY, isSigner: false, isWritable: true },
      { pubkey: PROGRAM_ID, isSigner: false, isWritable: true },
      { pubkey: BUFFER_PUBKEY, isSigner: false, isWritable: true },
      { pubkey: payer.publicKey, isSigner: false, isWritable: true }, // spill account (receives leftover SOL)
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: vaultPda, isSigner: true, isWritable: false }, // upgrade authority
    ],
    data: Buffer.from([3, 0, 0, 0]), // Upgrade instruction discriminator (little-endian u32)
  });

  // Create the vault transaction
  const [transactionPda] = multisig.getTransactionPda({
    multisigPda: MULTISIG_PUBKEY,
    index: transactionIndex,
  });

  // Get recent blockhash for the transaction message
  const { blockhash } = await connection.getLatestBlockhash();

  // Create a transaction message for the vault transaction
  const message = new TransactionMessage({
    payerKey: vaultPda,
    recentBlockhash: blockhash,
    instructions: [upgradeIx],
  });

  // Create the vault transaction
  const createVaultTxIx = multisig.instructions.vaultTransactionCreate({
    multisigPda: MULTISIG_PUBKEY,
    transactionIndex,
    creator: payer.publicKey,
    vaultIndex: 0,
    ephemeralSigners: 0,
    transactionMessage: message,
    memo: "Upgrade attention-oracle-program: V3 proof expiry enforcement",
  });

  // Create proposal
  const [proposalPda] = multisig.getProposalPda({
    multisigPda: MULTISIG_PUBKEY,
    transactionIndex,
  });

  const createProposalIx = multisig.instructions.proposalCreate({
    multisigPda: MULTISIG_PUBKEY,
    transactionIndex,
    creator: payer.publicKey,
  });

  // Approve proposal (as the proposer)
  const approveIx = multisig.instructions.proposalApprove({
    multisigPda: MULTISIG_PUBKEY,
    transactionIndex,
    member: payer.publicKey,
  });

  // Build and send transaction
  const tx = new VersionedTransaction(
    new TransactionMessage({
      payerKey: payer.publicKey,
      recentBlockhash: blockhash,
      instructions: [createVaultTxIx, createProposalIx, approveIx],
    }).compileToV0Message()
  );

  tx.sign([payer]);

  console.log("\nSending proposal transaction...");
  const sig = await connection.sendTransaction(tx, {
    skipPreflight: false,
    preflightCommitment: "confirmed",
  });

  console.log("Transaction signature:", sig);
  console.log("Waiting for confirmation...");

  await connection.confirmTransaction(sig, "confirmed");

  console.log("\n✅ Proposal created successfully!");
  console.log("Transaction PDA:", transactionPda.toBase58());
  console.log("Proposal PDA:", proposalPda.toBase58());
  console.log("\n🔗 View and approve at:");
  console.log(`https://v4.squads.so/squads/${MULTISIG_PUBKEY.toBase58()}/transactions/${transactionIndex}`);
  console.log("\n⚠️  Needs 2 more approvals (3-of-5 threshold)");
}

main().catch((err) => {
  console.error("Error:", err);
  process.exit(1);
});
