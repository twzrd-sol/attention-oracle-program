/**
 * Check if Oracle stake pool exists for a channel
 */

import { Connection, PublicKey } from "@solana/web3.js";

const ORACLE_PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");

// Trial vault channel configs
const CHANNELS: Record<string, string> = {
  "lofi-vault-3h": "J3HAT4NbL6REyyNqbW1BDGF9BXXc3FYuQ1fr6NbCQaoW",
  "lofi-vault-6h": "dJvatt5bga4ak64ghTLEtxs1jxfLX4TNoZuvfiDCcGy",
  "lofi-vault-9h": "2TWM1H1gHWrA6Ta6A9tH3E1TTTRbPpmSL2Xg7KdHwxCM",
  "lofi-vault-12h": "GZL7vAo9vxdNbsmrreVueVd1Xm9oWmatkQauFcxhq8qP",
};

const CHANNEL_STAKE_POOL_SEED = Buffer.from("channel_pool");

async function main() {
  const connection = new Connection("https://api.mainnet-beta.solana.com", "confirmed");

  console.log("Checking Oracle stake pools for trial vaults...\n");

  for (const [name, configAddr] of Object.entries(CHANNELS)) {
    const channelConfig = new PublicKey(configAddr);

    const [stakePool] = PublicKey.findProgramAddressSync(
      [CHANNEL_STAKE_POOL_SEED, channelConfig.toBuffer()],
      ORACLE_PROGRAM_ID
    );

    const info = await connection.getAccountInfo(stakePool);

    console.log(`${name}:`);
    console.log(`  Channel Config: ${configAddr}`);
    console.log(`  Stake Pool PDA: ${stakePool.toBase58()}`);
    console.log(`  Exists: ${info !== null ? "YES ✅" : "NO ❌"}`);
    if (info) {
      console.log(`  Size: ${info.data.length} bytes`);
    }
    console.log("");
  }
}

main().catch(console.error);
