import SwitchboardProgram from '@switchboard-xyz/sbv2-lite';
import { Connection, clusterApiUrl, PublicKey } from '@solana/web3.js';

export type SwitchboardPrice = {
  ok: boolean;
  price?: number; // numeric value from aggregator
  feed: string;
  cluster: 'devnet' | 'mainnet-beta';
  updatedRecently?: boolean;
  error?: string;
};

/**
 * Fetch the latest price from a Switchboard aggregator.
 * Defaults: devnet cluster and SOL/USD public aggregator from docs.
 */
export async function fetchSwitchboardPrice(params?: {
  feedPubkey?: string;
  cluster?: 'devnet' | 'mainnet-beta';
  maxStalenessSec?: number;
}): Promise<SwitchboardPrice> {
  const feed = params?.feedPubkey || process.env.SB_FEED || 'GvDMxPzN1sCj7L26YDK2HnMRXEQmQ2aemov8YBtPS7vR';
  const cluster = (params?.cluster || (process.env.SB_CLUSTER as 'devnet' | 'mainnet-beta') || 'devnet');
  const maxStalenessSec = params?.maxStalenessSec ?? Number(process.env.SB_MAX_STALENESS_SEC || 300);

  try {
    const connection = new Connection(
      process.env.SOLANA_RPC_URL || clusterApiUrl(cluster),
      'confirmed'
    );

    // Load lightweight Switchboard decoder
    const sbv2 =
      cluster === 'mainnet-beta'
        ? await SwitchboardProgram.loadMainnet(connection)
        : await SwitchboardProgram.load(connection);

    const pubkey = new PublicKey(feed);
    const accountInfo = await connection.getAccountInfo(pubkey);
    if (!accountInfo) {
      return { ok: false, feed, cluster, error: 'Aggregator account not found' };
    }

    const latest = sbv2.decodeLatestAggregatorValue(accountInfo, maxStalenessSec);
    if (latest === null) {
      return { ok: false, feed, cluster, error: 'Stale or unavailable price', updatedRecently: false };
    }

    const price = Number(latest.toString());
    return { ok: true, price, feed, cluster, updatedRecently: true };
  } catch (e: any) {
    return { ok: false, feed, cluster, error: e?.message || String(e) };
  }
}

