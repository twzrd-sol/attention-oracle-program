import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, PublicKey } from "@solana/web3.js";
import { CHANNELS } from "../keepers/lib/channels.js";

const ORACLE_PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const CHANNEL_STAKE_POOL_SEED = Buffer.from("channel_pool");

async function main() {
  const rpcUrl = process.env.RPC_URL;
  if (!rpcUrl) throw new Error("Set RPC_URL");

  const connection = new Connection(rpcUrl, "confirmed");
  const wallet = new anchor.Wallet(anchor.web3.Keypair.generate());
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed" });
  const oracleIdl = await Program.fetchIdl(ORACLE_PROGRAM_ID, provider);
  if (!oracleIdl) throw new Error("Failed to fetch Oracle IDL");
  const oracleProgram = new Program(oracleIdl, provider);

  console.log("=".repeat(70));
  console.log("ON-CHAIN CHANNEL AUDIT");
  console.log("=".repeat(70));
  console.log(`\nSource: scripts/keepers/lib/channels.ts (${CHANNELS.length} channels)\n`);

  const channelData: Array<{
    name: string;
    lockDuration: number;
    rewardRate: number;
    totalWeighted: number;
    pubkey: string;
  }> = [];

  for (const channel of CHANNELS) {
    const channelConfig = new PublicKey(channel.channelConfig);
    const [stakePool] = PublicKey.findProgramAddressSync(
      [CHANNEL_STAKE_POOL_SEED, channelConfig.toBuffer()],
      ORACLE_PROGRAM_ID
    );

    try {
      const poolData: any = await oracleProgram.account.channelStakePool.fetch(stakePool);

      // Lock duration comes from CHANNELS config, not on-chain
      const lockDuration = channel.lockDurationSlots;

      // poolData values are decimal strings, not hex
      const rewardRate = parseInt(poolData.rewardPerSlot, 10);
      const totalWeighted = parseInt(poolData.totalWeighted, 10);

      channelData.push({
        name: channel.name,
        lockDuration,
        rewardRate,
        totalWeighted,
        pubkey: stakePool.toString(),
      });
    } catch (err: any) {
      // Account might not exist on-chain yet
      console.log(`  ⚠️  ${channel.name}: ${err.message || "fetch failed"}`);
    }
  }

  // Group by category
  const categories = {
    lofi: channelData.filter((ch) => ch.name.startsWith("lofi")),
    audio: channelData.filter((ch) => ch.name.startsWith("audio")),
    twzrd: channelData.filter((ch) => ch.name.startsWith("twzrd")),
    other: channelData.filter(
      (ch) =>
        !ch.name.startsWith("lofi") &&
        !ch.name.startsWith("audio") &&
        !ch.name.startsWith("twzrd")
    ),
  };

  for (const [category, pools] of Object.entries(categories)) {
    if (pools.length === 0) continue;

    console.log(`\n${category.toUpperCase()} (${pools.length} pools):`);
    console.log("-".repeat(70));

    for (const pool of pools) {
      // Convert slots to hours: ~7200 slots/hour (2 slots/sec × 3600 sec/hour)
      const lockHours = (pool.lockDuration / 7200).toFixed(1);
      const weightedBillions = (pool.totalWeighted / 1e9).toFixed(1);
      const active = pool.rewardRate > 0 ? "✅" : "💀";

      console.log(`\n  ${active} ${pool.name}`);
      console.log(`     Lock:     ${lockHours}h`);
      console.log(`     Reward:   ${pool.rewardRate.toLocaleString()} per slot`);
      console.log(`     Staked:   ${weightedBillions}B weighted`);
    }
  }

  // Summary
  console.log("\n" + "=".repeat(70));
  console.log("SUMMARY");
  console.log("=".repeat(70));
  console.log(`Total pools:     ${channelData.length}`);
  console.log(`  - Lofi:        ${categories.lofi.length}`);
  console.log(`  - Audio:       ${categories.audio.length}`);
  console.log(`  - TWZRD:       ${categories.twzrd.length}`);
  console.log(`  - Other:       ${categories.other.length}`);

  const totalStake = channelData.reduce((sum, ch) => sum + ch.totalWeighted, 0);
  const totalRewardRate = channelData.reduce((sum, ch) => sum + ch.rewardRate, 0);
  const activeCount = channelData.filter((ch) => ch.rewardRate > 0).length;

  console.log(`\nTotal weighted stake: ${(totalStake / 1e9).toFixed(1)}B`);
  console.log(`Active pools:         ${activeCount}/${channelData.length}`);
  console.log(`Total reward rate:    ${totalRewardRate.toLocaleString()} per slot`);
  console.log(`Daily emission:       ${((totalRewardRate * 216000) / 1e9).toFixed(2)} CCM/day`);

  // Lock duration distribution
  const lockCounts = new Map<string, number>();
  channelData.forEach((ch) => {
    const hours = (ch.lockDuration / 7200).toFixed(1) + "h";
    lockCounts.set(hours, (lockCounts.get(hours) || 0) + 1);
  });

  console.log(`\nLock duration distribution:`);
  for (const [hours, count] of Array.from(lockCounts.entries()).sort((a, b) => parseFloat(a) - parseFloat(b))) {
    console.log(`  ${hours}: ${count} pools`);
  }

  // Dead pools (zero stake, zero rewards)
  const deadPools = channelData.filter((ch) => ch.totalWeighted === 0 && ch.rewardRate === 0);
  if (deadPools.length > 0) {
    console.log(`\nDead pools (0 stake, 0 rewards): ${deadPools.length}`);
    deadPools.forEach((ch) => console.log(`  💀 ${ch.name}`));
  }
}

main().catch(console.error);
