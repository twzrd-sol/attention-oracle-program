/**
 * Set reward rate for a channel stake pool
 *
 * Usage: CHANNEL=lofi-vault-3h RATE=15200000000 npx ts-node scripts/set-reward-rate.ts
 *
 * Rate calculation for target APR:
 *   slots_per_year = 365 * 24 * 60 * 60 / 0.4 = 78,840,000
 *   rate_per_slot = (target_apr * total_staked) / slots_per_year
 *
 * Example: 12% APR on 10M CCM staked
 *   rate = (0.12 * 10_000_000 * 1e9) / 78_840_000 = 15,220,700,152
 *   ~15.2 CCM per slot (in base units with 9 decimals)
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, SystemProgram } from "@solana/web3.js";
import BN from "bn.js";

const ORACLE_PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const CHANNEL_STAKE_POOL_SEED = Buffer.from("channel_pool");

// Channel config pubkeys keyed by human-readable name.
// Add your channel configs here after running initialize_channel_cumulative.
const CHANNELS: Record<string, string> = {
  // Example:
  // "my-channel-6h": "<channel_config_pubkey>",
};

async function main() {
  const channelName = process.env.CHANNEL;
  const rateStr = process.env.RATE;

  if (!channelName || !CHANNELS[channelName]) {
    console.error("Usage: CHANNEL=lofi-vault-3h RATE=15200000000 npx ts-node scripts/set-reward-rate.ts");
    console.error("Available channels:", Object.keys(CHANNELS).join(", "));
    process.exit(1);
  }

  if (!rateStr) {
    console.error("RATE is required (reward per slot in base units)");
    console.error("Example: RATE=15200000000 for ~12% APR on 10M CCM");
    process.exit(1);
  }

  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const idl = await Program.fetchIdl(ORACLE_PROGRAM_ID, provider);
  if (!idl) throw new Error("Oracle IDL not found");
  const program = new Program(idl, provider);

  const admin = provider.wallet.publicKey;
  const channelConfig = new PublicKey(CHANNELS[channelName]);
  const newRate = new BN(rateStr);

  // Derive stake pool PDA
  const [stakePool] = PublicKey.findProgramAddressSync(
    [CHANNEL_STAKE_POOL_SEED, channelConfig.toBuffer()],
    ORACLE_PROGRAM_ID
  );

  console.log("\n📊 SET REWARD RATE");
  console.log("==================");
  console.log("Channel:", channelName);
  console.log("Channel Config:", channelConfig.toBase58());
  console.log("Stake Pool:", stakePool.toBase58());
  console.log("Admin:", admin.toBase58());
  console.log("New Rate:", newRate.toString(), "per slot");

  // Calculate approximate APR for display
  const SLOTS_PER_YEAR = 78_840_000;
  const rateNum = Number(newRate.toString());
  // Assuming ~10M CCM staked for display purposes
  const estimatedApr = (rateNum * SLOTS_PER_YEAR) / (10_000_000 * 1e9) * 100;
  console.log(`Estimated APR (at 10M TVL): ${estimatedApr.toFixed(2)}%`);

  try {
    const tx = await program.methods
      .setRewardRate(newRate)
      .accounts({
        admin: admin,
        channelConfig: channelConfig,
        stakePool: stakePool,
      })
      .rpc();

    console.log("\n✅ REWARD RATE SET!");
    console.log("Signature:", tx);
    console.log("View: https://solscan.io/tx/" + tx);
  } catch (e: any) {
    console.error("\n❌ Failed:", e.message || e);
    if (e.logs) {
      console.log("\nLogs:");
      e.logs.slice(-10).forEach((log: string) => console.log("  ", log));
    }
  }
}

main().catch(console.error);
