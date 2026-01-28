/**
 * Test compound on trial vaults
 * Usage: VAULT=lofi-vault-3h npx ts-node scripts/test-compound.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";

// Program IDs
const VAULT_PROGRAM_ID = new PublicKey("5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ");
const ORACLE_PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");
const PROTOCOL_STATE = new PublicKey("596VBoVvzASAhe38CcBSJnv1LdVFPu4EdB8gw1Ko2nx3");

// Seeds
const CHANNEL_STAKE_POOL_SEED = Buffer.from("channel_pool");
const STAKE_VAULT_SEED = Buffer.from("stake_vault");
const CHANNEL_USER_STAKE_SEED = Buffer.from("channel_user");
const STAKE_NFT_MINT_SEED = Buffer.from("stake_nft");
const VAULT_CCM_BUFFER_SEED = Buffer.from("vault_ccm");
const VAULT_ORACLE_POSITION_SEED = Buffer.from("vault_oracle");

// Trial vault configs
const TRIAL_VAULTS: Record<string, { channelConfig: string; vault: string }> = {
  "lofi-vault-3h": {
    channelConfig: "J3HAT4NbL6REyyNqbW1BDGF9BXXc3FYuQ1fr6NbCQaoW",
    vault: "7tjCgZcsK4sgV65wsNajUVRuGHQ7GZELWfTaWYbCBDTw",
  },
  "lofi-vault-6h": {
    channelConfig: "dJvatt5bga4ak64ghTLEtxs1jxfLX4TNoZuvfiDCcGy",
    vault: "3BumiGZYw96eiyHEjy3wkjnrBTgcUspYmFHHptMpHof9",
  },
  "lofi-vault-9h": {
    channelConfig: "2TWM1H1gHWrA6Ta6A9tH3E1TTTRbPpmSL2Xg7KdHwxCM",
    vault: "BnN5JfewvFZ93RFsduKyYbBc3NYvVc4xuYRDsMptEWu8",
  },
  "lofi-vault-12h": {
    channelConfig: "GZL7vAo9vxdNbsmrreVueVd1Xm9oWmatkQauFcxhq8qP",
    vault: "8j7M2aQg7FdaN6dTW33km2zfJX5USVqQwSZ2WPA4kaPz",
  },
};

async function main() {
  const vaultName = process.env.VAULT || "lofi-vault-3h";
  const config = TRIAL_VAULTS[vaultName];
  if (!config) {
    console.error("Unknown vault:", vaultName);
    process.exit(1);
  }

  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const idl = await Program.fetchIdl(VAULT_PROGRAM_ID, provider);
  if (!idl) throw new Error("Vault IDL not found");
  const program = new Program(idl, provider);

  const payer = provider.wallet.publicKey;
  const channelConfig = new PublicKey(config.channelConfig);
  const vault = new PublicKey(config.vault);

  console.log("🔄 COMPOUND TEST");
  console.log("=================");
  console.log("Vault:", vaultName);
  console.log("Payer:", payer.toBase58());

  // Derive Oracle PDAs
  const [stakePool] = PublicKey.findProgramAddressSync(
    [CHANNEL_STAKE_POOL_SEED, channelConfig.toBuffer()],
    ORACLE_PROGRAM_ID
  );

  const [oracleVault] = PublicKey.findProgramAddressSync(
    [STAKE_VAULT_SEED, stakePool.toBuffer()],
    ORACLE_PROGRAM_ID
  );

  const [userStake] = PublicKey.findProgramAddressSync(
    [CHANNEL_USER_STAKE_SEED, channelConfig.toBuffer(), vault.toBuffer()],
    ORACLE_PROGRAM_ID
  );

  const [nftMint] = PublicKey.findProgramAddressSync(
    [STAKE_NFT_MINT_SEED, stakePool.toBuffer(), vault.toBuffer()],
    ORACLE_PROGRAM_ID
  );

  // Derive vault-side PDAs
  const [ccmBuffer] = PublicKey.findProgramAddressSync(
    [VAULT_CCM_BUFFER_SEED, vault.toBuffer()],
    VAULT_PROGRAM_ID
  );

  const [vaultOraclePosition] = PublicKey.findProgramAddressSync(
    [VAULT_ORACLE_POSITION_SEED, vault.toBuffer()],
    VAULT_PROGRAM_ID
  );

  // Vault's NFT ATA
  const { getAssociatedTokenAddressSync } = await import("@solana/spl-token");
  const vaultNftAta = getAssociatedTokenAddressSync(nftMint, vault, true, TOKEN_2022_PROGRAM_ID);

  console.log("\nAddresses:");
  console.log("  Channel Config:", channelConfig.toBase58());
  console.log("  Vault:", vault.toBase58());
  console.log("  Stake Pool:", stakePool.toBase58());
  console.log("  Oracle Vault:", oracleVault.toBase58());
  console.log("  User Stake:", userStake.toBase58());
  console.log("  NFT Mint:", nftMint.toBase58());
  console.log("  Vault NFT ATA:", vaultNftAta.toBase58());

  // Check vault state
  try {
    const vaultData = await provider.connection.getAccountInfo(vault);
    if (!vaultData) throw new Error("Vault not found");

    // Parse pending_deposits (offset: 8+1+1+32*4+8+8 = 146)
    const pendingDeposits = vaultData.data.readBigUInt64LE(8 + 1 + 1 + 32*4 + 8 + 8);
    const pendingWithdrawals = vaultData.data.readBigUInt64LE(8 + 1 + 1 + 32*4 + 8 + 8 + 8);
    const stakeable = Number(pendingDeposits) - Number(pendingWithdrawals);

    console.log("\nVault State:");
    console.log("  Pending Deposits:", Number(pendingDeposits) / 1e9, "CCM");
    console.log("  Pending Withdrawals:", Number(pendingWithdrawals) / 1e9, "CCM");
    console.log("  Stakeable:", stakeable / 1e9, "CCM");

    if (stakeable <= 0) {
      console.log("\n⚠️  No stakeable deposits (all reserved for withdrawals)");
      return;
    }
  } catch (e: any) {
    console.error("Failed to read vault:", e.message);
  }

  console.log("\n📡 Calling compound...");

  try {
    const tx = await program.methods
      .compound()
      .accounts({
        payer: payer,
        vault: vault,
        vaultOraclePosition: vaultOraclePosition,
        vaultCcmBuffer: ccmBuffer,
        ccmMint: CCM_MINT,
        oracleProgram: ORACLE_PROGRAM_ID,
        oracleProtocol: PROTOCOL_STATE,
        oracleChannelConfig: channelConfig,
        oracleStakePool: stakePool,
        oracleVault: oracleVault,
        oracleUserStake: userStake,
        oracleNftMint: nftMint,
        vaultNftAta: vaultNftAta,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    console.log("\n✅ COMPOUND SUCCESSFUL!");
    console.log("Signature:", tx);
    console.log("View: https://solscan.io/tx/" + tx);
  } catch (e: any) {
    console.error("\n❌ Compound failed:", e.message || e);
    if (e.logs) {
      console.log("\nLogs:");
      e.logs.slice(-15).forEach((log: string) => console.log("  ", log));
    }
  }
}

main();
