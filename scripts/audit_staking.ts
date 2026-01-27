import { Connection, PublicKey } from "@solana/web3.js";

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");
const STAKE_POOL_SEED = Buffer.from("stake_pool");
const STAKE_VAULT_SEED = Buffer.from("stake_vault");

async function main() {
  const rpcUrl = process.env.SOLANA_RPC_URL || "https://api.mainnet-beta.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");

  console.log("=== Staking Pool Audit ===\n");

  // Derive StakePool PDA
  const [stakePool] = PublicKey.findProgramAddressSync(
    [STAKE_POOL_SEED, CCM_MINT.toBuffer()],
    PROGRAM_ID
  );

  // Derive Stake Vault PDA
  const [stakeVault] = PublicKey.findProgramAddressSync(
    [STAKE_VAULT_SEED, CCM_MINT.toBuffer()],
    PROGRAM_ID
  );

  console.log("StakePool PDA: " + stakePool.toBase58());
  console.log("StakeVault PDA: " + stakeVault.toBase58());

  // Check StakePool account
  const poolInfo = await connection.getAccountInfo(stakePool);
  if (!poolInfo) {
    console.log("\nStakePool: NOT INITIALIZED");
    return;
  }

  console.log("\nStakePool: EXISTS (" + (poolInfo.lamports / 1e9).toFixed(4) + " SOL rent)");

  // Parse StakePool data
  // Skip 8-byte discriminator
  const data = poolInfo.data;
  let offset = 8;

  const version = data.readUInt8(offset); offset += 1;
  const bump = data.readUInt8(offset); offset += 1;
  const mint = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const authority = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const totalStaked = data.readBigUInt64LE(offset); offset += 8;
  const totalWeightedStake = data.readBigUInt64LE(offset); offset += 8;
  const rewardRate = data.readBigUInt64LE(offset); offset += 8;
  const lastUpdateSlot = data.readBigUInt64LE(offset); offset += 8;
  const accRewardPerShare = data.readBigUInt64LE(offset); offset += 8;

  console.log("\n--- StakePool State ---");
  console.log("Version: " + version);
  console.log("Authority: " + authority.toBase58());
  console.log("Total Staked: " + (Number(totalStaked) / 1e9).toLocaleString() + " CCM");
  console.log("Total Weighted Stake: " + (Number(totalWeightedStake) / 1e9).toLocaleString() + " CCM");
  console.log("Reward Rate: " + (Number(rewardRate) / 1e9).toLocaleString() + " CCM/second");
  console.log("Last Update Slot: " + lastUpdateSlot.toString());
  console.log("Acc Reward Per Share: " + accRewardPerShare.toString());

  // Check vault balance
  const vaultInfo = await connection.getAccountInfo(stakeVault);
  if (vaultInfo) {
    // Token account data layout: 36 bytes before amount (8 for mint, 32 for owner, then u64 amount)
    // Actually for Token-2022 ATA, layout might differ. Let's use getTokenAccountBalance instead.
    try {
      const balance = await connection.getTokenAccountBalance(stakeVault);
      console.log("\n--- Stake Vault ---");
      console.log("Vault Balance: " + (Number(balance.value.amount) / 1e9).toLocaleString() + " CCM");
    } catch (e) {
      console.log("\nStake Vault: Error reading balance");
    }
  } else {
    console.log("\nStake Vault: NOT INITIALIZED");
  }

  // Search for UserStake accounts
  console.log("\n--- Searching for UserStake accounts ---");
  const USER_STAKE_SEED = Buffer.from("user_stake");

  // We can't enumerate all UserStake accounts easily without getProgramAccounts
  // Let's check if any exist by looking at known wallets
  const knownWallets = [
    "CSqL9UjtTKc3pFVkt7FFsCJbWKpwxfJZcycpgWeVVTTJ", // zohaibmohd
    "DDs2M6rMALbHSMmBUV3GZeb1W1B2TJnA13KMj8GA9TXe", // ecosystem agent
    "2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD", // upgrade authority
  ];

  for (const walletStr of knownWallets) {
    const wallet = new PublicKey(walletStr);
    const [userStake] = PublicKey.findProgramAddressSync(
      [USER_STAKE_SEED, wallet.toBuffer(), CCM_MINT.toBuffer()],
      PROGRAM_ID
    );
    const info = await connection.getAccountInfo(userStake);
    if (info) {
      console.log("  " + walletStr.slice(0, 8) + "...: HAS STAKE");
    }
  }

  console.log("\n(Full enumeration requires getProgramAccounts with memcmp filter)");
}

main().catch(console.error);
