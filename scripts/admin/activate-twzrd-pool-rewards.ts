/**
 * Activate rewards for 5 TWZRD abstract pools via Squads Multisig
 *
 * Creates a single Squads vault transaction with 5 set_reward_rate instructions,
 * then creates a proposal and approves with 2 local keypairs. The 3rd approver
 * can batch-approve in the Squads UI.
 *
 * Current status (verified Feb 2, 2026):
 *   twzrd-247-6h:   reward_per_slot = 0  (has 980B weighted staked!)
 *   twzrd-1999-6h:  reward_per_slot = 0  (no stakers)
 *   twzrd-415-6h:   reward_per_slot = 0  (no stakers)
 *   twzrd-3121-6h:  reward_per_slot = 0  (no stakers)
 *   twzrd-69-6h:    reward_per_slot = 0  (no stakers)
 *
 * Target rate: 12,894 per slot (matching lower Spotify playlist tier)
 *
 * Usage:
 *   RPC_URL="..." npx tsx scripts/admin/activate-twzrd-pool-rewards.ts
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

/** Target reward rate: 12,894 per slot (matching lower Spotify playlist tier) */
const NEW_REWARD_RATE = 12_894;

/** The 5 TWZRD pools that currently have reward_per_slot = 0 */
const TWZRD_POOLS = [
  {
    name: "twzrd-247-6h",
    channelConfig: "DT7ztXPv4SMMPGNdaXQ8YMPvFwt82YG2LJNKiBHpFTa8",
  },
  {
    name: "twzrd-1999-6h",
    channelConfig: "3v2V4PtxmUfk22DZhLgVx8wSMPKgpNkv83a7cKEXJq6z",
  },
  {
    name: "twzrd-415-6h",
    channelConfig: "4W3hJ1MWnKEUfNM2hPZQPEPxHx7m7B9h6z3TpDPW7dK9",
  },
  {
    name: "twzrd-3121-6h",
    channelConfig: "9wZ4tJXKx7Y5VqPKmNhD8FgQXZ2Yx3pW6R1vT8sNcMd4",
  },
  {
    name: "twzrd-69-6h",
    channelConfig: "3E5vP2tKm8L9XqR1wT6yN4zH7sF8dJ2vB9cA5xG1nY6W",
  },
];

const KEYPAIR_PATHS = [
  `${process.env.HOME}/.config/solana/id.json`, // 2pHj...
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
 * Calculate Anchor discriminator for an instruction name.
 * Formula: first 8 bytes of SHA-256("global:<instruction_name>")
 */
function anchorDiscriminator(name: string): Buffer {
  const preimage = `global:${name}`;
  const hash = crypto.createHash("sha256").update(preimage).digest();
  return hash.subarray(0, 8);
}

/**
 * Build the set_reward_rate instruction.
 *
 * Accounts (from staking.rs SetRewardRate):
 *   0. admin           — mut, signer (the Squads vault PDA)
 *   1. protocol_state  — PDA derived from ["protocol", mint]
 *   2. channel_config  — the channel config pubkey
 *   3. stake_pool      — mut, PDA derived from ["channel_pool", channel_config]
 *   4. system_program  — System Program (required for realloc)
 *
 * Data: 8-byte discriminator + u64 LE new_rate
 */
function setRewardRateIx(
  admin: PublicKey,
  protocolState: PublicKey,
  channelConfig: PublicKey,
  stakePool: PublicKey,
  newRate: number,
): TransactionInstruction {
  const discriminator = anchorDiscriminator("set_reward_rate");
  const data = Buffer.alloc(8 + 8);
  discriminator.copy(data, 0);
  data.writeBigUInt64LE(BigInt(newRate), 8);

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
      '  RPC_URL="https://..." npx tsx scripts/admin/activate-twzrd-pool-rewards.ts',
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

  // Check if tx #27 or #28 exist from failed previous runs
  // If so, skip to the next available index
  const existingTxs = [27, 28];
  for (const txIdx of existingTxs) {
    try {
      const [txPda] = multisig.getTransactionPda({
        multisigPda: MULTISIG_PDA,
        index: BigInt(txIdx),
      });
      const txInfo = await connection.getAccountInfo(txPda);
      if (txInfo) {
        console.log(`  Skipping tx #${txIdx} (already exists)`);
        currentIndex = Math.max(currentIndex, txIdx);
      }
    } catch {
      // Doesn't exist, ok
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

  // --- Build instructions for all 5 pools ---

  console.log(
    `\n  Building ${TWZRD_POOLS.length} set_reward_rate instructions (rate=${NEW_REWARD_RATE})...\n`,
  );

  const instructions: TransactionInstruction[] = [];

  for (const pool of TWZRD_POOLS) {
    const channelConfig = new PublicKey(pool.channelConfig);

    // Derive stake pool PDA
    const [stakePool] = PublicKey.findProgramAddressSync(
      [CHANNEL_STAKE_POOL_SEED, channelConfig.toBuffer()],
      ORACLE_PROGRAM_ID,
    );

    console.log(`  ${pool.name.padEnd(18)} ${stakePool.toBase58()}`);

    const ix = setRewardRateIx(
      squadsVaultPda,
      protocolState,
      channelConfig,
      stakePool,
      NEW_REWARD_RATE,
    );
    instructions.push(ix);
  }

  // --- Create single Squads vault transaction with all 5 instructions ---

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
  console.log("  SUMMARY - Proposal created");
  console.log("=".repeat(70));
  console.log(`\n  Target rate:  ${NEW_REWARD_RATE} per slot`);
  console.log(`  Pools:        ${TWZRD_POOLS.length}`);
  console.log(`  Tx index:     ${txIndex}`);
  console.log(`  Approvals:    2 / ${multisigAccount.threshold} (need 3rd)`);
  console.log();

  console.log("  Pools to activate:");
  for (const pool of TWZRD_POOLS) {
    console.log(`    • ${pool.name}`);
  }

  console.log(`\n  Next steps:`);
  console.log(
    `  1. Open app.squads.so -> approve tx #${txIndex}`,
  );
  console.log(`  2. Execute the proposal`);
  console.log(
    `  3. Verify: npx tsx scripts/admin/verify-twzrd-pool-rewards.ts`,
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
