import { PublicKey } from '@solana/web3.js';

const MINT = new PublicKey('AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5');
const TOKEN_2022 = new PublicKey('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPvZeS');
const ATP = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
const PROTOCOL_STATE = new PublicKey('FEwsakAJZrEojzRrNJCwmS91Jn5gamuDjk1GGZXMNYwr');
const CLAIMER = new PublicKey('DV879FdRD3LAot5MTfzftrVmMv9WC9giUeYsDnmTSZh1');

const [treasuryAta] = PublicKey.findProgramAddressSync(
  [MINT.toBuffer(), PROTOCOL_STATE.toBuffer(), TOKEN_2022.toBuffer()],
  ATP
);

const [claimerAta] = PublicKey.findProgramAddressSync(
  [MINT.toBuffer(), CLAIMER.toBuffer(), TOKEN_2022.toBuffer()],
  ATP
);

console.log('Treasury ATA:', treasuryAta.toBase58());
console.log('Claimer ATA:', claimerAta.toBase58());
console.log('\nThese should match what we derive in the submit script');
