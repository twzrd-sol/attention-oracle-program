import { PublicKey } from '@solana/web3.js';
import { getAssociatedTokenAddress, TOKEN_2022_PROGRAM_ID } from '@solana/spl-token';

const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const MINT = new PublicKey('AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5');

async function main() {
  // Derive protocol state PDA
  const [protocolState] = PublicKey.findProgramAddressSync(
    [Buffer.from('protocol'), MINT.toBuffer()],
    PROGRAM_ID,
  );

  console.log('Protocol State:', protocolState.toBase58());
  console.log('TOKEN_2022_PROGRAM_ID:', TOKEN_2022_PROGRAM_ID.toBase58());

  // Derive treasury ATA
  const treasuryAta = await getAssociatedTokenAddress(
    MINT,
    protocolState,
    true, // allowOwnerOffCurve
    TOKEN_2022_PROGRAM_ID,
  );

  console.log('Treasury ATA:', treasuryAta.toBase58());
  console.log('\nExpected from init-gng-treasury-ata.ts: Fmwebxkgwhpi1vKQnvvypRNEV2DKnzck6Kd3o3zxUCNa');
  console.log('Match?', treasuryAta.toBase58() === 'Fmwebxkgwhpi1vKQnvvypRNEV2DKnzck6Kd3o3zxUCNa' ? '✅ YES' : '❌ NO');
}

main().catch(console.error);
