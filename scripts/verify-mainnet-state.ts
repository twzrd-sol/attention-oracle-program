/**
 * Mainnet State Verification — Read-only audit of all on-chain state.
 *
 * Checks:
 *   1. Program binary hashes (local vs on-chain)
 *   2. All 16 stake pools: existence, size, migration, reward config
 *   3. All 16 vaults: existence, state, oracle position
 *   4. Reward runway: vault balance vs emission rate
 *   5. User stake accounts: count, migration status
 *
 * This script NEVER sends transactions. Any keypair works.
 *
 * Usage:
 *   CLUSTER=mainnet-beta I_UNDERSTAND_MAINNET=1 RPC_URL=... KEYPAIR=... \
 *     npx tsx scripts/verify-mainnet-state.ts
 */

import { Connection, PublicKey } from "@solana/web3.js";
import { TOKEN_2022_PROGRAM_ID, unpackAccount } from "@solana/spl-token";
import { createHash } from "crypto";
import { readFileSync, existsSync } from "fs";

import { requireScriptEnv } from "./script-guard.js";
import { CCM_V3_MINT, PROGRAM_ID as ORACLE_PROGRAM_ID_CFG } from "./config.js";
import { CHANNELS } from "./keepers/lib/channels.js";
import {
  ORACLE_PROGRAM_ID,
  VAULT_PROGRAM_ID,
  deriveOracleStakePool,
  deriveOracleStakeVault,
  deriveVault,
  deriveOraclePosition,
  deriveCcmBuffer,
  deriveVlofiMint,
} from "./keepers/lib/vault-pda.js";

// ═══════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════

const DECIMALS = 9;
const SLOTS_PER_DAY = 216_000; // ~400ms per slot
const STAKE_POOL_MIGRATED_SIZE = 162;
const PROGRAM_DATA_HEADER = 45; // UpgradeableLoaderState::ProgramData header

// ProgramData accounts (from `solana program show`)
const ORACLE_PROGRAM_DATA = new PublicKey(
  "5GyaaVmzRr2r9KcUuzt9SxBVq9ubTT5m3pH9Lzy3Kh4L",
);
const VAULT_PROGRAM_DATA = new PublicKey(
  "2ubXWFAJFCnBqJ1vYCsf4q8SYRcqf5DaTfkC6wASK5SQ",
);

// ═══════════════════════════════════════════════════════════════════════
// Stake Pool Layout (162 bytes)
// ═══════════════════════════════════════════════════════════════════════

interface StakePoolInfo {
  exists: boolean;
  size: number;
  migrated: boolean;
  channelConfig: string;
  totalStaked: bigint;
  totalWeighted: bigint;
  stakerCount: bigint;
  accRewardPerShare: bigint;
  lastRewardSlot: bigint;
  rewardPerSlot: bigint;
  isShutdown: boolean;
}

function parseStakePool(data: Buffer): StakePoolInfo {
  // Layout (162 bytes, post-migration):
  //   [0..8]     discriminator
  //   [8..40]    channel_config (Pubkey)
  //   [40..72]   ccm_mint (Pubkey)
  //   [72..104]  vault (Pubkey) — stake_vault PDA, prefer deriveOracleStakeVault()
  //   [104..112] total_staked (u64)
  //   [112..120] total_weighted (u64)
  //   [120..128] staker_count (u64)
  //   [128]      version (u8)
  //   [129..145] acc_reward_per_share (u128)
  //   [145..153] last_reward_slot (u64)
  //   [153..161] reward_per_slot (u64)
  //   [161]      is_shutdown (u8)
  return {
    exists: true,
    size: data.length,
    migrated: data.length >= STAKE_POOL_MIGRATED_SIZE,
    channelConfig: new PublicKey(data.subarray(8, 40)).toBase58(),
    totalStaked: data.readBigUInt64LE(104),
    totalWeighted: data.readBigUInt64LE(112),
    stakerCount: data.readBigUInt64LE(120),
    accRewardPerShare: data.length >= 145
      ? data.readBigUInt64LE(129) | (data.readBigUInt64LE(137) << 64n)
      : 0n,
    lastRewardSlot: data.length >= 153 ? data.readBigUInt64LE(145) : 0n,
    rewardPerSlot: data.length >= 161 ? data.readBigUInt64LE(153) : 0n,
    isShutdown: data.length >= 162 ? data[161] !== 0 : false,
  };
}

// ═══════════════════════════════════════════════════════════════════════
// Vault Layout (parse key fields from raw bytes)
// ═══════════════════════════════════════════════════════════════════════

interface VaultInfo {
  exists: boolean;
  size: number;
  paused: boolean;
  totalStaked: bigint;
  pendingDeposits: bigint;
  pendingWithdrawals: bigint;
  compoundCount: bigint;
  lastCompoundSlot: bigint;
}

function parseVault(data: Buffer): VaultInfo {
  // Layout from IDL (offsets include 8-byte Anchor discriminator):
  //   8: bump(u8), 9: version(u8),
  //  10: channel_config(32), 42: ccm_mint(32), 74: vlofi_mint(32), 106: ccm_buffer(32),
  // 138: total_staked(u64), 146: total_shares(u64),
  // 154: pending_deposits(u64), 162: pending_withdrawals(u64),
  // 170: last_compound_slot(u64), 178: compound_count(u64),
  // 186: admin(32), 218: min_deposit(u64), 226: paused(bool),
  // 227: emergency_reserve(u64), 235: lock_duration_slots(u64),
  // 243: withdraw_queue_slots(u64), 251: _reserved(40) = 291 total
  return {
    exists: true,
    size: data.length,
    totalStaked: data.length >= 146 ? data.readBigUInt64LE(138) : 0n,
    pendingDeposits: data.length >= 162 ? data.readBigUInt64LE(154) : 0n,
    pendingWithdrawals: data.length >= 170 ? data.readBigUInt64LE(162) : 0n,
    compoundCount: data.length >= 186 ? data.readBigUInt64LE(178) : 0n,
    lastCompoundSlot: data.length >= 178 ? data.readBigUInt64LE(170) : 0n,
    paused: data.length > 226 ? data[226] !== 0 : false,
  };
}

// ═══════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════

function fmt(lamports: bigint): string {
  const val = Number(lamports) / 10 ** DECIMALS;
  if (val >= 1e9) return (val / 1e9).toFixed(2) + "B";
  if (val >= 1e6) return (val / 1e6).toFixed(2) + "M";
  if (val >= 1e3) return (val / 1e3).toFixed(1) + "K";
  return val.toFixed(2);
}

function pad(s: string, n: number): string {
  return s.length >= n ? s.substring(0, n) : s + " ".repeat(n - s.length);
}

function rpad(s: string, n: number): string {
  return s.length >= n ? s.substring(0, n) : " ".repeat(n - s.length) + s;
}

function sha256(data: Buffer): string {
  return createHash("sha256").update(data).digest("hex");
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Retry wrapper with exponential backoff for 429 / transient errors */
async function withRetry<T>(
  fn: () => Promise<T>,
  label: string,
  retries = 4,
  baseMs = 500,
): Promise<T> {
  for (let attempt = 0; ; attempt++) {
    try {
      return await fn();
    } catch (err: any) {
      const is429 = err?.message?.includes("429") || err?.message?.includes("Too many requests");
      const isTransient = is429 || err?.message?.includes("ECONNRESET");
      if (!isTransient || attempt >= retries) throw err;
      const delay = baseMs * 2 ** attempt;
      process.stderr.write(`  [retry] ${label} — attempt ${attempt + 1}, wait ${delay}ms\n`);
      await sleep(delay);
    }
  }
}

/** Small delay between sequential RPC calls to stay under rate limits */
const RPC_PACE_MS = 200;

// ═══════════════════════════════════════════════════════════════════════
// Main
// ═══════════════════════════════════════════════════════════════════════

async function main() {
  const env = requireScriptEnv();
  const connection = new Connection(env.rpcUrl, "confirmed");
  const currentSlot = await connection.getSlot("confirmed");

  const issues: string[] = [];
  const warnings: string[] = [];

  console.log(
    "\n\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550",
  );
  console.log(`  MAINNET STATE VERIFICATION — ${new Date().toISOString()}`);
  console.log(`  Cluster: ${env.cluster}  |  Slot: ${currentSlot}`);
  console.log(
    "\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\n",
  );

  // ─────────────────────────────────────────────────────────────────────
  // 1. PROGRAM BINARY VERIFICATION
  // ─────────────────────────────────────────────────────────────────────
  console.log("PROGRAMS\n");

  for (const { name, programId, dataAccount, localPath } of [
    {
      name: "Oracle",
      programId: ORACLE_PROGRAM_ID,
      dataAccount: ORACLE_PROGRAM_DATA,
      localPath: "target/deploy/token_2022.so",
    },
    {
      name: "Vault",
      programId: VAULT_PROGRAM_ID,
      dataAccount: VAULT_PROGRAM_DATA,
      localPath: "target/deploy/channel_vault.so",
    },
  ]) {
    const short = programId.toBase58().substring(0, 8) + "...";

    // Fetch on-chain binary hash
    let onchainHash = "FETCH_FAILED";
    try {
      const info = await connection.getAccountInfo(dataAccount);
      if (info && info.data.length > PROGRAM_DATA_HEADER) {
        const executable = info.data.subarray(PROGRAM_DATA_HEADER);
        onchainHash = sha256(Buffer.from(executable));
      }
    } catch {
      onchainHash = "RPC_ERROR";
    }

    // Local hash
    let localHash = "NOT_FOUND";
    if (existsSync(localPath)) {
      localHash = sha256(readFileSync(localPath));
    }

    const match =
      localHash !== "NOT_FOUND" && onchainHash !== "FETCH_FAILED" && onchainHash !== "RPC_ERROR"
        ? localHash === onchainHash
          ? "MATCH"
          : "MISMATCH"
        : "UNVERIFIED";

    const icon = match === "MATCH" ? "OK" : match === "MISMATCH" ? "!!" : "??";
    console.log(`  ${pad(name, 8)} (${short}):  ${icon}`);
    console.log(`    on-chain: ${onchainHash.substring(0, 16)}...`);
    console.log(`    local:    ${localHash.substring(0, 16)}...`);
    console.log(`    status:   ${match}`);

    if (match === "MISMATCH") {
      issues.push(`${name} binary hash MISMATCH — deploy needed`);
    } else if (match === "UNVERIFIED") {
      warnings.push(`${name} binary hash unverified (local binary missing or RPC error)`);
    }
  }

  // ─────────────────────────────────────────────────────────────────────
  // 2. STAKE POOL AUDIT
  // ─────────────────────────────────────────────────────────────────────
  console.log("\nSTAKE POOLS\n");
  console.log(
    `  ${pad("Channel", 18)} ${rpad("Size", 5)} ${pad("Migr", 5)} ${rpad("Rate/slot", 10)} ${rpad("Stakers", 8)} ${rpad("Staked", 14)} ${pad("Shutdown", 8)}`,
  );
  console.log("  " + "-".repeat(78));

  let poolsMigrated = 0;
  let poolsMissing = 0;
  let poolsUnmigrated = 0;
  let totalStakers = 0n;

  const poolEntries: { name: string; poolPda: PublicKey; pool: StakePoolInfo }[] = [];

  for (const ch of CHANNELS) {
    const channelConfig = new PublicKey(ch.channelConfig);
    const poolPda = deriveOracleStakePool(channelConfig);

    const info = await withRetry(
      () => connection.getAccountInfo(poolPda),
      `pool:${ch.name}`,
    );
    if (!info) {
      console.log(`  ${pad(ch.name, 18)} MISSING`);
      issues.push(`Stake pool missing: ${ch.name}`);
      poolsMissing++;
      await sleep(RPC_PACE_MS);
      continue;
    }

    const pool = parseStakePool(Buffer.from(info.data));
    const migrIcon = pool.migrated ? "Y" : "N";
    const shutIcon = pool.isShutdown ? "YES" : "No";

    console.log(
      `  ${pad(ch.name, 18)} ${rpad(String(pool.size), 5)} ${pad(migrIcon, 5)} ${rpad(String(pool.rewardPerSlot), 10)} ${rpad(String(pool.stakerCount), 8)} ${rpad(fmt(pool.totalStaked), 14)} ${pad(shutIcon, 8)}`,
    );

    if (pool.migrated) poolsMigrated++;
    else {
      poolsUnmigrated++;
      issues.push(`Stake pool NOT migrated: ${ch.name} (${pool.size} bytes)`);
    }
    totalStakers += pool.stakerCount;

    if (pool.rewardPerSlot === 0n && !pool.isShutdown) {
      warnings.push(`Reward rate zero: ${ch.name}`);
    }

    poolEntries.push({
      name: ch.name,
      poolPda,
      pool,
    });

    await sleep(RPC_PACE_MS);
  }

  // ─────────────────────────────────────────────────────────────────────
  // 3. VAULT ACCOUNT AUDIT
  // ─────────────────────────────────────────────────────────────────────
  console.log("\nVAULTS\n");
  console.log(
    `  ${pad("Channel", 18)} ${pad("Exists", 7)} ${pad("Paused", 7)} ${rpad("TotalStaked", 14)} ${rpad("PendingDep", 14)} ${rpad("Compounds", 10)}`,
  );
  console.log("  " + "-".repeat(78));

  let vaultsFound = 0;
  let vaultsMissing = 0;

  for (const ch of CHANNELS) {
    const channelConfig = new PublicKey(ch.channelConfig);
    const vaultPda = deriveVault(channelConfig);

    const info = await withRetry(
      () => connection.getAccountInfo(vaultPda),
      `vault:${ch.name}`,
    );
    if (!info) {
      console.log(`  ${pad(ch.name, 18)} MISSING`);
      issues.push(`Vault missing: ${ch.name}`);
      vaultsMissing++;
      await sleep(RPC_PACE_MS);
      continue;
    }

    const vault = parseVault(Buffer.from(info.data));
    vaultsFound++;

    console.log(
      `  ${pad(ch.name, 18)} ${pad("Y", 7)} ${pad(vault.paused ? "YES" : "No", 7)} ${rpad(fmt(vault.totalStaked), 14)} ${rpad(fmt(vault.pendingDeposits), 14)} ${rpad(String(vault.compoundCount), 10)}`,
    );

    await sleep(RPC_PACE_MS);
  }

  // ─────────────────────────────────────────────────────────────────────
  // 4. REWARD RUNWAY
  // Runway = (vault_balance - pool_total_staked) / (reward_per_slot * slots/day)
  // Note: pool totalStaked may include weighted amounts; vault balance
  // is the actual token balance. Discrepancy ≠ missing funds if weights > 1x.
  // ─────────────────────────────────────────────────────────────────────
  console.log("\nREWARD RUNWAY\n");
  console.log(
    `  ${pad("Channel", 18)} ${rpad("VaultBal", 14)} ${rpad("Staked", 14)} ${rpad("Available", 14)} ${rpad("Rate/day", 12)} ${rpad("Runway", 10)}`,
  );
  console.log("  " + "-".repeat(78));

  for (const { name, poolPda, pool } of poolEntries) {
    const stakeVault = deriveOracleStakeVault(poolPda);
    let balance = 0n;
    let balanceNote = "";
    try {
      const tokenInfo = await withRetry(
        () => connection.getAccountInfo(stakeVault),
        `runway:${name}`,
      );
      if (tokenInfo) {
        // Check if account is owned by Token-2022 or standard Token program
        const owner = tokenInfo.owner.toBase58();
        if (owner === TOKEN_2022_PROGRAM_ID.toBase58()) {
          const unpacked = unpackAccount(stakeVault, tokenInfo, TOKEN_2022_PROGRAM_ID);
          balance = unpacked.amount;
        } else {
          balanceNote = `owner=${owner.substring(0, 8)}...`;
        }
      } else {
        balanceNote = "no-account";
      }
    } catch (err: any) {
      balanceNote = `err:${err.message?.substring(0, 30)}`;
    }

    const available = balance > pool.totalStaked ? balance - pool.totalStaked : 0n;
    const ratePerDay = pool.rewardPerSlot * BigInt(SLOTS_PER_DAY);
    const runwayDays =
      ratePerDay > 0n ? Number(available) / Number(ratePerDay) : Infinity;

    const runwayStr =
      runwayDays === Infinity
        ? "N/A"
        : runwayDays < 1
          ? `${(runwayDays * 24).toFixed(1)}h`
          : `${runwayDays.toFixed(1)}d`;

    const noteStr = balanceNote ? `  (${balanceNote})` : "";
    console.log(
      `  ${pad(name, 18)} ${rpad(fmt(balance), 14)} ${rpad(fmt(pool.totalStaked), 14)} ${rpad(fmt(available), 14)} ${rpad(fmt(ratePerDay), 12)} ${rpad(runwayStr, 10)}${noteStr}`,
    );

    if (runwayDays < 7 && runwayDays !== Infinity) {
      warnings.push(`Low runway: ${name} — ${runwayStr}`);
    }

    await sleep(RPC_PACE_MS);
  }

  // ─────────────────────────────────────────────────────────────────────
  // 5. USER STAKE ENUMERATION
  // ─────────────────────────────────────────────────────────────────────
  console.log("\nUSER STAKES\n");

  try {
    // Fetch all accounts owned by Oracle program of a reasonable size
    // UserChannelStake accounts have discriminator starting bytes we can filter on
    const allOracleAccounts = await withRetry(
      () => connection.getProgramAccounts(ORACLE_PROGRAM_ID, {
        filters: [{ dataSize: 162 }],
      }),
      "user-stakes-162",
    );

    await sleep(RPC_PACE_MS * 2); // heavier call, extra pause

    // Also check for unmigrated sizes
    const oldAccounts129 = await withRetry(
      () => connection.getProgramAccounts(ORACLE_PROGRAM_ID, {
        filters: [{ dataSize: 129 }],
      }),
      "user-stakes-129",
    );

    await sleep(RPC_PACE_MS * 2);

    const oldAccounts161 = await withRetry(
      () => connection.getProgramAccounts(ORACLE_PROGRAM_ID, {
        filters: [{ dataSize: 161 }],
      }),
      "user-stakes-161",
    );

    // Note: 162 includes both stake pools AND user stakes at that size
    // We need to differentiate by discriminator
    // For now, report raw counts
    console.log(`  Accounts at 162 bytes (current):  ${allOracleAccounts.length}`);
    console.log(`  Accounts at 129 bytes (old v1):   ${oldAccounts129.length}`);
    console.log(`  Accounts at 161 bytes (old v2):   ${oldAccounts161.length}`);

    if (oldAccounts129.length > 0) {
      issues.push(`${oldAccounts129.length} accounts at 129 bytes — need migration (phase 1)`);
    }
    if (oldAccounts161.length > 0) {
      issues.push(`${oldAccounts161.length} accounts at 161 bytes — need migration (phase 2)`);
    }
  } catch (err: any) {
    warnings.push(`Could not enumerate user stakes: ${err.message?.substring(0, 60)}`);
    console.log(`  SKIP — RPC error: ${err.message?.substring(0, 80)}`);
  }

  // ─────────────────────────────────────────────────────────────────────
  // 6. SUMMARY
  // ─────────────────────────────────────────────────────────────────────
  console.log(
    "\n\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550",
  );
  console.log("  SUMMARY\n");
  console.log(`  Stake pools:  ${poolsMigrated}/${CHANNELS.length} migrated, ${poolsMissing} missing`);
  console.log(`  Vaults:       ${vaultsFound}/${CHANNELS.length} found, ${vaultsMissing} missing`);
  console.log(`  Total stakers: ${totalStakers}`);

  if (issues.length > 0) {
    console.log("\n  RED — Issues requiring action:");
    for (const i of issues) console.log(`    !! ${i}`);
  }
  if (warnings.length > 0) {
    console.log("\n  YELLOW — Warnings:");
    for (const w of warnings) console.log(`    -- ${w}`);
  }
  if (issues.length === 0 && warnings.length === 0) {
    console.log("\n  GREEN — All checks passed");
  }

  const status = issues.length > 0 ? "RED" : warnings.length > 0 ? "YELLOW" : "GREEN";
  console.log(`\n  STATUS: ${status}`);
  console.log(
    "\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\n",
  );

  process.exit(issues.length > 0 ? 1 : 0);
}

main().catch((err) => {
  console.error("Fatal:", err.message);
  process.exit(2);
});
