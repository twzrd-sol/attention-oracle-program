/**
 * Shut down old pools via Squads Multisig
 *
 * Shutting down a pool:
 * - Sets is_shutdown = true (blocks new stakes)
 * - Sets reward_per_slot = 0 (stops emission)
 * - Waives any lock durations for exit
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

/** All old pools to shut down — consolidating to 14 new lock-tier pools */
const DEAD_POOLS = [
  // Lofi vaults
  { name: "lofi-vault-3h", channelConfig: "J3HAT4NbL6REyyNqbW1BDGF9BXXc3FYuQ1fr6NbCQaoW" },
  { name: "lofi-vault-6h", channelConfig: "dJvatt5bga4ak64ghTLEtxs1jxfLX4TNoZuvfiDCcGy" },
  { name: "lofi-vault-9h", channelConfig: "2TWM1H1gHWrA6Ta6A9tH3E1TTTRbPpmSL2Xg7KdHwxCM" },
  { name: "lofi-vault-12h", channelConfig: "GZL7vAo9vxdNbsmrreVueVd1Xm9oWmatkQauFcxhq8qP" },
  // TWZRD vault
  { name: "twzrd-247-6h", channelConfig: "84SxXryEL2dFT5rno9F1SGBAFvvkEDyp3wNQZyxT3hQ9" },
  // Audio standard (7.5h) pools
  { name: "audio-999", channelConfig: "9G1MvnVq3dX6UwGFvhTC9bDArNt9TyvS5UimffTL1BAJ" },
  { name: "audio-212", channelConfig: "Dg84d5BkSYxKSix9m6YgbLz1L7mEsSH81Svp24watxEC" },
  { name: "audio-247", channelConfig: "GdrV9DjKZFePZadxuQANKEBvVaB7rM8aEhMEzMHWrFJE" },
  { name: "audio-1999", channelConfig: "8LCSiL2a4FjTAveMMn8SjLVxrYecWSfFDH48sdhzdbv" },
  { name: "audio-415", channelConfig: "GxzK9iqyFJf3TRJG5XAQJD3eJtgKCivzkQtj7iPKrUsG" },
  { name: "audio-3121", channelConfig: "4JawzmsofxVCim7eDtFPCMwiP21NMcAQqsZRPT7k9uL1" },
  { name: "audio-69", channelConfig: "2uGQDJMsGy3undJCT9NazdJXjSoCcXd71vgkvYzMt3eR" },
];

const SHUTDOWN_REASON = "Consolidating to new lock-tier pools (3h + 12h)";

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

  // --- Build shutdown instructions in batches (tx size limits) ---

  const BATCH_SIZE = 6;
  const batches: typeof DEAD_POOLS[] = [];
  for (let i = 0; i < DEAD_POOLS.length; i += BATCH_SIZE) {
    batches.push(DEAD_POOLS.slice(i, i + BATCH_SIZE));
  }

  console.log(
    `\n  ${DEAD_POOLS.length} pools -> ${batches.length} batch(es) of ${BATCH_SIZE}\n`,
  );

  const txIndices: bigint[] = [];

  for (let b = 0; b < batches.length; b++) {
    const batch = batches[b];
    console.log(`  === Batch ${b + 1} (${batch.length} pools) ===\n`);

    const instructions: TransactionInstruction[] = [];
    for (const pool of batch) {
      const channelConfig = new PublicKey(pool.channelConfig);
      const [stakePool] = PublicKey.findProgramAddressSync(
        [CHANNEL_STAKE_POOL_SEED, channelConfig.toBuffer()],
        ORACLE_PROGRAM_ID,
      );
      console.log(`  ${pool.name.padEnd(18)} -> shutdown`);
      instructions.push(
        adminShutdownPoolIx(squadsVaultPda, protocolState, channelConfig, stakePool, SHUTDOWN_REASON),
      );
    }

    const txIndex = BigInt(currentIndex + 1 + b);
    txIndices.push(txIndex);
    console.log(`\n  Creating vault transaction #${txIndex}...`);

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
    await sleep(2000);

    const proposalSig = await multisig.rpc.proposalCreate({
      connection,
      feePayer,
      creator: feePayer,
      multisigPda: MULTISIG_PDA,
      transactionIndex: txIndex,
      isDraft: false,
    });
    console.log(`  Proposal created: ${proposalSig}`);
    await sleep(2000);

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

    console.log();
  }

  // --- Summary ---

  console.log("\n" + "=".repeat(70));
  console.log("  SUMMARY - Shutdown Proposals Created");
  console.log("=".repeat(70));
  console.log(`\n  Tx indices:   ${txIndices.join(", ")}`);
  console.log(`  Approvals:    2 / ${multisigAccount.threshold} each`);
  console.log(`  Reason:       ${SHUTDOWN_REASON}`);
  console.log();

  console.log("  Pools to shut down:");
  for (const pool of DEAD_POOLS) {
    console.log(`    • ${pool.name}`);
  }

  console.log(`\n  Next steps:`);
  console.log(
    `  1. Open app.squads.so -> approve tx #${txIndices.join(" and #")}`,
  );
  console.log(`  2. Execute both proposals`);
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
