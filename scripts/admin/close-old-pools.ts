/**
 * Close old stake pools and recover surplus reward CCM via Squads Multisig
 *
 * Prerequisites (per pool):
 *   - Pool must be shut down (admin_shutdown_pool already called)
 *   - All stakers have unstaked (staker_count = 0, total_staked = 0)
 *   - Rewards claimed before unstaking
 *
 * What this does:
 *   1. Transfers remaining vault tokens (reward surplus) to destination ATA
 *   2. Closes the vault Token-2022 ATA (recovers SOL rent)
 *   3. Closes the stake pool PDA (recovers SOL rent)
 *
 * Usage:
 *   RPC_URL="..." npx tsx scripts/admin/close-old-pools.ts
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
import {
  getAssociatedTokenAddressSync,
  TOKEN_2022_PROGRAM_ID,
} from "@solana/spl-token";
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

const CCM_MINT = new PublicKey(
  "Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM",
);

const PROTOCOL_SEED = Buffer.from("protocol");
const CHANNEL_STAKE_POOL_SEED = Buffer.from("channel_pool");

/** Old pools to close (must be shutdown with 0 stakers first) */
const OLD_POOLS = [
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
 * Build the close_stake_pool instruction.
 *
 * Accounts (from CloseStakePool):
 *   0. admin           — mut, signer (Squads vault PDA)
 *   1. protocol_state  — PDA from ["protocol", mint]
 *   2. channel_config  — the channel config pubkey
 *   3. stake_pool      — mut, PDA from ["channel_pool", channel_config]
 *   4. vault           — mut, address = stake_pool.vault
 *   5. mint            — mut, CCM mint (modified by withheld fee withdrawal)
 *   6. destination     — mut, where surplus tokens go
 *   7. token_program   — Token-2022
 *   8. system_program
 *
 * Data: 8-byte discriminator only (no args)
 */
function closeStakePoolIx(
  admin: PublicKey,
  protocolState: PublicKey,
  channelConfig: PublicKey,
  stakePool: PublicKey,
  vault: PublicKey,
  destination: PublicKey,
): TransactionInstruction {
  const data = anchorDiscriminator("close_stake_pool");

  return new TransactionInstruction({
    programId: ORACLE_PROGRAM_ID,
    keys: [
      { pubkey: admin, isSigner: true, isWritable: true },
      { pubkey: protocolState, isSigner: false, isWritable: false },
      { pubkey: channelConfig, isSigner: false, isWritable: false },
      { pubkey: stakePool, isSigner: false, isWritable: true },
      { pubkey: vault, isSigner: false, isWritable: true },
      { pubkey: CCM_MINT, isSigner: false, isWritable: true },
      { pubkey: destination, isSigner: false, isWritable: true },
      { pubkey: TOKEN_2022_PROGRAM_ID, isSigner: false, isWritable: false },
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

  // Destination for recovered CCM: the Squads vault's own ATA
  const destination = getAssociatedTokenAddressSync(
    CCM_MINT,
    squadsVaultPda,
    true, // allowOwnerOffCurve (PDA)
    TOKEN_2022_PROGRAM_ID,
  );

  console.log(`\n  Squads vault PDA: ${squadsVaultPda.toBase58()}`);
  console.log(`  Protocol state:   ${protocolState.toBase58()}`);
  console.log(`  Destination ATA:  ${destination.toBase58()}`);

  // --- Pre-flight: check which pools are ready to close ---

  console.log("\n" + "=".repeat(70));
  console.log("  CLOSE OLD POOLS — Pre-flight Check");
  console.log("=".repeat(70));
  console.log();

  const closeable: Array<{
    name: string;
    channelConfig: PublicKey;
    stakePool: PublicKey;
    vault: PublicKey;
    vaultBalance: number;
  }> = [];

  const notReady: string[] = [];

  for (const pool of OLD_POOLS) {
    const channelConfig = new PublicKey(pool.channelConfig);
    const [stakePool] = PublicKey.findProgramAddressSync(
      [CHANNEL_STAKE_POOL_SEED, channelConfig.toBuffer()],
      ORACLE_PROGRAM_ID,
    );

    try {
      const poolData: any =
        await oracleProgram.account.channelStakePool.fetch(stakePool);

      const isShutdown = poolData.isShutdown;
      const stakerCount = parseInt(poolData.stakerCount, 10);
      const totalStaked = parseInt(poolData.totalStaked, 10);
      const totalWeighted = parseInt(poolData.totalWeighted, 10);
      // Vault pubkey is stored on the pool state (not derived by seeds)
      const vault = new PublicKey(poolData.vault);

      if (!isShutdown) {
        notReady.push(`${pool.name}: NOT shutdown`);
        continue;
      }
      if (stakerCount > 0) {
        notReady.push(`${pool.name}: ${stakerCount} staker(s) remaining`);
        continue;
      }
      if (totalStaked > 0) {
        notReady.push(`${pool.name}: ${totalStaked} tokens still staked`);
        continue;
      }
      if (totalWeighted > 0) {
        notReady.push(`${pool.name}: ${totalWeighted} weighted stake remaining`);
        continue;
      }

      // Check vault balance
      let vaultBalance = 0;
      try {
        const vaultInfo = await connection.getTokenAccountBalance(vault);
        vaultBalance = parseInt(vaultInfo.value.amount, 10);
      } catch {
        // vault might already be closed
      }

      console.log(
        `  ${pool.name.padEnd(18)} READY (surplus: ${(vaultBalance / 1e9).toFixed(2)} CCM)`,
      );

      closeable.push({
        name: pool.name,
        channelConfig,
        stakePool,
        vault,
        vaultBalance,
      });
    } catch (err: any) {
      if (err.message?.includes("Account does not exist")) {
        console.log(`  ${pool.name.padEnd(18)} Already closed`);
      } else {
        notReady.push(`${pool.name}: ${err.message}`);
      }
    }
  }

  if (notReady.length > 0) {
    console.log(`\n  NOT READY (${notReady.length}):`);
    for (const msg of notReady) {
      console.log(`    - ${msg}`);
    }
  }

  if (closeable.length === 0) {
    console.log("\n  No pools ready to close.");
    console.log("  Ensure pools are shut down and all stakers have exited.");
    return;
  }

  // --- Build close instructions ---

  const totalSurplus = closeable.reduce((sum, p) => sum + p.vaultBalance, 0);
  console.log(
    `\n  ${closeable.length} pool(s) ready to close. Total surplus: ${(totalSurplus / 1e9).toFixed(2)} CCM`,
  );

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

  for (const pool of closeable) {
    instructions.push(
      closeStakePoolIx(
        squadsVaultPda,
        protocolState,
        pool.channelConfig,
        pool.stakePool,
        pool.vault,
        destination,
      ),
    );
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
  console.log("  SUMMARY — Close Pool Proposal Created");
  console.log("=".repeat(70));
  console.log(`\n  Tx index:       ${txIndex}`);
  console.log(`  Approvals:      2 / ${multisigAccount.threshold}`);
  console.log(`  Pools to close: ${closeable.length}`);
  console.log(`  Surplus CCM:    ${(totalSurplus / 1e9).toFixed(2)} (recoverable minus 0.5% transfer fee)`);
  console.log(`  Destination:    ${destination.toBase58()}`);
  console.log();

  for (const pool of closeable) {
    console.log(
      `    ${pool.name.padEnd(18)} ${(pool.vaultBalance / 1e9).toFixed(2)} CCM`,
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
