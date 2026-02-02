/**
 * Upgrade Channel Vault Program via Squads Multisig
 *
 * Steps:
 *   1. Write buffer:  solana program write-buffer target/deploy/channel_vault.so --url mainnet-beta
 *   2. Run this:      RPC_URL="..." npx tsx scripts/admin/upgrade-program.ts <buffer_address>
 *   3. Approve 3rd wallet in Squads UI
 *   4. Execute in Squads UI (or --execute <txIndex>)
 *
 * What this script does:
 *   - Verifies the buffer exists and has valid program data
 *   - Creates a Squads vault transaction wrapping BPF Loader Upgrade
 *   - Creates a proposal
 *   - Approves with 2 local member keypairs (2pHj + 87d5)
 *
 * After upgrade completes:
 *   - Push new code to GitHub
 *   - Re-submit for OtterSec verification
 *   - Run oracle position migration
 */

import {
  Connection,
  Keypair,
  PublicKey,
  TransactionMessage,
  TransactionInstruction,
  SYSVAR_RENT_PUBKEY,
  SYSVAR_CLOCK_PUBKEY,
} from "@solana/web3.js";
import * as multisig from "@sqds/multisig";
import * as fs from "fs";

// ============================================================================
// Constants
// ============================================================================

const PROGRAMS: Record<string, { programId: PublicKey; programData: PublicKey; label: string }> = {
  vault: {
    programId: new PublicKey("5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ"),
    programData: new PublicKey("2ubXWFAJFCnBqJ1vYCsf4q8SYRcqf5DaTfkC6wASK5SQ"),
    label: "Channel Vault",
  },
  oracle: {
    programId: new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"),
    programData: new PublicKey("5GyaaVmzRr2r9KcUuzt9SxBVq9ubTT5m3pH9Lzy3Kh4L"),
    label: "Attention Oracle",
  },
};

const MULTISIG_PDA = new PublicKey("BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ");
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

/**
 * BPF Upgradeable Loader Upgrade instruction (enum index 3).
 *
 * Accounts:
 *   0. programdata  (writable)  — program's data account
 *   1. program      (writable)  — the program account
 *   2. buffer       (writable)  — buffer containing new program data
 *   3. spill        (writable)  — receives buffer rent refund
 *   4. rent sysvar
 *   5. clock sysvar
 *   6. authority    (signer)    — current upgrade authority
 */
function upgradeIx(
  programdata: PublicKey,
  program: PublicKey,
  buffer: PublicKey,
  spill: PublicKey,
  authority: PublicKey,
): TransactionInstruction {
  return new TransactionInstruction({
    programId: BPF_LOADER_UPGRADEABLE,
    keys: [
      { pubkey: programdata, isSigner: false, isWritable: true },
      { pubkey: program,     isSigner: false, isWritable: true },
      { pubkey: buffer,      isSigner: false, isWritable: true },
      { pubkey: spill,       isSigner: false, isWritable: true },
      { pubkey: SYSVAR_RENT_PUBKEY,  isSigner: false, isWritable: false },
      { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: authority,   isSigner: true,  isWritable: false },
    ],
    // UpgradeableLoaderInstruction::Upgrade = variant 3, bincode u32 LE
    data: Buffer.from([3, 0, 0, 0]),
  });
}

// ============================================================================
// Create: vault transaction + proposal + approvals
// ============================================================================

async function createAndApprove(
  connection: Connection,
  bufferAddress: PublicKey,
  programConfig: typeof PROGRAMS[string],
): Promise<void> {
  const PROGRAM_ID = programConfig.programId;
  const PROGRAMDATA_ADDRESS = programConfig.programData;

  console.log("\n" + "=".repeat(60));
  console.log(`  ${programConfig.label} Program Upgrade - Squads Proposal`);
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

  console.log(`\n  Squads vault:    ${vaultPda.toBase58()}`);
  console.log(`  Program:         ${PROGRAM_ID.toBase58()}`);
  console.log(`  ProgramData:     ${PROGRAMDATA_ADDRESS.toBase58()}`);
  console.log(`  Buffer:          ${bufferAddress.toBase58()}`);

  // --- Verify buffer ---
  console.log("\n--- Verifying buffer ---\n");

  const bufferInfo = await connection.getAccountInfo(bufferAddress);
  if (!bufferInfo) {
    console.error("  ERROR: Buffer not found. Did you run:");
    console.error("  solana program write-buffer target/deploy/channel_vault.so --url mainnet-beta");
    process.exit(1);
  }

  if (!bufferInfo.owner.equals(BPF_LOADER_UPGRADEABLE)) {
    console.error(`  ERROR: Buffer not owned by BPF Loader (owner: ${bufferInfo.owner.toBase58()})`);
    process.exit(1);
  }

  // Buffer account type tag should be 1 (Buffer)
  const accountType = bufferInfo.data.readUInt32LE(0);
  if (accountType !== 1) {
    console.error(`  ERROR: Not a buffer account (type=${accountType})`);
    process.exit(1);
  }

  const bufferSize = bufferInfo.data.length;
  const bufferLamports = bufferInfo.lamports;
  console.log(`  Buffer size:     ${(bufferSize / 1024).toFixed(1)} KB`);
  console.log(`  Buffer rent:     ${(bufferLamports / 1e9).toFixed(4)} SOL`);

  // Parse buffer authority
  const hasAuthority = bufferInfo.data[4];
  if (hasAuthority === 1) {
    const bufferAuth = new PublicKey(bufferInfo.data.slice(5, 37));
    console.log(`  Buffer authority: ${bufferAuth.toBase58()}`);
  }

  // --- Verify program ---
  console.log("\n--- Verifying program ---\n");

  const programInfo = await connection.getAccountInfo(PROGRAM_ID);
  if (!programInfo) {
    console.error("  ERROR: Program account not found");
    process.exit(1);
  }

  const programdataInfo = await connection.getAccountInfo(PROGRAMDATA_ADDRESS);
  if (!programdataInfo) {
    console.error("  ERROR: ProgramData account not found");
    process.exit(1);
  }

  // Parse upgrade authority from programdata: [4] accountType, [4+1] Option tag, [5+1..37+1] authority
  // ProgramData layout: [4] type, [8] slot, [1] option, [32] authority
  const pdAuthOption = programdataInfo.data[12]; // offset 4+8
  if (pdAuthOption !== 1) {
    console.error("  ERROR: Program has no upgrade authority (immutable)");
    process.exit(1);
  }
  const currentAuthority = new PublicKey(programdataInfo.data.slice(13, 45));
  console.log(`  Current authority: ${currentAuthority.toBase58()}`);

  if (!currentAuthority.equals(vaultPda)) {
    console.error(`  ERROR: Upgrade authority is not the Squads vault`);
    console.error(`    Expected: ${vaultPda.toBase58()}`);
    console.error(`    Got:      ${currentAuthority.toBase58()}`);
    process.exit(1);
  }
  console.log("  OK: Upgrade authority matches Squads vault");

  // Current program size
  const currentSize = programdataInfo.data.length;
  console.log(`  Current size:    ${(currentSize / 1024).toFixed(1)} KB`);

  // --- Fetch multisig state ---
  console.log("\n--- Fetching multisig state ---\n");

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

  // --- Create vault transaction ---
  console.log("\n--- Creating vault transaction ---\n");

  // Spill account receives buffer rent refund after upgrade
  const spill = keypairs[0].publicKey;

  const ix = upgradeIx(
    PROGRAMDATA_ADDRESS,
    PROGRAM_ID,
    bufferAddress,
    spill,
    vaultPda,
  );

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

  // --- Create proposal ---
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

  // --- Approve ---
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

  // --- Summary ---
  const [txPda] = multisig.getTransactionPda({
    multisigPda: MULTISIG_PDA,
    index: txIndex,
  });

  console.log("\n" + "=".repeat(60));
  console.log("  Upgrade Proposal Ready");
  console.log("=".repeat(60));
  console.log(`\n  Tx Index:    ${txIndex}`);
  console.log(`  Tx PDA:      ${txPda.toBase58()}`);
  console.log(`  Approvals:   2 / ${multisigAccount.threshold}`);
  console.log(`  Buffer:      ${bufferAddress.toBase58()}`);
  console.log(`  Buffer rent: ${(bufferLamports / 1e9).toFixed(4)} SOL (refunded to ${spill.toBase58().slice(0, 8)}...)`);
  console.log(`\n  Next steps:`);
  console.log(`  1. Open app.squads.so -> approve with 3rd wallet`);
  console.log(`  2. Execute the proposal`);
  console.log(`     OR: RPC_URL="..." npx tsx scripts/admin/upgrade-program.ts --execute ${txIndex}`);
  console.log(`\n  After upgrade:`);
  console.log(`  3. Verify: solana program show ${PROGRAM_ID.toBase58()} --url mainnet-beta`);
  console.log(`  4. Run migration: npx tsx scripts/admin/migrate-oracle-positions.ts`);
  console.log(`  5. Push code + re-submit OtterSec verification`);
  console.log();
}

// ============================================================================
// Execute: run an approved vault transaction
// ============================================================================

async function executeTransaction(
  connection: Connection,
  txIndex: bigint,
  programConfig: typeof PROGRAMS[string],
): Promise<void> {
  const PROGRAMDATA_ADDRESS = programConfig.programData;
  console.log("\n" + "=".repeat(60));
  console.log("  Execute Upgrade Transaction");
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

  const status = (proposal.status as any).__kind || Object.keys(proposal.status)[0];
  console.log(`  Proposal status: ${status}`);
  console.log(`  Approved by:     ${proposal.approved.length} members`);

  if (status.toLowerCase() !== "approved") {
    console.error(`\n  ERROR: Proposal is '${status}', need 'approved' to execute`);
    process.exit(1);
  }

  console.log("\n--- Executing upgrade ---\n");

  const execSig = await multisig.rpc.vaultTransactionExecute({
    connection,
    feePayer,
    multisigPda: MULTISIG_PDA,
    transactionIndex: txIndex,
    member: feePayer.publicKey,
  });

  console.log(`  Executed: ${execSig}`);
  await sleep(2000);

  // Verify program was updated
  const programInfo = await connection.getAccountInfo(PROGRAMDATA_ADDRESS);
  if (programInfo) {
    // ProgramData layout: [4] type, [8] slot
    const lastDeploySlot = Number(programInfo.data.readBigUInt64LE(4));
    console.log(`\n  Program updated!`);
    console.log(`  Last deployed slot: ${lastDeploySlot}`);
    console.log(`  Program size: ${(programInfo.data.length / 1024).toFixed(1)} KB`);
  }

  console.log(`\n  Next:`);
  console.log(`  1. Run oracle position migration`);
  console.log(`  2. Push new code to GitHub`);
  console.log(`  3. Re-submit for OtterSec verification`);
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

  // Parse --oracle / --vault flag (default: vault for backwards compat)
  const programKey = args.includes("--oracle") ? "oracle" : "vault";
  const programConfig = PROGRAMS[programKey];
  const filteredArgs = args.filter((a) => a !== "--oracle" && a !== "--vault");

  if (filteredArgs[0] === "--execute") {
    const idx = filteredArgs[1];
    if (!idx) {
      console.error("Usage: --execute <txIndex> [--oracle|--vault]");
      process.exit(1);
    }
    await executeTransaction(connection, BigInt(idx), programConfig);
  } else {
    const bufferArg = filteredArgs[0];
    if (!bufferArg) {
      console.error("Usage:");
      console.error("  # Step 1: Write buffer");
      console.error("  solana program write-buffer target/deploy/<program>.so --url mainnet-beta");
      console.error("");
      console.error("  # Step 2: Create upgrade proposal");
      console.error("  RPC_URL=\"...\" npx tsx scripts/admin/upgrade-program.ts [--oracle|--vault] <buffer_address>");
      console.error("");
      console.error("  # Step 3: Execute after 3rd approval");
      console.error("  RPC_URL=\"...\" npx tsx scripts/admin/upgrade-program.ts [--oracle|--vault] --execute <txIndex>");
      process.exit(1);
    }

    let bufferAddress: PublicKey;
    try {
      bufferAddress = new PublicKey(bufferArg);
    } catch {
      console.error(`ERROR: Invalid buffer address: ${bufferArg}`);
      process.exit(1);
    }

    await createAndApprove(connection, bufferAddress, programConfig);
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
