/**
 * End-to-End Vault Integration Test
 *
 * Tests the complete user flow:
 *   1. Deposit CCM → receive vLOFI shares
 *   2. Wait for compound cycle
 *   3. Verify exchange rate increased (rewards accrued)
 *   4. Redeem vLOFI → receive CCM back
 *
 * This validates:
 *   - Deposit instruction works
 *   - Share calculation is correct
 *   - Compound keeper is operational
 *   - Reward distribution flows to depositors
 *   - Redeem/withdraw works
 *
 * Usage:
 *   # Step 1: Deposit
 *   RPC_URL="..." KEYPAIR=~/.config/solana/id.json \
 *     npx ts-node scripts/test-vault-integration.ts deposit <vault_name> <amount_ccm>
 *
 *   # Step 2: Check status (run anytime)
 *   RPC_URL="..." npx ts-node scripts/test-vault-integration.ts status <vault_name>
 *
 *   # Step 3: Redeem (after observing exchange rate increase)
 *   RPC_URL="..." KEYPAIR=~/.config/solana/id.json \
 *     npx ts-node scripts/test-vault-integration.ts redeem <vault_name> <amount_vlofi>
 *
 * Example:
 *   # Deposit 10 CCM to TWZRD 247 vault
 *   npx ts-node scripts/test-vault-integration.ts deposit "TWZRD 247" 10
 *
 *   # Check status
 *   npx ts-node scripts/test-vault-integration.ts status "TWZRD 247"
 *
 *   # Redeem all shares
 *   npx ts-node scripts/test-vault-integration.ts redeem "TWZRD 247" 10.5
 */

import { Connection, Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { Program, AnchorProvider, Wallet, BN } from "@coral-xyz/anchor";
import {
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
} from "@solana/spl-token";
import * as fs from "fs";

// ============================================================================
// Constants
// ============================================================================

const VAULT_PROGRAM_ID = new PublicKey("5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");

const VAULT_SEED = Buffer.from("vault");
const VLOFI_MINT_SEED = Buffer.from("vlofi");
const CCM_BUFFER_SEED = Buffer.from("ccm_buffer");

// All 16 vaults
const VAULTS = [
  { name: "Lofi 3h", channelConfig: "J3HAT4NbL6REyyNqbW1BDGF9BXXc3FYuQ1fr6NbCQaoW" },
  { name: "Lofi 6h", channelConfig: "dJvatt5bga4ak64ghTLEtxs1jxfLX4TNoZuvfiDCcGy" },
  { name: "Lofi 9h", channelConfig: "2TWM1H1gHWrA6Ta6A9tH3E1TTTRbPpmSL2Xg7KdHwxCM" },
  { name: "Lofi 12h", channelConfig: "GZL7vAo9vxdNbsmrreVueVd1Xm9oWmatkQauFcxhq8qP" },
  { name: "TWZRD 247", channelConfig: "84SxXryEL2dFT5rno9F1SGBAFvvkEDyp3wNQZyxT3hQ9" },
  { name: "TWZRD 1999", channelConfig: "7g1qkWgZkbhZNFgbEzxxvYxCJHt4NMb3fwE2RHyrygDL" },
  { name: "TWZRD 415", channelConfig: "DqoM3QcGPbUD2Hic1fxsSLqZY1CaSDkiaNaas2ufZUpb" },
  { name: "TWZRD 3121", channelConfig: "EADvLuoe6ZXTfVBpVEKAMSfnFr1oZuHMxiButLVMnHuE" },
  { name: "TWZRD 69", channelConfig: "HEa4KgAyuvRZPyAsUPmVTRXiTRuxVEkkGbmtEeybzGB9" },
  { name: "999", channelConfig: "9G1MvnVq3dX6UwGFvhTC9bDArNt9TyvS5UimffTL1BAJ" },
  { name: "212", channelConfig: "Dg84d5BkSYxKSix9m6YgbLz1L7mEsSH81Svp24watxEC" },
  { name: "247", channelConfig: "GdrV9DjKZFePZadxuQANKEBvVaB7rM8aEhMEzMHWrFJE" },
  { name: "1999", channelConfig: "8LCSiL2a4FjTAveMMn8SjLVxrYecWSfFDH48sdhzdbv" },
  { name: "415", channelConfig: "GxzK9iqyFJf3TRJG5XAQJD3eJtgKCivzkQtj7iPKrUsG" },
  { name: "3121", channelConfig: "4JawzmsofxVCim7eDtFPCMwiP21NMcAQqsZRPT7k9uL1" },
  { name: "69", channelConfig: "2uGQDJMsGy3undJCT9NazdJXjSoCcXd71vgkvYzMt3eR" },
];

const CCM_DECIMALS = 9;

// ============================================================================
// Helpers
// ============================================================================

function loadKeypair(path: string): Keypair {
  const expanded = path.replace("~", process.env.HOME || "");
  const raw = JSON.parse(fs.readFileSync(expanded, "utf-8"));
  return Keypair.fromSecretKey(new Uint8Array(raw));
}

function formatCCM(lamports: bigint | BN | number): string {
  const amount = typeof lamports === "number" ? lamports : Number(lamports.toString());
  return (amount / 10 ** CCM_DECIMALS).toLocaleString(undefined, {
    minimumFractionDigits: 2,
    maximumFractionDigits: 4,
  });
}

function findVault(name: string) {
  const vault = VAULTS.find(v => v.name.toLowerCase() === name.toLowerCase());
  if (!vault) {
    console.error(`Vault not found: ${name}`);
    console.error(`Available vaults: ${VAULTS.map(v => v.name).join(", ")}`);
    process.exit(1);
  }
  return vault;
}

// ============================================================================
// Status Check
// ============================================================================

async function checkStatus(
  connection: Connection,
  program: Program,
  vaultName: string,
  userPubkey?: PublicKey,
) {
  const vaultConfig = findVault(vaultName);
  const channelConfig = new PublicKey(vaultConfig.channelConfig);

  const [vault] = PublicKey.findProgramAddressSync(
    [VAULT_SEED, channelConfig.toBuffer()],
    VAULT_PROGRAM_ID,
  );
  const [vlofiMint] = PublicKey.findProgramAddressSync(
    [VLOFI_MINT_SEED, vault.toBuffer()],
    VAULT_PROGRAM_ID,
  );

  console.log("=".repeat(60));
  console.log(`  Vault Status: ${vaultConfig.name}`);
  console.log("=".repeat(60));
  console.log(`  Vault:    ${vault.toBase58()}`);
  console.log(`  vLOFI:    ${vlofiMint.toBase58()}`);
  console.log();

  // Fetch vault state
  const vaultData = await program.account.channelVault.fetch(vault);

  const totalStaked = BigInt(vaultData.totalStaked.toString());
  const totalShares = BigInt(vaultData.totalShares.toString());
  const pendingDeposits = BigInt(vaultData.pendingDeposits.toString());
  const pendingWithdrawals = BigInt(vaultData.pendingWithdrawals.toString());

  let exchangeRate = "1.000000";
  if (totalShares > 0n) {
    const nav = totalStaked + pendingDeposits - pendingWithdrawals;
    const rate = (nav * 1_000_000_000n) / totalShares;
    exchangeRate = (Number(rate) / 1e9).toFixed(6);
  }

  console.log("  Vault State:");
  console.log(`    Total Staked:       ${formatCCM(totalStaked)} CCM`);
  console.log(`    Total Shares:       ${formatCCM(totalShares)} vLOFI`);
  console.log(`    Pending Deposits:   ${formatCCM(pendingDeposits)} CCM`);
  console.log(`    Pending Withdrawals: ${formatCCM(pendingWithdrawals)} CCM`);
  console.log(`    Exchange Rate:      ${exchangeRate} CCM per vLOFI`);
  console.log(`    Paused:             ${vaultData.paused}`);
  console.log(`    Compound Count:     ${vaultData.compoundCount.toString()}`);
  console.log();

  if (userPubkey) {
    const userCcm = getAssociatedTokenAddressSync(CCM_MINT, userPubkey, false, TOKEN_2022_PROGRAM_ID);
    const userVlofi = getAssociatedTokenAddressSync(vlofiMint, userPubkey, false, TOKEN_PROGRAM_ID);

    const [ccmInfo, vlofiInfo] = await Promise.all([
      connection.getTokenAccountBalance(userCcm).catch(() => null),
      connection.getTokenAccountBalance(userVlofi).catch(() => null),
    ]);

    console.log(`  User Balances (${userPubkey.toBase58().slice(0, 8)}...):`);
    console.log(`    CCM:   ${ccmInfo ? formatCCM(ccmInfo.value.amount) : "0.00"} CCM`);
    console.log(`    vLOFI: ${vlofiInfo ? formatCCM(vlofiInfo.value.amount) : "0.00"} vLOFI`);

    if (vlofiInfo && BigInt(vlofiInfo.value.amount) > 0n) {
      const shares = BigInt(vlofiInfo.value.amount);
      const rate = exchangeRate === "1.000000" ? 1n : BigInt(Math.floor(parseFloat(exchangeRate) * 1e9));
      const redeemable = (shares * rate) / 1_000_000_000n;
      console.log(`    Redeemable:        ${formatCCM(redeemable)} CCM (at current rate)`);
    }
    console.log();
  }
}

// ============================================================================
// Deposit
// ============================================================================

async function deposit(
  connection: Connection,
  program: Program,
  user: Keypair,
  vaultName: string,
  amountCcm: number,
) {
  const vaultConfig = findVault(vaultName);
  const channelConfig = new PublicKey(vaultConfig.channelConfig);

  const [vault] = PublicKey.findProgramAddressSync(
    [VAULT_SEED, channelConfig.toBuffer()],
    VAULT_PROGRAM_ID,
  );
  const [vlofiMint] = PublicKey.findProgramAddressSync(
    [VLOFI_MINT_SEED, vault.toBuffer()],
    VAULT_PROGRAM_ID,
  );
  const [ccmBuffer] = PublicKey.findProgramAddressSync(
    [CCM_BUFFER_SEED, vault.toBuffer()],
    VAULT_PROGRAM_ID,
  );

  const userCcm = getAssociatedTokenAddressSync(CCM_MINT, user.publicKey, false, TOKEN_2022_PROGRAM_ID);
  const userVlofi = getAssociatedTokenAddressSync(vlofiMint, user.publicKey, false, TOKEN_PROGRAM_ID);

  console.log("=".repeat(60));
  console.log(`  Deposit to ${vaultConfig.name}`);
  console.log("=".repeat(60));
  console.log(`  User:     ${user.publicKey.toBase58()}`);
  console.log(`  Amount:   ${amountCcm} CCM`);
  console.log(`  Vault:    ${vault.toBase58()}`);
  console.log();

  // Check user CCM balance
  const ccmBalance = await connection.getTokenAccountBalance(userCcm);
  const ccmAmount = BigInt(ccmBalance.value.amount);
  const depositLamports = BigInt(amountCcm * 10 ** CCM_DECIMALS);

  if (ccmAmount < depositLamports) {
    console.error(`Insufficient CCM balance: ${formatCCM(ccmAmount)} (need ${formatCCM(depositLamports)})`);
    process.exit(1);
  }

  // Check if user vLOFI ATA exists, create if not
  const vlofiInfo = await connection.getAccountInfo(userVlofi);
  const tx = new Transaction();

  if (!vlofiInfo) {
    console.log("  Creating vLOFI token account...");
    tx.add(
      createAssociatedTokenAccountInstruction(
        user.publicKey,
        userVlofi,
        user.publicKey,
        vlofiMint,
        TOKEN_PROGRAM_ID,
      ),
    );
  }

  // Fetch vault state for min shares calculation
  const vaultData = await program.account.channelVault.fetch(vault);
  const totalStaked = BigInt(vaultData.totalStaked.toString());
  const totalShares = BigInt(vaultData.totalShares.toString());
  const pendingDeposits = BigInt(vaultData.pendingDeposits.toString());

  // Calculate expected shares (with 1% slippage tolerance)
  let expectedShares = depositLamports;
  if (totalShares > 0n) {
    const nav = totalStaked + pendingDeposits;
    expectedShares = (depositLamports * totalShares) / nav;
  }
  const minShares = (expectedShares * 99n) / 100n; // 1% slippage

  console.log(`  Expected shares: ${formatCCM(expectedShares)} vLOFI`);
  console.log(`  Min shares:      ${formatCCM(minShares)} vLOFI (1% slippage)`);
  console.log();

  // Build deposit instruction
  const depositIx = await program.methods
    .deposit(new BN(depositLamports.toString()), new BN(minShares.toString()))
    .accounts({
      user: user.publicKey,
      vault,
      ccmMint: CCM_MINT,
      vlofiMint,
      userCcm,
      userVlofi,
      ccmBuffer,
      tokenProgram: TOKEN_PROGRAM_ID,
      token2022Program: TOKEN_2022_PROGRAM_ID,
    })
    .instruction();

  tx.add(depositIx);

  console.log("  Sending deposit transaction...");
  const sig = await connection.sendTransaction(tx, [user], { skipPreflight: false });
  console.log(`  TX: ${sig}`);

  await connection.confirmTransaction(sig, "confirmed");
  console.log(`  Confirmed!`);
  console.log();

  // Show updated balances
  await checkStatus(connection, program, vaultName, user.publicKey);

  console.log("=".repeat(60));
  console.log("  Next Steps:");
  console.log("=".repeat(60));
  console.log(`  1. Wait for compound cycle (runs every 1 hour)`);
  console.log(`  2. Check status again:`);
  console.log(`     npx ts-node scripts/test-vault-integration.ts status "${vaultName}"`);
  console.log(`  3. Look for exchange rate increase (proves rewards accrued)`);
  console.log(`  4. Redeem when ready:`);
  console.log(`     npx ts-node scripts/test-vault-integration.ts redeem "${vaultName}" <amount_vlofi>`);
  console.log();
}

// ============================================================================
// Redeem
// ============================================================================

async function redeem(
  connection: Connection,
  program: Program,
  user: Keypair,
  vaultName: string,
  amountVlofi: number,
) {
  const vaultConfig = findVault(vaultName);
  const channelConfig = new PublicKey(vaultConfig.channelConfig);

  const [vault] = PublicKey.findProgramAddressSync(
    [VAULT_SEED, channelConfig.toBuffer()],
    VAULT_PROGRAM_ID,
  );
  const [vlofiMint] = PublicKey.findProgramAddressSync(
    [VLOFI_MINT_SEED, vault.toBuffer()],
    VAULT_PROGRAM_ID,
  );
  const [ccmBuffer] = PublicKey.findProgramAddressSync(
    [CCM_BUFFER_SEED, vault.toBuffer()],
    VAULT_PROGRAM_ID,
  );

  const userCcm = getAssociatedTokenAddressSync(CCM_MINT, user.publicKey, false, TOKEN_2022_PROGRAM_ID);
  const userVlofi = getAssociatedTokenAddressSync(vlofiMint, user.publicKey, false, TOKEN_PROGRAM_ID);

  console.log("=".repeat(60));
  console.log(`  Redeem from ${vaultConfig.name}`);
  console.log("=".repeat(60));
  console.log(`  User:     ${user.publicKey.toBase58()}`);
  console.log(`  Amount:   ${amountVlofi} vLOFI`);
  console.log();

  const redeemLamports = BigInt(Math.floor(amountVlofi * 10 ** CCM_DECIMALS));

  // Fetch vault state for CCM calculation
  const vaultData = await program.account.channelVault.fetch(vault);
  const totalStaked = BigInt(vaultData.totalStaked.toString());
  const totalShares = BigInt(vaultData.totalShares.toString());
  const emergencyReserve = BigInt(vaultData.emergencyReserve.toString());

  let expectedCcm = redeemLamports;
  if (totalShares > 0n) {
    const nav = totalStaked + emergencyReserve;
    expectedCcm = (redeemLamports * nav) / totalShares;
  }

  // Use instant redeem (takes from emergency reserve, 5% penalty)
  const penalty = (expectedCcm * 5n) / 100n;
  const netCcm = expectedCcm - penalty;
  const minCcm = (netCcm * 99n) / 100n; // 1% slippage

  console.log(`  Expected CCM:    ${formatCCM(expectedCcm)} CCM`);
  console.log(`  Instant penalty: ${formatCCM(penalty)} CCM (5%)`);
  console.log(`  Net CCM:         ${formatCCM(netCcm)} CCM`);
  console.log(`  Min CCM:         ${formatCCM(minCcm)} CCM (1% slippage)`);
  console.log();

  // Build instant redeem instruction
  const redeemIx = await program.methods
    .instantRedeem(new BN(redeemLamports.toString()), new BN(minCcm.toString()))
    .accounts({
      user: user.publicKey,
      vault,
      ccmMint: CCM_MINT,
      vlofiMint,
      userCcm,
      userVlofi,
      ccmBuffer,
      tokenProgram: TOKEN_PROGRAM_ID,
      token2022Program: TOKEN_2022_PROGRAM_ID,
    })
    .instruction();

  const tx = new Transaction().add(redeemIx);

  console.log("  Sending redeem transaction...");
  const sig = await connection.sendTransaction(tx, [user], { skipPreflight: false });
  console.log(`  TX: ${sig}`);

  await connection.confirmTransaction(sig, "confirmed");
  console.log(`  Confirmed!`);
  console.log();

  // Show updated balances
  await checkStatus(connection, program, vaultName, user.publicKey);
}

// ============================================================================
// Main
// ============================================================================

async function main() {
  const rpcUrl = process.env.RPC_URL;
  if (!rpcUrl) {
    console.error("ERROR: RPC_URL required");
    process.exit(1);
  }

  const args = process.argv.slice(2);
  if (args.length < 2) {
    console.error("Usage:");
    console.error("  deposit <vault_name> <amount_ccm>");
    console.error("  status <vault_name>");
    console.error("  redeem <vault_name> <amount_vlofi>");
    process.exit(1);
  }

  const command = args[0];
  const vaultName = args[1];

  const connection = new Connection(rpcUrl, "confirmed");

  // Load program
  const idl = await Program.fetchIdl(VAULT_PROGRAM_ID, new AnchorProvider(
    connection,
    new Wallet(Keypair.generate()),
    { commitment: "confirmed" },
  ));
  if (!idl) {
    console.error("ERROR: Vault program IDL not found");
    process.exit(1);
  }

  const dummyWallet = new Wallet(Keypair.generate());
  const provider = new AnchorProvider(connection, dummyWallet, { commitment: "confirmed" });
  const program = new Program(idl, provider);

  if (command === "status") {
    const keypairPath = process.env.KEYPAIR;
    const user = keypairPath ? loadKeypair(keypairPath) : undefined;
    await checkStatus(connection, program, vaultName, user?.publicKey);
  } else if (command === "deposit") {
    const amount = parseFloat(args[2]);
    if (isNaN(amount) || amount <= 0) {
      console.error("ERROR: Invalid amount");
      process.exit(1);
    }

    const keypairPath = process.env.KEYPAIR;
    if (!keypairPath) {
      console.error("ERROR: KEYPAIR required for deposit");
      process.exit(1);
    }

    const user = loadKeypair(keypairPath);
    await deposit(connection, program, user, vaultName, amount);
  } else if (command === "redeem") {
    const amount = parseFloat(args[2]);
    if (isNaN(amount) || amount <= 0) {
      console.error("ERROR: Invalid amount");
      process.exit(1);
    }

    const keypairPath = process.env.KEYPAIR;
    if (!keypairPath) {
      console.error("ERROR: KEYPAIR required for redeem");
      process.exit(1);
    }

    const user = loadKeypair(keypairPath);
    await redeem(connection, program, user, vaultName, amount);
  } else {
    console.error(`Unknown command: ${command}`);
    process.exit(1);
  }
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
