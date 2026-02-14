/**
 * Discover live vault pools from on-chain AO stake-pool accounts.
 *
 * Use this when the canonical channels list is stale and you need an accurate
 * set of active channel configs before regenerating scripts/keepers input.
 *
 * Usage:
 *   RPC_URL=https://api.mainnet-beta.solana.com npx tsx scripts/admin/discover-live-vault-pools.ts
 *   RPC_URL=... npx tsx scripts/admin/discover-live-vault-pools.ts --top 2 --json
 *
 * Flags:
 *   --top N           Print only the top-N pools by totalWeighted.
 *   --min-weighted N   Minimum totalWeighted cutoff (integer). default 0
 *   --include-closed   Keep shutdown pools (default: false)
 *   --json             Print machine-readable JSON payload.
 */
import { Connection, PublicKey } from "@solana/web3.js";
import {
  deriveOracleStakePool,
  deriveOracleStakeVault,
  deriveVault,
} from "../keepers/lib/vault-pda.js";

interface PoolSummary {
  channelConfig: string;
  channelStakePool: string;
  oracleStakeVault: string;
  channelVault: string;
  totalWeighted: bigint;
  totalStaked: bigint;
  stakerCount: bigint;
  isShutdown: boolean;
  stakePoolPdaMatches: boolean;
  stakeVaultPdaMatches: boolean;
}

interface ScriptArgs {
  top?: number;
  minWeighted: bigint;
  includeClosed: boolean;
  json: boolean;
  dataSize: number;
  noSizeFilter: boolean;
}

function parseArgs(argv: string[]): ScriptArgs {
  const args: ScriptArgs = {
    minWeighted: 0n,
    includeClosed: false,
    json: false,
    dataSize: 162,
    noSizeFilter: false,
  };

  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--top" && argv[i + 1]) {
      args.top = Number.parseInt(argv[++i], 10);
      continue;
    }

    if (a === "--min-weighted" && argv[i + 1]) {
      args.minWeighted = BigInt(argv[++i]);
      continue;
    }

    if (a === "--data-size" && argv[i + 1]) {
      const v = Number.parseInt(argv[++i], 10);
      if (!Number.isSafeInteger(v) || v <= 0) {
        throw new Error(`--data-size must be a positive integer (got ${JSON.stringify(argv[i])})`);
      }
      args.dataSize = v;
      continue;
    }

    if (a === "--no-size-filter") {
      args.noSizeFilter = true;
      continue;
    }

    if (a === "--include-closed") {
      args.includeClosed = true;
      continue;
    }

    if (a === "--json") {
      args.json = true;
      continue;
    }

    if (a === "--help" || a === "-h") {
      throw new Error(
        "Usage: RPC_URL=... npx tsx scripts/admin/discover-live-vault-pools.ts " +
        "[--top N] [--min-weighted N] [--include-closed] [--json] " +
        "[--data-size N] [--no-size-filter]"
      );
    }

    throw new Error(`Unknown arg: ${a}`);
  }

  return args;
}

function formatNumber(value: bigint): string {
  return value.toLocaleString("en-US");
}

async function main() {
  const args = parseArgs(process.argv);
  const rpcUrl = process.env.RPC_URL;
  if (!rpcUrl) {
    throw new Error("Set RPC_URL");
  }

  const connection = new Connection(rpcUrl, "confirmed");

  const oracleProgramId = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
  const MIN_CHANNEL_STAKE_POOL_LEN = 162;

  const filters = args.noSizeFilter ? [] : [{ dataSize: args.dataSize }];
  if (args.noSizeFilter) {
    console.warn("WARN: --no-size-filter will scan all program accounts (may be slow / rate-limited).");
  }

  const rawPools = await connection.getProgramAccounts(oracleProgramId, {
    commitment: "confirmed",
    ...(filters.length ? { filters } : {}),
  });
  if (rawPools.length === 0) {
    throw new Error(
      "No accounts returned from getProgramAccounts().\n" +
      "If the ChannelStakePool layout changed, rerun with:\n" +
      "  --no-size-filter   (slower)\n" +
      "  --data-size <N>    (if you know the new size)\n"
    );
  }

  const byChannel = new Map<string, PoolSummary>();

  for (const pool of rawPools) {
    const data = pool.account.data;
    if (data.length < MIN_CHANNEL_STAKE_POOL_LEN) continue;

    // ChannelStakePool layout offsets (from programs/token_2022/src/state.rs):
    // [0..8] discriminator
    // [40..72] channel_config
    // [72..104] vault
    // [104..112] total_staked
    // [112..120] total_weighted
    // [120..128] staker_count
    // [161] is_shutdown
    const channelConfig = new PublicKey(data.slice(40, 72)).toBase58();
    const oracleStakeVault = new PublicKey(data.slice(72, 104)).toBase58();
    const isShutdown = data[161] !== 0;
    const totalStaked = data.readBigUInt64LE(104);
    const totalWeighted = data.readBigUInt64LE(112);
    const stakerCount = data.readBigUInt64LE(120);

    const channelConfigPk = new PublicKey(channelConfig);
    const expectedStakePool = deriveOracleStakePool(channelConfigPk);
    const expectedStakeVault = deriveOracleStakeVault(pool.pubkey);
    const channelVault = deriveVault(channelConfigPk).toBase58();

    const stakePoolPdaMatches = expectedStakePool.equals(pool.pubkey);
    const stakeVaultPdaMatches = expectedStakeVault.toBase58() === oracleStakeVault;

    const existing = byChannel.get(channelConfig);
    if (
      !existing ||
      totalWeighted > existing.totalWeighted ||
      (totalWeighted === existing.totalWeighted && totalStaked > existing.totalStaked)
    ) {
      byChannel.set(channelConfig, {
        channelConfig,
        channelStakePool: pool.pubkey.toBase58(),
        totalWeighted,
        totalStaked,
        stakerCount,
        isShutdown,
        oracleStakeVault,
        channelVault,
        stakePoolPdaMatches,
        stakeVaultPdaMatches,
      });
    }
  }

  const all = [...byChannel.values()]
    .filter((entry) => entry.totalWeighted >= args.minWeighted)
    .filter((entry) => args.includeClosed || !entry.isShutdown)
    .sort((a, b) => (a.totalWeighted > b.totalWeighted ? -1 : a.totalWeighted < b.totalWeighted ? 1 : 0));

  if (all.length === 0) {
    throw new Error(
      "No pools found after parsing/filtering.\n" +
      "Try lowering --min-weighted, adding --include-closed, or rerun with --no-size-filter."
    );
  }

  if (args.top && args.top > 0) {
    all.length = Math.min(all.length, args.top);
  }

  if (args.json) {
    console.log(JSON.stringify(all, (_key, value) =>
      typeof value === "bigint" ? value.toString() : value,
    2));
    return;
  }

  const verified = all.filter((p) => p.stakePoolPdaMatches && p.stakeVaultPdaMatches);

  console.log("onchain discover: channel_stake_pool candidates", all.length);
  console.log(
    `stake-pool PDA match: ${all.filter((p) => p.stakePoolPdaMatches).length}/${all.length}`,
  );
  console.log(
    `stake-vault PDA match: ${all.filter((p) => p.stakeVaultPdaMatches).length}/${all.length}`,
  );
  console.log(
    `verified (both PDAs): ${verified.length}/${all.length}`,
  );
  for (const pool of all) {
    const status = pool.isShutdown ? "shutdown" : "open";
    const stakePoolStatus = pool.stakePoolPdaMatches ? "ok" : "mismatch";
    const stakeVaultStatus = pool.stakeVaultPdaMatches ? "ok" : "mismatch";
    console.log(
      `${pool.channelConfig} [${status}]` +
        ` | weight=${formatNumber(pool.totalWeighted)} staked=${formatNumber(pool.totalStaked)}` +
        ` | stakers=${formatNumber(pool.stakerCount)}` +
        ` | stakePool=${stakePoolStatus} stakeVault=${stakeVaultStatus}` +
        ` | channelVault=${pool.channelVault}`,
    );
  }

  if (verified.length === 0) {
    throw new Error(
      "No pools passed PDA sanity checks.\n" +
      "This usually means the account layout offsets are wrong or you scanned unrelated accounts.\n" +
      "Try running with --data-size <N> (if layout changed) and verify offsets against programs/token_2022/src/state.rs."
    );
  }

  console.log("\nSuggested TWZRD_CHANNELS_JSON skeleton (fill oracleChannel + slot params):");
  for (const pool of verified) {
    console.log(`
  {
    "name": "${pool.channelConfig.slice(0, 8)}",
    "label": "vLOFI ${pool.channelConfig.slice(0, 8)}",
    "channelConfig": "${pool.channelConfig}",
    "lockDurationSlots": 54000,
    "withdrawQueueSlots": 9000,
    "oracleChannel": "<set to on-chain channel seed string, e.g. stream:tv>"
  },`);
  }
}

main().catch((error) => {
  console.error("Discovery failed:", (error as Error).message);
  process.exit(1);
});
