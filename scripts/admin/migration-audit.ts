/**
 * Migration Audit — Check vault balances before shutdown
 *
 * For each old pool, shows:
 *   - Vault token balance (total CCM in the vault)
 *   - Total staked (user principal, fully recoverable)
 *   - Reward excess (vault_balance - total_staked)
 *   - Current emission rate
 *   - Whether the pool has pending rewards to claim
 *
 * The "excess" is funded reward CCM. After shutdown:
 *   - Staked principal: 100% recoverable via unstake
 *   - Accumulated rewards: claimable via claim_rewards
 *   - Remaining excess after all claims: STUCK (no admin withdrawal)
 *
 * Usage:
 *   RPC_URL="..." npx tsx scripts/admin/migration-audit.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, PublicKey } from "@solana/web3.js";
import { getAccount, TOKEN_2022_PROGRAM_ID } from "@solana/spl-token";

const ORACLE_PROGRAM_ID = new PublicKey(
  "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop",
);
const CHANNEL_STAKE_POOL_SEED = Buffer.from("channel_pool");
const STAKE_VAULT_SEED = Buffer.from("stake_vault");

/** Old pools to migrate CCM out of (everything except the 14 new lock-tier pools) */
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

function formatCCM(raw: bigint | number): string {
  const n = Number(raw) / 1e9;
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(2)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(2)}K`;
  return n.toFixed(2);
}

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

  console.log("=".repeat(70));
  console.log("  MIGRATION AUDIT — CCM Recovery Analysis");
  console.log("=".repeat(70));
  console.log(`\n  Checking ${OLD_POOLS.length} old pools...\n`);

  let totalVaultBalance = BigInt(0);
  let totalStaked = BigInt(0);
  let totalExcess = BigInt(0);
  let totalRewardRate = 0;
  let poolsWithStake = 0;

  const results: Array<{
    name: string;
    vaultBalance: bigint;
    staked: bigint;
    excess: bigint;
    rewardRate: number;
    stakerCount: number;
    isShutdown: boolean;
  }> = [];

  for (const pool of OLD_POOLS) {
    const channelConfig = new PublicKey(pool.channelConfig);
    const [stakePool] = PublicKey.findProgramAddressSync(
      [CHANNEL_STAKE_POOL_SEED, channelConfig.toBuffer()],
      ORACLE_PROGRAM_ID,
    );
    const [vaultPda] = PublicKey.findProgramAddressSync(
      [STAKE_VAULT_SEED, stakePool.toBuffer()],
      ORACLE_PROGRAM_ID,
    );

    try {
      // Fetch pool state
      const poolData: any =
        await oracleProgram.account.channelStakePool.fetch(stakePool);

      const staked = BigInt(poolData.totalStaked.toString());
      const rewardRate = parseInt(poolData.rewardPerSlot, 10);
      const stakerCount = parseInt(poolData.stakerCount, 10);
      const isShutdown = poolData.isShutdown;

      // Fetch vault token balance
      let vaultBalance = BigInt(0);
      try {
        const vaultAccount = await getAccount(
          connection,
          vaultPda,
          "confirmed",
          TOKEN_2022_PROGRAM_ID,
        );
        vaultBalance = vaultAccount.amount;
      } catch {
        // Vault might not exist
      }

      const excess = vaultBalance > staked ? vaultBalance - staked : BigInt(0);

      results.push({
        name: pool.name,
        vaultBalance,
        staked,
        excess,
        rewardRate,
        stakerCount,
        isShutdown,
      });

      totalVaultBalance += vaultBalance;
      totalStaked += staked;
      totalExcess += excess;
      totalRewardRate += rewardRate;
      if (staked > 0) poolsWithStake++;
    } catch (err: any) {
      console.log(`  ${pool.name.padEnd(18)} ERROR: ${err.message}`);
    }
  }

  // Display results
  console.log(
    `  ${"POOL".padEnd(18)} ${"VAULT BAL".padStart(12)} ${"STAKED".padStart(12)} ${"REWARD EXCESS".padStart(14)} ${"RATE/SLOT".padStart(10)} ${"STAKERS".padStart(8)}`,
  );
  console.log("  " + "-".repeat(76));

  for (const r of results) {
    const status = r.isShutdown ? " [SHUT]" : "";
    console.log(
      `  ${(r.name + status).padEnd(18)} ${formatCCM(r.vaultBalance).padStart(12)} ${formatCCM(r.staked).padStart(12)} ${formatCCM(r.excess).padStart(14)} ${r.rewardRate.toLocaleString().padStart(10)} ${r.stakerCount.toString().padStart(8)}`,
    );
  }

  // Summary
  console.log("\n" + "=".repeat(70));
  console.log("  RECOVERY SUMMARY");
  console.log("=".repeat(70));

  console.log(`\n  Total vault balance:     ${formatCCM(totalVaultBalance)} CCM`);
  console.log(`  Total staked (principal): ${formatCCM(totalStaked)} CCM  <- 100% RECOVERABLE`);
  console.log(`  Total reward excess:      ${formatCCM(totalExcess)} CCM  <- CLAIMABLE (then stuck remainder)`);
  console.log(`  Pools with stakers:       ${poolsWithStake}`);
  console.log(`  Current emission:         ${totalRewardRate.toLocaleString()}/slot`);

  // Daily emission in CCM
  const dailyEmission = (totalRewardRate * 216_000) / 1e9;
  console.log(`  Daily emission:           ${dailyEmission.toFixed(2)} CCM/day`);

  // Estimate how long excess would last at current rates
  if (totalRewardRate > 0 && totalExcess > BigInt(0)) {
    const excessCCM = Number(totalExcess) / 1e9;
    const daysOfRunway = excessCCM / dailyEmission;
    console.log(`  Excess runway:            ${daysOfRunway.toFixed(1)} days at current rate`);
  }

  console.log(`\n  MIGRATION PLAN:`);
  console.log(`  1. All stakers claim_rewards from each pool (recover accumulated CCM)`);
  console.log(`  2. admin_shutdown_pool on all ${OLD_POOLS.length} pools (waives locks, stops emission)`);
  console.log(`  3. All stakers unstake from each pool (recover principal CCM)`);
  console.log(`  4. Remaining vault excess = stuck (no admin withdrawal exists)`);
  console.log();

  // Warn about pools still emitting
  const emittingPools = results.filter((r) => r.rewardRate > 0 && !r.isShutdown);
  if (emittingPools.length > 0) {
    console.log(`  WARNING: ${emittingPools.length} pools still emitting rewards.`);
    console.log(`  Every slot that passes drains more CCM into accumulated rewards.`);
    console.log(`  Shut down sooner = less CCM stuck as unclaimed surplus.\n`);
  }
}

main().catch((err) => {
  console.error("\nError:", err.message || err);
  process.exit(1);
});
