import { PublicKey } from '@solana/web3.js';

const MINT = new PublicKey('AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5');
const TOKEN_2022 = new PublicKey('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPvZeS');
const ATP = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
const PROTOCOL_STATE = new PublicKey('FEwsakAJZrEojzRrNJCwmS91Jn5gamuDjk1GGZXMNYwr');

console.log('Testing different ATA derivation formulas:\n');

// Current formula (with token_program)
const [ata1] = PublicKey.findProgramAddressSync(
  [MINT.toBuffer(), PROTOCOL_STATE.toBuffer(), TOKEN_2022.toBuffer()],
  ATP
);
console.log('1. With token_program in seeds:');
console.log('   ', ata1.toBase58(), '\n');

// Without token_program
const [ata2] = PublicKey.findProgramAddressSync(
  [MINT.toBuffer(), PROTOCOL_STATE.toBuffer()],
  ATP
);
console.log('2. Without token_program:');
console.log('   ', ata2.toBase58(), '\n');

// With "associated_token_program" magic string
const [ata3] = PublicKey.findProgramAddressSync(
  [Buffer.from('associated_token_program'), MINT.toBuffer(), PROTOCOL_STATE.toBuffer()],
  ATP
);
console.log('3. With "associated_token_program" prefix:');
console.log('   ', ata3.toBase58(), '\n');

// Order variations
const [ata4] = PublicKey.findProgramAddressSync(
  [PROTOCOL_STATE.toBuffer(), MINT.toBuffer(), TOKEN_2022.toBuffer()],
  ATP
);
console.log('4. Reversed order (authority, mint, token_program):');
console.log('   ', ata4.toBase58(), '\n');

// Try with standard Token program instead
const TOKEN_PROGRAM = new PublicKey('TokenkegQfeZyiNwAJsyFbPVwwQQforro5QWGrWasting');
const [ata5] = PublicKey.findProgramAddressSync(
  [MINT.toBuffer(), PROTOCOL_STATE.toBuffer(), TOKEN_PROGRAM.toBuffer()],
  ATP
);
console.log('5. With standard Token program:');
console.log('   ', ata5.toBase58(), '\n');

console.log('\nOriginal target: 5A3Nwosu7dxnGK72WYCUrRfWdWohuL7nBuxxJdKkri3D');
console.log('Matches #1?', ata1.toBase58() === '5A3Nwosu7dxnGK72WYCUrRfWdWohuL7nBuxxJdKkri3D' ? '✅ YES' : '❌ NO');
console.log('Matches #2?', ata2.toBase58() === '5A3Nwosu7dxnGK72WYCUrRfWdWohuL7nBuxxJdKkri3D' ? '✅ YES' : '❌ NO');
console.log('Matches #3?', ata3.toBase58() === '5A3Nwosu7dxnGK72WYCUrRfWdWohuL7nBuxxJdKkri3D' ? '✅ YES' : '❌ NO');
console.log('Matches #4?', ata4.toBase58() === '5A3Nwosu7dxnGK72WYCUrRfWdWohuL7nBuxxJdKkri3D' ? '✅ YES' : '❌ NO');
console.log('Matches #5?', ata5.toBase58() === '5A3Nwosu7dxnGK72WYCUrRfWdWohuL7nBuxxJdKkri3D' ? '✅ YES' : '❌ NO');
