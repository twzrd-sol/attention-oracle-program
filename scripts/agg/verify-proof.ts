/**
 * Verify a single proof from a compute/publish JSON file.
 *
 * Usage:
 *   pnpm agg:verify -- --file out/test.json --index 0
 */

import { readFileSync } from 'node:fs';
import { PublicKey } from '@solana/web3.js';
import { computeLeafByVersion, hexToBytes, verifyProof } from './helpers.ts';

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
  const file = req('file');
  const index = Number(req('index'));
  const data = JSON.parse(readFileSync(file, 'utf8'));

  const leafVersion: number = Number(data.leafVersion ?? 0);
  const claim = data.claims[index];
  if (!claim) throw new Error(`No claim at index ${index}`);

  const extras = data.subject && data.epoch ? { subject: new PublicKey(data.subject), epoch: BigInt(data.epoch) } : undefined;

  const leaf = computeLeafByVersion(
    leafVersion,
    { claimer: new PublicKey(claim.claimer), index: claim.index, amount: BigInt(claim.amount), id: claim.id },
    extras,
  );

  const proof = (data.nodes?.[index] ?? claim.proof?.map(String))?.map((h: string) => hexToBytes(h));
  if (!proof) throw new Error('No proof in file');
  const root = hexToBytes(data.root);

  const ok = verifyProof(leaf, proof, root);
  console.log(JSON.stringify({ index, ok }));
}

main().catch((e) => { console.error(e); process.exit(1); });

