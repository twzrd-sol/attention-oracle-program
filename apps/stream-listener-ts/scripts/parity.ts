import fs from 'node:fs';
import path from 'node:path';

function readLines(p: string): string[] {
  if (!fs.existsSync(p)) throw new Error(`missing file: ${p}`);
  const raw = fs.readFileSync(p, 'utf8').trim();
  return raw ? raw.split(/\r?\n/) : [];
}

const legacy = process.argv[2] || path.resolve('..', 'stream-listener', 'logs', 'stream-events.ndjson');
const next = process.argv[3] || path.resolve('..', 'logs', 'events.ndjson');

const a = readLines(legacy);
const b = readLines(next);

if (a.length !== b.length) throw new Error(`length mismatch: legacy=${a.length} v2=${b.length}`);

for (let i = Math.max(0, a.length - 1000); i < a.length; i++) {
  const A = JSON.parse(a[i]);
  const B = JSON.parse(b[i]);
  if (A.signature !== B.signature || A.name !== B.name) {
    throw new Error(`mismatch at index ${i}: ${A.signature}/${A.name} vs ${B.signature}/${B.name}`);
  }
}

console.log('PARITY OK (last 1000 events)');

