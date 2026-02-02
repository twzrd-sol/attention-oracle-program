/**
 * Shut down 4 dead TWZRD pools via Squads Multisig
 *
 * These pools have 0 stakers and 0 weighted stake. Shutting them down:
 * - Sets is_shutdown = true (blocks new stakes)
 * - Sets reward_per_slot = 0 (stops emission)
 * - Waives any lock durations for exit
 *
 * Pools to shut down:
 *   twzrd-1999-6h:  0 stakers, 0 weighted
 *   twzrd-415-6h:   0 stakers, 0 weighted
 *   twzrd-3121-6h:  0 stakers, 0 weighted
 *   twzrd-69-6h:    0 stakers, 0 weighted
 *
 * Usage:
 *   RPC_URL="..." npx tsx scripts/admin/shutdown-dead-pools.ts
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
import * as crypto from "crypto";
// Channel configs hardcoded (pools already removed from channels.ts)

// ============================================================================
// Constants
// ============================================================================

const ORACLE_PROGRAM_ID = new PublicKey(
  "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop",
);

const MULTISIG_PDA = new PublicKey(
  "BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ",
);

const PROTOCOL_SEED = Buffer.from("protocol");
const CHANNEL_STAKE_POOL_SEED = Buffer.from("channel_pool");

const CCM_MINT = new PublicKey(
  "Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM",
);

/** Pools to shut down (0 stakers, dead) — hardcoded since already removed from channels.ts */
const DEAD_POOLS = [
  { name: "twzrd-1999-6h", channelConfig: "7g1qkWgZkbhZNFgbEzxxvYxCJHt4NMb3fwE2RHyrygDL" },
  { name: "twzrd-415-6h", channelConfig: "DqoM3QcGPbUD2Hic1fxsSLqZY1CaSDkiaNaas2ufZUpb" },
  { name: "twzrd-3121-6h", channelConfig: "EADvLuoe6ZXTfVBpVEKAMSfnFr1oZuHMxiButLVMnHuE" },
  { name: "twzrd-69-6h", channelConfig: "HEa4KgAyuvRZPyAsUPmVTRXiTRuxVEkkGbmtEeybzGB9" },
];

const SHUTDOWN_REASON = "Pruned: zero stakers, consolidating lock tiers";

const KEYPAIR_PATHS = [
  `${process.env.HOME}/.config/solana/id.json`,
  `${process.env.HOME}/.config/solana/oracle-authority.json`,
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

function anchorDiscriminator(name: string): Buffer {
  const preimage = `global:${name}`;
  const hash = crypto.createHash("sha256").update(preimage).digest();
  return hash.subarray(0, 8);
}

/**
 * Build the admin_shutdown_pool instruction.
 *
 * Accounts (from staking.rs AdminShutdownPool):
 *   0. admin           — mut, signer (Squads vault PDA)
 *   1. protocol_state  — PDA from ["protocol", mint]
 *   2. channel_config  — the channel config pubkey
 *   3. stake_pool      — mut, PDA from ["channel_pool", channel_config]
 *   4. system_program  — required for realloc
 *
 * Data: 8-byte discriminator + Borsh string (u32 LE length + UTF-8 bytes)
 */
function adminShutdownPoolIx(
  admin: PublicKey,
  protocolState: PublicKey,
  channelConfig: PublicKey,
  stakePool: PublicKey,
  reason: string,
): TransactionInstruction {
  const discriminator = anchorDiscriminator("admin_shutdown_pool");
  const reasonBytes = Buffer.from(reason, "utf-8");
  const data = Buffer.alloc(8 + 4 + reasonBytes.length);
  discriminator.copy(data, 0);
  data.writeUInt32LE(reasonBytes.length, 8);
  reasonBytes.copy(data, 12);

  return new TransactionInstruction({
    programId: ORACLE_PROGRAM_ID,
    keys: [
      { pubkey: admin, isSigner: true, isWritable: true },
      { pubkey: protocolState, isSigner: false, isWritable: false },
      { pubkey: channelConfig, isSigner: false, isWritable: false },
      { pubkey: stakePool, isSigner: false, isWritable: true },
      { pubkey: new PublicKey("11111111111111111111111111111111"), isSigner: false, isWritable: false },
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
      '  RPC_URL="https://..." npx tsx scripts/admin/shutdown-dead-pools.ts',
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

  // --- Derive Squads vault PDA (the admin) ---

  const [squadsVaultPda] = multisig.getVaultPda({
    multisigPda: MULTISIG_PDA,
    index: 0,
  });
  console.log(`\n  Squads vault PDA: ${squadsVaultPda.toBase58()}`);

  // --- Derive protocol state ---

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

  // Skip existing proposals
  for (let txIdx = currentIndex + 1; txIdx <= currentIndex + 10; txIdx++) {
    try {
      const [proposalPda] = multisig.getProposalPda({
        multisigPda: MULTISIG_PDA,
        transactionIndex: BigInt(txIdx),
      });
      const proposalInfo = await connection.getAccountInfo(proposalPda);
      if (proposalInfo) {
        console.log(`  Skipping tx #${txIdx} (proposal already exists)`);
        currentIndex = txIdx;
      }
    } catch {
      break;
    }
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

  // --- Build shutdown instructions for all 4 dead pools ---

  console.log(
    `\n  Building ${DEAD_POOLS.length} admin_shutdown_pool instructions...\n`,
  );

  const instructions: TransactionInstruction[] = [];

  for (const pool of DEAD_POOLS) {
    const channelConfig = new PublicKey(pool.channelConfig);

    const [stakePool] = PublicKey.findProgramAddressSync(
      [CHANNEL_STAKE_POOL_SEED, channelConfig.toBuffer()],
      ORACLE_PROGRAM_ID,
    );

    console.log(`  ${pool.name.padEnd(18)} -> shutdown`);

    const ix = adminShutdownPoolIx(
      squadsVaultPda,
      protocolState,
      channelConfig,
      stakePool,
      SHUTDOWN_REASON,
    );
    instructions.push(ix);
  }

  // --- Create Squads vault transaction ---

  const txIndex = BigInt(currentIndex + 1);
  console.log(`\n  Creating Squads vault transaction #${txIndex}...\n`);

  const { blockhash } = await connection.getLatestBlockhash("confirmed");
  const message = new TransactionMessage({
    payerKey: squadsVaultPda,
    recentBlockhash: blockhash,
    instructions,
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

  // --- Create proposal ---

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

  // --- Approve with both local keypairs ---

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

  // --- Summary ---

  console.log("\n" + "=".repeat(70));
  console.log("  SUMMARY - Shutdown Proposal Created");
  console.log("=".repeat(70));
  console.log(`\n  Tx index:     ${txIndex}`);
  console.log(`  Approvals:    2 / ${multisigAccount.threshold}`);
  console.log(`  Reason:       ${SHUTDOWN_REASON}`);
  console.log();

  console.log("  Pools to shut down:");
  for (const pool of DEAD_POOLS) {
    console.log(`    • ${pool.name}`);
  }

  console.log(`\n  Next steps:`);
  console.log(
    `  1. Open app.squads.so -> approve tx #${txIndex}`,
  );
  console.log(`  2. Execute the proposal`);
  console.log(
    `  3. Remove dead pools from scripts/keepers/lib/channels.ts`,
  );
  console.log(
    `  4. Run: npx tsx scripts/admin/audit-onchain-channels.ts to verify`,
  );
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
