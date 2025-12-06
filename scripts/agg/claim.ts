/**
 * Submit a claim_channel_open for a given proof.
 *
 * Example:
 *   ts-node --esm scripts/agg/claim.ts \
 *     --rpc https://api.devnet.solana.com \
 *     --keypair ~/.config/solana/id.json \
 *     --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
 *     --mint CCM_MINT_PUBKEY \
 *     --channel <pump_token_mint> \
 *     --epoch 123456 \
 *     --index 42 \
 *     --amount 1000000000 \
 *     --id some-id \
 *     --namespace pump: \
 *     --leaf-version 0 \
 *     --proof-file ./out/epoch-123456.json
 */

import { readFileSync } from 'node:fs';
import { Connection, Keypair, PublicKey, SystemProgram, Transaction, sendAndConfirmTransaction } from '@solana/web3.js';
import { getAssociatedTokenAddressSync, ASSOCIATED_TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID } from '@solana/spl-token';
import { PROGRAM_ID, buildClaimOpenIx, deriveSubjectId } from './helpers.ts';

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

type ProofFile = {
  programId: string;
  mint: string;
  channel: string;
  epoch: string;
  subject: string;
  leafVersion: number;
  root: string;
  claims: { claimer: string; amount: string; id: string; index: number }[];
  nodes: string[][]; // hex strings per claim
};

async function main() {
  const rpc = req('rpc');
  const keypairPath = req('keypair');
  const programId = new PublicKey(argv['program-id'] ?? PROGRAM_ID.toBase58());
  const mint = new PublicKey(req('mint'));
  const channel = req('channel');
  const epoch = BigInt(req('epoch'));
  const index = Number(req('index'));
  const amount = BigInt(req('amount'));
  const id = req('id');
  const namespace = argv['namespace'] as string | undefined;
  const leafVersion = Number(argv['leaf-version'] ?? '0');
  const proofFilePath = req('proof-file');

  const connection = new Connection(rpc, 'confirmed');
  const claimer = Keypair.fromSecretKey(new Uint8Array(JSON.parse(readFileSync(keypairPath, 'utf8'))));

  const proofFile: ProofFile = JSON.parse(readFileSync(proofFilePath, 'utf8'));
  const nodesHex = proofFile.nodes[index];
  if (!nodesHex) throw new Error(`No proof for index ${index}`);
  const proof = nodesHex.map((h) => Uint8Array.from(Buffer.from(h.replace(/^0x/, ''), 'hex')));

  // PDAs & ATAs
  const subject = deriveSubjectId(channel, namespace);
  const [protocolState] = PublicKey.findProgramAddressSync(
    [Buffer.from('protocol'), mint.toBytes()],
    programId,
  );
  const [channelState] = PublicKey.findProgramAddressSync(
    [Buffer.from('channel_state'), mint.toBytes(), Buffer.from(subject)],
    programId,
  );

  const treasuryAta = getAssociatedTokenAddressSync(
    mint,
    protocolState,
    true,
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,
  );
  const claimerAta = getAssociatedTokenAddressSync(
    mint,
    claimer.publicKey,
    false,
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,
  );

  const ix = buildClaimOpenIx(
    programId,
    {
      claimer: claimer.publicKey,
      protocolState,
      channelState,
      mint,
      treasuryAta,
      claimerAta,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    },
    { channel, epoch, index, amount, id, proof },
  );

  const tx = new Transaction().add(ix);
  tx.feePayer = claimer.publicKey;
  const sig = await sendAndConfirmTransaction(connection, tx, [claimer], { commitment: 'confirmed' });
  console.log(`Claimed. Tx: ${sig}`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
