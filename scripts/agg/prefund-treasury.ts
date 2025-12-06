/**
 * Prefund the protocol treasury ATA for a CCM mint (Token-2022).
 *
 * - Creates ATA for owner = protocol_state PDA (seed: ["protocol", mint])
 * - Creates payer's ATA if missing
 * - Transfers an initial CCM amount from payer -> treasury ATA
 *
 * Usage:
 *   pnpm agg:prefund -- \
 *     --rpc https://api.devnet.solana.com \
 *     --keypair ~/.config/solana/id.json \
 *     --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
 *     --mint <CCM_MINT> \
 *     --amount 1000000000
 */

import { readFileSync } from 'node:fs';
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
} from '@solana/web3.js';
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  createAssociatedTokenAccountIdempotentInstruction,
  createTransferCheckedInstruction,
  getAssociatedTokenAddressSync,
  getMint,
} from '@solana/spl-token';

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

const PROTOCOL_SEED = Buffer.from('protocol');

async function main() {
  const rpc = req('rpc');
  const keypairPath = req('keypair');
  const programId = new PublicKey(req('program-id'));
  const mint = new PublicKey(req('mint'));
  const amount = BigInt(req('amount'));

  const connection = new Connection(rpc, 'confirmed');
  const payer = Keypair.fromSecretKey(new Uint8Array(JSON.parse(readFileSync(keypairPath, 'utf8'))));

  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, mint.toBytes()],
    programId,
  );

  const decimals = (await getMint(connection, mint, 'confirmed', TOKEN_2022_PROGRAM_ID)).decimals;

  const treasuryAta = getAssociatedTokenAddressSync(
    mint,
    protocolState,
    true,
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,
  );
  const payerAta = getAssociatedTokenAddressSync(
    mint,
    payer.publicKey,
    false,
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,
  );

  console.log('Prefund config');
  console.log('  Program  :', programId.toBase58());
  console.log('  Mint     :', mint.toBase58());
  console.log('  Decimals :', decimals);
  console.log('  Protocol :', protocolState.toBase58());
  console.log('  Treasury :', treasuryAta.toBase58());
  console.log('  Source   :', payerAta.toBase58());
  console.log('  Amount   :', amount.toString());

  const tx = new Transaction();

  // Ensure ATAs exist
  tx.add(
    createAssociatedTokenAccountIdempotentInstruction(
      payer.publicKey,
      payerAta,
      payer.publicKey,
      mint,
      TOKEN_2022_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID,
    ),
  );

  tx.add(
    createAssociatedTokenAccountIdempotentInstruction(
      payer.publicKey,
      treasuryAta,
      protocolState,
      mint,
      TOKEN_2022_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID,
    ),
  );

  // Transfer from payer to protocol treasury
  tx.add(
    createTransferCheckedInstruction(
      payerAta,
      mint,
      treasuryAta,
      payer.publicKey,
      Number(amount),
      decimals,
      [],
      TOKEN_2022_PROGRAM_ID,
    ),
  );

  tx.feePayer = payer.publicKey;
  tx.recentBlockhash = (await connection.getLatestBlockhash('finalized')).blockhash;

  const sig = await sendAndConfirmTransaction(connection, tx, [payer], { commitment: 'confirmed' });
  console.log('✅ Prefunded. Tx:', sig);
  console.log(`Explorer: https://explorer.solana.com/tx/${sig}?cluster=devnet`);
}

main().catch((e) => { console.error(e); process.exit(1); });

