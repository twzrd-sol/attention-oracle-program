/**
 * Publish a Merkle root for a pump.fun channel epoch.
 *
 * Usage (example):
 *   ts-node --esm scripts/agg/publish-epoch.ts \
 *     --rpc https://api.devnet.solana.com \
 *     --keypair ~/.config/solana/id.json \
 *     --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
 *     --mint CCM_MINT_PUBKEY \
 *     --channel <pump_token_mint> \
 *     --epoch 123456 \
 *     --leaf-version 0 \
 *     --namespace pump: \
 *     --claims ./claims.json \
 *     --out ./out/epoch-123456.json
 */

import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { PublicKey, Connection, Keypair, SystemProgram, sendAndConfirmTransaction, Transaction } from '@solana/web3.js';
import {
  deriveSubjectId,
  computeLeafByVersion,
  merkleRoot,
  merkleProof,
  PROGRAM_ID,
  buildSetRootIx,
  toHex,
} from './helpers.ts';

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
  const rpc = req('rpc');
  const keypairPath = req('keypair');
  const programId = new PublicKey(argv['program-id'] ?? PROGRAM_ID.toBase58());
  const mint = new PublicKey(req('mint'));
  const channel = req('channel');
  const epoch = BigInt(req('epoch'));
  const leafVersion = Number(argv['leaf-version'] ?? '0');
  const namespace = argv['namespace'] as string | undefined;
  const claimsPath = req('claims');
  const outPath = argv['out'] as string | undefined;
  const dryRun = argv['dry-run'] === 'true';

  const connection = new Connection(rpc, 'confirmed');
  const payer = Keypair.fromSecretKey(new Uint8Array(JSON.parse(readFileSync(keypairPath, 'utf8'))));

  type ClaimRow = { claimer: string; amount: string | number; id: string };
  const claims: ClaimRow[] = JSON.parse(readFileSync(claimsPath, 'utf8'));

  // Subject derivation must match on-chain namespace toggle
  const subject = deriveSubjectId(channel, namespace);

  // Build leaves in array order as indices
  const leaves = claims.map((row, i) =>
    computeLeafByVersion(
      leafVersion,
      { claimer: new PublicKey(row.claimer), index: i, amount: BigInt(row.amount), id: row.id },
      { subject: new PublicKey(subject), epoch }
    )
  );
  const root = merkleRoot(leaves);

  // PDAs
  const [protocolState] = PublicKey.findProgramAddressSync(
    [Buffer.from('protocol'), mint.toBytes()],
    programId,
  );
  const [channelState] = PublicKey.findProgramAddressSync(
    [Buffer.from('channel_state'), mint.toBytes(), Buffer.from(subject)],
    programId,
  );

  // Instruction
  const ix = buildSetRootIx(
    programId,
    {
      payer: payer.publicKey,
      protocolState,
      channelState,
      systemProgram: SystemProgram.programId,
    },
    { channel, epoch, root },
  );

  if (dryRun) {
    console.log(`Root: 0x${toHex(root)} (subject=${new PublicKey(subject).toBase58()})`);
  } else {
    const tx = new Transaction().add(ix);
    tx.feePayer = payer.publicKey;
    const sig = await sendAndConfirmTransaction(connection, tx, [payer], { commitment: 'confirmed' });
    console.log(`Published root. Tx: ${sig}`);
  }

  if (outPath) {
    const outDir = dirname(resolve(outPath));
    mkdirSync(outDir, { recursive: true });
    // Emit basic proof file for clients: { root, subject, leaves: [hex], proofs: [[hex...]] }
    const proofs = leaves.map((_, i) => merkleProof(leaves, i));
    const json = {
      programId: programId.toBase58(),
      mint: mint.toBase58(),
      channel,
      epoch: epoch.toString(),
      subject: new PublicKey(subject).toBase58(),
      leafVersion,
      root: `0x${toHex(root)}`,
      claims: claims.map((c, i) => ({ ...c, index: i })),
      nodes: proofs.map((p) => p.map((n) => `0x${toHex(n)}`)),
    };
    writeFileSync(outPath, JSON.stringify(json, null, 2));
    console.log(`Wrote proofs: ${outPath}`);
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
