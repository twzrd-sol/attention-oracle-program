/**
 * Helius Enhanced WebSocket ingest for pump.fun token trades.
 *
 * Subscribes to transaction stream filtered by `accountInclude: [<MINT>]` and aggregates
 * per-owner presence/volume over rolling epochs. Emits claims JSON files per epoch.
 *
 * Requires: HELIUS_API_KEY (Business/Pro for Enhanced WS). For dev/test, Free should work
 * on standard streams, but this script uses the Enhanced WS endpoint.
 *
 * Usage:
 *   pnpm agg:ingest:helius -- \
 *     --cluster devnet \
 *     --mint <TOKEN_MINT> \
 *     --epochSec 60 \
 *     --mode presence|volume \
 *     --outDir out \
 *     [--apiKey <KEY>]
 */

import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import WebSocket from 'ws';
import { PublicKey } from '@solana/web3.js';

type Mode = 'presence' | 'volume';

const argv = Object.fromEntries(process.argv.slice(2).map((a, i, arr) => {
  if (!a.startsWith('--')) return [] as any;
  const k = a.slice(2);
  const v = arr[i + 1] && !arr[i + 1].startsWith('--') ? arr[i + 1] : 'true';
  return [k, v];
}).filter(Boolean));

function req(name: string): string {
  const v = argv[name];
  if (!v) throw new Error(`Missing --${name}`);
  return v;
}

function epochBounds(nowMs: number, durSec: number) {
  const durMs = durSec * 1000;
  const start = nowMs - (nowMs % durMs);
  return { start, end: start + durMs - 1 };
}

async function main() {
  const cluster = (argv['cluster'] ?? process.env.HELIUS_CLUSTER ?? 'devnet') as 'devnet' | 'mainnet';
  const apiKey = (argv['apiKey'] ?? process.env.HELIUS_API_KEY) as string;
  if (!apiKey) throw new Error('Provide --apiKey or HELIUS_API_KEY');
  const mint = new PublicKey(req('mint'));
  const epochSec = Number(argv['epochSec'] ?? '60');
  const outDir = argv['outDir'] ?? 'out';
  const mode: Mode = (argv['mode'] ?? 'presence') as Mode;

  mkdirSync(outDir, { recursive: true });

  const host = cluster === 'mainnet' ? 'atlas-mainnet.helius-rpc.com' : 'atlas-devnet.helius-rpc.com';
  const url = `wss://${host}/?api-key=${apiKey}`;
  const ws = new WebSocket(url);

  let totals = new Map<string, bigint>();
  let cur = epochBounds(Date.now(), epochSec);

  ws.on('open', () => {
    const sub = {
      jsonrpc: '2.0',
      id: 1,
      method: 'transactionSubscribe',
      params: [
        {
          accountInclude: [mint.toBase58()],
          commitment: 'confirmed',
          encoding: 'jsonParsed',
          transactionDetails: 'full',
          maxSupportedTransactionVersion: 0,
        },
      ],
    };
    ws.send(JSON.stringify(sub));
    // Keepalive
    setInterval(() => ws.ping(), 30000);
    console.log('Helius WS connected:', host);
  });

  ws.on('message', (raw) => {
    try {
      const msg = JSON.parse(raw.toString());
      if (!msg?.params?.result?.transaction) return;
      const tx = msg.params.result.transaction;
      const meta = tx.meta;
      if (!meta) return;

      // epoch rollover
      const now = Date.now();
      if (now > cur.end) {
        flush(cur.start, totals, outDir);
        totals = new Map();
        cur = epochBounds(now, epochSec);
      }

      const pre = meta.preTokenBalances ?? [];
      const post = meta.postTokenBalances ?? [];
      const byIndex = new Map<number, { owner: string; amount: bigint }>();
      for (const b of pre) if (b.mint === mint.toBase58() && b.owner) byIndex.set(b.accountIndex, { owner: b.owner, amount: BigInt(b.uiTokenAmount.amount) });
      for (const b of post) if (b.mint === mint.toBase58() && b.owner) {
        const prev = byIndex.get(b.accountIndex) ?? { owner: b.owner!, amount: 0n };
        const delta = BigInt(b.uiTokenAmount.amount) - prev.amount;
        const key = b.owner!;
        const curAmt = totals.get(key) ?? 0n;
        if (mode === 'presence') {
          totals.set(key, curAmt + 1n);
        } else {
          const vol = delta < 0n ? -delta : delta;
          totals.set(key, curAmt + vol);
        }
      }
    } catch (e) {
      console.error('parse error:', e);
    }
  });

  ws.on('error', (e) => console.error('ws error', e));
  ws.on('close', () => console.log('ws closed'));
}

function flush(epochStartMs: number, totals: Map<string, bigint>, outDir: string) {
  const ranked = [...totals.entries()].sort((a, b) => (b[1] > a[1] ? 1 : b[1] < a[1] ? -1 : 0));
  const claims = ranked.map(([owner, amt]) => ({ claimer: owner, amount: amt.toString(), id: owner }));
  const file = join(outDir, `claims-${epochStartMs}.json`);
  writeFileSync(file, JSON.stringify(claims, null, 2));
  console.log(`Flushed ${claims.length} claims -> ${file}`);
}

main().catch((e) => { console.error(e); process.exit(1); });

