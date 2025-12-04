/**
 * Ingest traders for a token mint within an epoch window and emit claims.json.
 *
 * Strategy: fetch signatures referencing the mint account, filter by blockTime,
 * then fetch parsed transactions and aggregate owner balance deltas for that mint.
 *
 * Usage:
 *   pnpm agg:ingest -- \
 *     --rpc https://api.devnet.solana.com \
 *     --mint <TOKEN_MINT> \
 *     --start <unix_secs> --end <unix_secs> \
 *     --mode presence|volume \
 *     --limit 4096 \
 *     --out ./claims.json
 */

import { writeFileSync } from 'node:fs';
import { Connection, PublicKey } from '@solana/web3.js';

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

type ClaimRow = { claimer: string; amount: string; id: string };

async function main() {
  const rpc = req('rpc');
  const mint = new PublicKey(req('mint'));
  const start = Number(req('start'));
  const end = Number(req('end'));
  const out = req('out');
  const mode = (argv['mode'] ?? 'presence') as 'presence' | 'volume';
  const limit = Number(argv['limit'] ?? '4096');

  const connection = new Connection(rpc, 'confirmed');

  // Page signatures referencing this mint account
  const users = new Map<string, bigint>();

  let before: string | undefined = undefined;
  while (true) {
    const sigs = await connection.getSignaturesForAddress(mint, { before, limit: 1000 }, 'confirmed');
    if (sigs.length === 0) break;

    for (const s of sigs) {
      if (!s.blockTime) continue;
      if (s.blockTime < start) { before = s.signature; continue; }
      if (s.blockTime > end) continue; // signatures are newest-first

      const tx = await connection.getParsedTransaction(s.signature, { maxSupportedTransactionVersion: 0, commitment: 'confirmed' });
      if (!tx?.meta) continue;

      const pre = tx.meta.preTokenBalances ?? [];
      const post = tx.meta.postTokenBalances ?? [];
      // Filter balances for this mint
      type Bal = { owner: string; amount: bigint };
      const byIndex = new Map<number, Bal>();
      for (const b of pre) if (b.mint === mint.toBase58() && b.owner) byIndex.set(b.accountIndex, { owner: b.owner, amount: BigInt(b.uiTokenAmount.amount) });
      for (const b of post) if (b.mint === mint.toBase58() && b.owner) {
        const prev = byIndex.get(b.accountIndex) ?? { owner: b.owner!, amount: 0n };
        const delta = BigInt(b.uiTokenAmount.amount) - prev.amount;
        if (delta !== 0n) {
          // Attribute volume to the owner of this token account
          const cur = users.get(b.owner!) ?? 0n;
          users.set(b.owner!, cur + (mode === 'volume' ? (delta < 0n ? -delta : delta) : 1n));
        }
      }
    }

    before = sigs[sigs.length - 1].signature;
    if (sigs[sigs.length - 1].blockTime && sigs[sigs.length - 1].blockTime < start) break;
  }

  // Rank and cap
  const ranked = [...users.entries()].sort((a, b) => (b[1] > a[1] ? 1 : b[1] < a[1] ? -1 : 0)).slice(0, limit);
  const claims: ClaimRow[] = ranked.map(([owner, amt]) => ({ claimer: owner, amount: amt.toString(), id: owner }));

  writeFileSync(out, JSON.stringify(claims, null, 2));
  console.log(`Wrote ${claims.length} claims to ${out}`);
}

main().catch((e) => { console.error(e); process.exit(1); });

