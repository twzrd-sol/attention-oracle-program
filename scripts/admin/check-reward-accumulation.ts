import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, PublicKey } from "@solana/web3.js";

const ORACLE_PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const CHANNEL_STAKE_POOL_SEED = Buffer.from("channel_pool");
const CHANNEL_CONFIG = new PublicKey("84SxXryEL2dFT5rno9F1SGBAFvvkEDyp3wNQZyxT3hQ9");

(async () => {
  const rpcUrl = process.env.RPC_URL;
  if (!rpcUrl) throw new Error("Set RPC_URL");
  
  const connection = new Connection(rpcUrl, "confirmed");
  const wallet = new anchor.Wallet(anchor.web3.Keypair.generate());
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed" });
  const oracleIdl = await Program.fetchIdl(ORACLE_PROGRAM_ID, provider);
  const oracleProgram = new Program(oracleIdl!, provider);

  const [stakePool] = PublicKey.findProgramAddressSync(
    [CHANNEL_STAKE_POOL_SEED, CHANNEL_CONFIG.toBuffer()],
    ORACLE_PROGRAM_ID
  );

  console.log("\n=== twzrd-247-6h Reward Accumulation ===\n");
  
  const poolData: any = await oracleProgram.account.channelStakePool.fetch(stakePool);
  const currentSlot = await connection.getSlot("confirmed");
  
  const rewardPerSlot = Number(poolData.rewardPerSlot.toString());
  const totalWeighted = Number(poolData.totalWeighted.toString());
  const accRewardPerShare = Number(poolData.accRewardPerShare.toString());
  const lastUpdateSlot = Number(poolData.lastRewardSlot.toString());
  
  console.log("Reward per slot:", rewardPerSlot);
  console.log("Total weighted:", (totalWeighted / 1e9).toFixed(1), "B");
  console.log("Acc reward/share:", accRewardPerShare);
  console.log("Last update slot:", lastUpdateSlot);
  console.log("Current slot:", currentSlot);
  console.log("Slots passed:", currentSlot - lastUpdateSlot);
  
  const slotsPassed = currentSlot - lastUpdateSlot;
  const totalReward = slotsPassed * rewardPerSlot;
  const rewardIncrease = totalWeighted > 0 ? Math.floor((totalReward * 1e9) / totalWeighted) : 0;
  
  console.log("\nProjected accumulation:");
  console.log("  Total reward:", totalReward.toLocaleString());
  console.log("  Reward increase:", rewardIncrease);
  console.log("  New acc/share:", accRewardPerShare + rewardIncrease);
})();
