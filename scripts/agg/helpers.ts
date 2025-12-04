/*
 Minimal helpers for building cross-context attention proofs off-chain.
 - Subject derivation (namespaced)
 - Versioned leaf hashing (v0/v1)
 - Merkle tree (sorted-pair keccak)
 - Anchor-style instruction data builders (discriminators + Borsh encoders)
*/

import { PublicKey, TransactionInstruction } from '@solana/web3.js';
import { keccak_256 } from '@noble/hashes/sha3';
import { sha256 } from '@noble/hashes/sha256';

export const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');

// ---------- Byte utilities ----------

export function u32le(n: number): Uint8Array {
  const b = new Uint8Array(4);
  new DataView(b.buffer).setUint32(0, n >>> 0, true);
  return b;
}

export function u64le(n: bigint | number): Uint8Array {
  const x = typeof n === 'bigint' ? n : BigInt(n);
  const b = new Uint8Array(8);
  const view = new DataView(b.buffer);
  view.setUint32(0, Number(x & 0xffffffffn), true);
  view.setUint32(4, Number((x >> 32n) & 0xffffffffn), true);
  return b;
}

export function concat(parts: Uint8Array[]): Uint8Array {
  const len = parts.reduce((a, p) => a + p.length, 0);
  const out = new Uint8Array(len);
  let off = 0;
  for (const p of parts) {
    out.set(p, off);
    off += p.length;
  }
  return out;
}

export function cmpBytes(a: Uint8Array, b: Uint8Array): number {
  const len = Math.min(a.length, b.length);
  for (let i = 0; i < len; i++) {
    const d = a[i] - b[i];
    if (d !== 0) return d;
  }
  return a.length - b.length;
}

export function hashv(parts: Uint8Array[]): Uint8Array {
  const h = keccak_256.create();
  for (const p of parts) h.update(p);
  return h.digest();
}

export function toUtf8(s: string): Uint8Array {
  return new TextEncoder().encode(s);
}

export function toHex(a: Uint8Array): string {
  return [...a].map((x) => x.toString(16).padStart(2, '0')).join('');
}

export function bytesToHex(a: Uint8Array): string { return toHex(a); }
export function hexToBytes(hex: string): Uint8Array {
  const clean = hex.startsWith('0x') ? hex.slice(2) : hex;
  if (clean.length % 2 !== 0) throw new Error('hex length must be even');
  const out = new Uint8Array(clean.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  return out;
}

// ---------- Subject Derivation ----------

export type NamespaceConfig = { namespace: string; enabled: boolean };

export function deriveSubjectId(channel: string, namespace?: string | NamespaceConfig): Uint8Array {
  const lower = toUtf8(channel.toLowerCase());
  const nsStr = typeof namespace === 'object' ? (namespace.enabled ? namespace.namespace : undefined) : namespace;
  const ns = nsStr ? toUtf8(nsStr) : undefined;
  const bytes = ns && ns.length > 0
    ? hashv([toUtf8('channel:'), ns, toUtf8(':'), lower])
    : hashv([toUtf8('channel:'), lower]);
  return bytes; // 32 bytes
}

export function subjectIdToPubkey(subjectId: Uint8Array): PublicKey { return new PublicKey(subjectId); }

// ---------- Versioned Leaf Hashing ----------

export type LeafInputV0 = {
  claimer: PublicKey;
  index: number;
  amount: bigint | number;
  id: string;
};

export type LeafInputV1 = LeafInputV0 & {
  subject: PublicKey | Uint8Array;
  epoch: bigint | number;
};

export function computeLeafV0(inp: LeafInputV0): Uint8Array {
  return hashv([
    inp.claimer.toBytes(),
    u32le(inp.index),
    u64le(inp.amount),
    toUtf8(inp.id),
  ]);
}

export function computeLeafV1(inp: LeafInputV1): Uint8Array {
  const subject = inp.subject instanceof PublicKey ? inp.subject.toBytes() : inp.subject;
  return hashv([
    toUtf8('leaf:'),
    subject,
    u64le(inp.epoch),
    inp.claimer.toBytes(),
    u32le(inp.index),
    u64le(inp.amount),
    toUtf8(inp.id),
  ]);
}

export function computeLeafByVersion(
  version: number,
  base: LeafInputV0,
  extras?: { subject: PublicKey | Uint8Array; epoch: bigint | number }
): Uint8Array {
  if (version === 1) {
    if (!extras) throw new Error('v1 leaf requires subject and epoch');
    return computeLeafV1({ ...base, subject: extras.subject, epoch: extras.epoch });
  }
  return computeLeafV0(base);
}

// ---------- Merkle Tree (sorted-pair keccak) ----------

export function buildLevels(leaves: Uint8Array[]): Uint8Array[][] {
  if (leaves.length === 0) throw new Error('No leaves');
  const L0 = leaves.map((l) => new Uint8Array(l));
  const levels: Uint8Array[][] = [L0];
  while (levels[levels.length - 1].length > 1) {
    const prev = levels[levels.length - 1];
    const next: Uint8Array[] = [];
    for (let i = 0; i < prev.length; i += 2) {
      const a = prev[i];
      const b = i + 1 < prev.length ? prev[i + 1] : prev[i];
      const [x, y] = cmpBytes(a, b) <= 0 ? [a, b] : [b, a];
      next.push(hashv([x, y]));
    }
    levels.push(next);
  }
  return levels;
}

export function merkleRoot(leaves: Uint8Array[]): Uint8Array {
  return buildLevels(leaves).at(-1)![0];
}

export function merkleProof(leaves: Uint8Array[], index: number): Uint8Array[] {
  const levels = buildLevels(leaves);
  const proof: Uint8Array[] = [];
  let idx = index;
  for (let d = 0; d < levels.length - 1; d++) {
    const level = levels[d];
    const pair = idx ^ 1; // sibling index
    const sib = level[pair] ?? level[idx];
    proof.push(sib);
    idx = Math.floor(idx / 2);
  }
  return proof;
}

export function verifyProof(leaf: Uint8Array, proof: Uint8Array[], root: Uint8Array): boolean {
  let hash = new Uint8Array(leaf);
  for (const sib of proof) {
    const [x, y] = cmpBytes(hash, sib) <= 0 ? [hash, sib] : [sib, hash];
    hash = hashv([x, y]);
  }
  return toHex(hash) === toHex(root);
}

// Build a full tree (root + proofs) from ClaimEntry[] with optional mode.
export type ClaimEntry = { claimer: PublicKey; index: number; amount: bigint | number; id: string };
export type MerkleTree = { root: Uint8Array; leaves: Uint8Array[]; proofs: Map<number, Uint8Array[]> };

export type MerkleMode = 'duplicate' | 'promote';

export function buildMerkleTree(
  entries: ClaimEntry[],
  version: 0 | 1 = 0,
  subject?: PublicKey | Uint8Array,
  epoch?: bigint | number,
  mode: MerkleMode = 'duplicate',
): MerkleTree {
  if (!entries.length) throw new Error('Cannot build tree from empty entries');
  const leaves = entries.map((e, i) => computeLeafByVersion(version, { claimer: e.claimer, index: i, amount: e.amount, id: e.id }, subject && epoch !== undefined ? { subject: subject instanceof PublicKey ? subject.toBytes() : subject, epoch } : undefined));

  // Construct levels according to mode
  const levels: Uint8Array[][] = [leaves.map((l) => new Uint8Array(l))];
  while (levels[levels.length - 1].length > 1) {
    const prev = levels[levels.length - 1];
    const next: Uint8Array[] = [];
    for (let i = 0; i < prev.length; i += 2) {
      if (i + 1 >= prev.length) {
        if (mode === 'promote') {
          next.push(prev[i]);
          continue;
        } else {
          const [x, y] = cmpBytes(prev[i], prev[i]) <= 0 ? [prev[i], prev[i]] : [prev[i], prev[i]];
          next.push(hashv([x, y]));
          continue;
        }
      }
      const a = prev[i];
      const b = prev[i + 1];
      const [x, y] = cmpBytes(a, b) <= 0 ? [a, b] : [b, a];
      next.push(hashv([x, y]));
    }
    levels.push(next);
  }

  const root = levels[levels.length - 1][0];
  const proofs = new Map<number, Uint8Array[]>();
  for (let leafIdx = 0; leafIdx < leaves.length; leafIdx++) {
    const proof: Uint8Array[] = [];
    let idx = leafIdx;
    for (let d = 0; d < levels.length - 1; d++) {
      const level = levels[d];
      const isLastOdd = level.length % 2 === 1 && idx === level.length - 1;
      if (isLastOdd && mode === 'promote') {
        // no sibling pushed
      } else {
        const pair = idx ^ 1;
        const sib = level[pair] ?? level[idx];
        proof.push(sib);
      }
      idx = Math.floor(idx / 2);
    }
    proofs.set(leafIdx, proof);
  }
  return { root, leaves, proofs };
}

// ---------- Anchor instruction builders ----------

export function anchorDiscriminator(name: string): Uint8Array {
  // first 8 bytes of sha256("global:<name>")
  const preimage = toUtf8(`global:${name}`);
  const h = sha256(preimage);
  return h.slice(0, 8);
}

export function borshString(s: string): Uint8Array {
  const bytes = toUtf8(s);
  return concat([u32le(bytes.length), bytes]);
}

export function vecBytes32(v: Uint8Array[]): Uint8Array {
  return concat([u32le(v.length), ...v.map((x) => {
    if (x.length !== 32) throw new Error('expected 32-byte node');
    return x;
  })]);
}

export function toAnchorRoot(root: Uint8Array): number[] { return Array.from(root); }
export function toAnchorProof(proof: Uint8Array[]): number[][] { return proof.map((n) => Array.from(n)); }

export type SetRootArgs = { channel: string; epoch: bigint | number; root: Uint8Array };

export function buildSetRootIx(
  programId: PublicKey,
  accounts: {
    payer: PublicKey;
    protocolState: PublicKey;
    channelState: PublicKey;
    systemProgram: PublicKey;
  },
  args: SetRootArgs,
): TransactionInstruction {
  const data = concat([
    anchorDiscriminator('set_channel_merkle_root'),
    borshString(args.channel),
    u64le(args.epoch),
    args.root,
  ]);
  const keys = [
    { pubkey: accounts.payer, isSigner: true, isWritable: true },
    { pubkey: accounts.protocolState, isSigner: false, isWritable: true },
    { pubkey: accounts.channelState, isSigner: false, isWritable: true },
    { pubkey: accounts.systemProgram, isSigner: false, isWritable: false },
  ];
  return new TransactionInstruction({ programId, keys, data });
}

// ---------- Epoch Utilities & Constants ----------
export function getCurrentEpoch(epochDurationMs: number, startTimestamp = 0): bigint {
  return BigInt(Math.floor((Date.now() - startTimestamp) / epochDurationMs));
}

export function getEpochBounds(epoch: bigint, epochDurationMs: number, startTimestamp = 0): { start: number; end: number } {
  const start = startTimestamp + Number(epoch) * epochDurationMs;
  return { start, end: start + epochDurationMs - 1 };
}

export const PUMP_NAMESPACE: NamespaceConfig = { namespace: 'pump:', enabled: true };
export const DEFAULT_EPOCH_DURATION_MS = 5 * 60 * 1000; // 5 minutes
export const MAX_CLAIMS_PER_EPOCH = 4096;

export type ClaimOpenArgs = {
  channel: string;
  epoch: bigint | number;
  index: number;
  amount: bigint | number;
  id: string;
  proof: Uint8Array[];
};

export function buildClaimOpenIx(
  programId: PublicKey,
  accounts: {
    claimer: PublicKey;
    protocolState: PublicKey;
    channelState: PublicKey;
    mint: PublicKey;
    treasuryAta: PublicKey;
    claimerAta: PublicKey;
    tokenProgram: PublicKey;
    associatedTokenProgram: PublicKey;
    systemProgram: PublicKey;
  },
  args: ClaimOpenArgs,
): TransactionInstruction {
  const data = concat([
    anchorDiscriminator('claim_channel_open'),
    borshString(args.channel),
    u64le(args.epoch),
    u32le(args.index),
    u64le(args.amount),
    borshString(args.id),
    vecBytes32(args.proof),
  ]);
  const keys = [
    { pubkey: accounts.claimer, isSigner: true, isWritable: true },
    { pubkey: accounts.protocolState, isSigner: false, isWritable: true },
    { pubkey: accounts.channelState, isSigner: false, isWritable: true },
    { pubkey: accounts.mint, isSigner: false, isWritable: false },
    { pubkey: accounts.treasuryAta, isSigner: false, isWritable: true },
    { pubkey: accounts.claimerAta, isSigner: false, isWritable: true },
    { pubkey: accounts.tokenProgram, isSigner: false, isWritable: false },
    { pubkey: accounts.associatedTokenProgram, isSigner: false, isWritable: false },
    { pubkey: accounts.systemProgram, isSigner: false, isWritable: false },
  ];
  return new TransactionInstruction({ programId, keys, data });
}
