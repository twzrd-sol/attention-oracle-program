/**
 * Pure off-chain: compute root + proofs from a claims JSON file.
 * Useful for testing end-to-end without publishing.
 *
 * Usage:
 *   ts-node --esm scripts/agg/compute-merkle.ts \
 *     --channel <pump_token_mint> \
 *     --epoch 123 \
 *     --leaf-version 1 \
 *     --namespace pump: \
 *     --claims ./claims.json \
 *     --out ./out/epoch-123.json
 */

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { PublicKey } from '@solana/web3.js';
import { computeLeafByVersion, deriveSubjectId, merkleProof, merkleRoot, toHex } from './helpers.ts';

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

async function main() {
  const channel = req('channel');
  const epoch = BigInt(req('epoch'));
  const leafVersion = Number(argv['leaf-version'] ?? '0');
  const namespace = argv['namespace'] as string | undefined;
  const claimsPath = req('claims');
  const outPath = req('out');

  type ClaimRow = { claimer: string; amount: string | number; id: string };
  const claims: ClaimRow[] = JSON.parse(readFileSync(claimsPath, 'utf8'));
  const subject = deriveSubjectId(channel, namespace);

  const leaves = claims.map((row, i) =>
    computeLeafByVersion(
      leafVersion,
      { claimer: new PublicKey(row.claimer), index: i, amount: BigInt(row.amount), id: row.id },
      { subject: new PublicKey(subject), epoch }
    )
  );
  const root = merkleRoot(leaves);
  const proofs = leaves.map((_, i) => merkleProof(leaves, i));

  const json = {
    channel,
    epoch: epoch.toString(),
    subject: new PublicKey(subject).toBase58(),
    leafVersion,
    root: `0x${toHex(root)}`,
    claims: claims.map((c, i) => ({ ...c, index: i })),
    nodes: proofs.map((p) => p.map((n) => `0x${toHex(n)}`)),
  };
  mkdirSync(dirname(resolve(outPath)), { recursive: true });
  writeFileSync(outPath, JSON.stringify(json, null, 2));
  console.log(`Wrote ${outPath}`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
