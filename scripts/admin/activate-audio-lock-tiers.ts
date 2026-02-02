/**
 * Activate rewards for new audio lock-tier pools via Squads Multisig
 *
 * Sets reward rates for pools that have received deposits.
 * Target APRs: 3h = 8%, 12h = 12% (matching existing schedule).
 *
 * Rate formula: rate = (APR_bps * total_weighted) / (BPS_DENOMINATOR * SLOTS_PER_YEAR)
 * APR cap: 15% (1500 bps) enforced on-chain.
 *
 * Usage:
 *   RPC_URL="..." npx tsx scripts/admin/activate-audio-lock-tiers.ts
 */

import {
  Connection,
  Keypair,
  PublicKey,
  TransactionMessage,
  TransactionInstruction,
} from "@solana/web3.js";
import * as multisig from "@sqds/multisig";
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import * as fs from "fs";
import * as crypto from "crypto";
import { keccak_256 } from "@noble/hashes/sha3";
import { CCM_V3_MINT } from "../config.js";

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
const CHANNEL_CONFIG_V2_SEED = Buffer.from("channel_cfg_v2");

const CCM_MINT = new PublicKey(
  "Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM",
);

const BPS_DENOMINATOR = 10_000;
const SLOTS_PER_YEAR = 78_840_000;

/** Target APRs per lock tier */
const APR_SCHEDULE: Record<string, number> = {
  "3h": 800,   // 8%
  "12h": 1200, // 12%
};

const PLAYLISTS = ["999", "212", "247", "1999", "415", "3121", "69"];
const TIERS = ["3h", "12h"];

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

function deriveSubjectId(channel: string): Buffer {
  const input = Buffer.concat([
    Buffer.from("channel:"),
    Buffer.from(channel.toLowerCase()),
  ]);
  return Buffer.from(keccak_256(input));
}

function deriveChannelConfig(channelName: string): PublicKey {
  return PublicKey.findProgramAddressSync(
    [CHANNEL_CONFIG_V2_SEED, CCM_MINT.toBuffer(), deriveSubjectId(channelName)],
    ORACLE_PROGRAM_ID,
  )[0];
}

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

/**
 * Calculate reward rate from target APR and pool TVL.
 * Returns 0 if pool has no stake (can't set rate on empty pool).
 */
function calculateRate(targetAprBps: number, totalWeighted: number): number {
  if (totalWeighted === 0) return 0;
  const rate = Math.floor(
    (targetAprBps * totalWeighted) / (BPS_DENOMINATOR * SLOTS_PER_YEAR),
  );
  // Also compute the max allowed rate (15% APR cap)
  const maxRate = Math.floor(
    (1500 * totalWeighted) / (BPS_DENOMINATOR * SLOTS_PER_YEAR),
  );
  return Math.min(rate, maxRate);
}

// ============================================================================
// Main
// ============================================================================

async function main() {
  const rpcUrl = process.env.RPC_URL;
  if (!rpcUrl) {
    console.error("ERROR: Set RPC_URL environment variable");
    process.exit(1);
  }

  const connection = new Connection(rpcUrl, "confirmed");
  const wallet = new anchor.Wallet(anchor.web3.Keypair.generate());
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });
  const oracleIdl = await Program.fetchIdl(ORACLE_PROGRAM_ID, provider);
  if (!oracleIdl) throw new Error("Oracle IDL not found on-chain");
  const oracleProgram = new Program(oracleIdl, provider);

  // --- Load keypairs ---

  const keypairs = KEYPAIR_PATHS.map((p) => {
    const kp = loadKeypair(p);
    console.log(
      `  Loaded: ${kp.publicKey.toBase58().slice(0, 8)}... (${p.split("/").pop()})`,
    );
    return kp;
  });
  const feePayer = keypairs[0];

  // --- Derive admin PDA ---

  const [squadsVaultPda] = multisig.getVaultPda({
    multisigPda: MULTISIG_PDA,
    index: 0,
  });

  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, CCM_MINT.toBuffer()],
    ORACLE_PROGRAM_ID,
  );

  // --- Scan all lock-tier pools for stake ---

  console.log("\n" + "=".repeat(70));
  console.log("  AUDIO LOCK-TIER REWARD ACTIVATION");
  console.log("=".repeat(70));
  console.log();

  const activatable: Array<{
    name: string;
    channelConfig: PublicKey;
    stakePool: PublicKey;
    totalWeighted: number;
    targetAprBps: number;
    rate: number;
  }> = [];

  for (const playlist of PLAYLISTS) {
    for (const tier of TIERS) {
      const channelName = `audio:${playlist}:${tier}`;
      const vaultName = `audio-${playlist}-${tier}`;
      const channelConfig = deriveChannelConfig(channelName);
      const [stakePool] = PublicKey.findProgramAddressSync(
        [CHANNEL_STAKE_POOL_SEED, channelConfig.toBuffer()],
        ORACLE_PROGRAM_ID,
      );

      try {
        const poolData: any =
          await oracleProgram.account.channelStakePool.fetch(stakePool);

        const rewardPerSlot = parseInt(poolData.rewardPerSlot, 10);
        const totalWeighted = parseInt(poolData.totalWeighted, 10);
        const isShutdown = poolData.isShutdown;

        if (isShutdown) {
          console.log(`  ${vaultName.padEnd(20)} SHUTDOWN (skip)`);
          continue;
        }

        if (rewardPerSlot > 0) {
          console.log(
            `  ${vaultName.padEnd(20)} Already active (${rewardPerSlot}/slot)`,
          );
          continue;
        }

        if (totalWeighted === 0) {
          console.log(`  ${vaultName.padEnd(20)} No stake yet (skip)`);
          continue;
        }

        const targetAprBps = APR_SCHEDULE[tier] || 1000;
        const rate = calculateRate(targetAprBps, totalWeighted);

        console.log(
          `  ${vaultName.padEnd(20)} ${(totalWeighted / 1e9).toFixed(1)}B staked -> ${rate}/slot (${targetAprBps / 100}% APR)`,
        );

        activatable.push({
          name: vaultName,
          channelConfig,
          stakePool,
          totalWeighted,
          targetAprBps,
          rate,
        });
      } catch (err: any) {
        console.log(`  ${vaultName.padEnd(20)} Pool not found (not deployed?)`);
      }
    }
  }

  if (activatable.length === 0) {
    console.log("\n  No pools ready for activation (all empty or already active).");
    console.log("  Wait for deposits, then run this script again.");
    return;
  }

  // --- Create Squads proposal ---

  console.log(`\n  ${activatable.length} pool(s) ready for activation.\n`);

  const multisigAccount = await multisig.accounts.Multisig.fromAccountAddress(
    connection,
    MULTISIG_PDA,
  );

  let currentIndex = Number(multisigAccount.transactionIndex);

  // Skip existing proposals
  for (let txIdx = currentIndex + 1; txIdx <= currentIndex + 10; txIdx++) {
    try {
      const [proposalPda] = multisig.getProposalPda({
        multisigPda: MULTISIG_PDA,
        transactionIndex: BigInt(txIdx),
      });
      const proposalInfo = await connection.getAccountInfo(proposalPda);
      if (proposalInfo) {
        currentIndex = txIdx;
      }
    } catch {
      break;
    }
  }

  const instructions: TransactionInstruction[] = [];

  for (const pool of activatable) {
    instructions.push(
      setRewardRateIx(
        squadsVaultPda,
        protocolState,
        pool.channelConfig,
        pool.stakePool,
        pool.rate,
      ),
    );
  }

  const txIndex = BigInt(currentIndex + 1);
  console.log(`  Creating Squads vault transaction #${txIndex}...\n`);

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
  console.log("  SUMMARY");
  console.log("=".repeat(70));
  console.log(`\n  Tx index:   ${txIndex}`);
  console.log(`  Approvals:  2 / ${multisigAccount.threshold}`);
  console.log();
  for (const pool of activatable) {
    console.log(
      `  ${pool.name.padEnd(20)} ${pool.rate}/slot (${pool.targetAprBps / 100}% APR, ${(pool.totalWeighted / 1e9).toFixed(1)}B staked)`,
    );
  }
  console.log(`\n  Next: approve tx #${txIndex} in app.squads.so and execute.`);
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
