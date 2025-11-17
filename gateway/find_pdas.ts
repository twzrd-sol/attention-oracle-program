import { Connection, PublicKey } from '@solana/web3.js';

const RPC_URL = 'https://mainnet.helius-rpc.com/?api-key=1fc5da66-dd53-4041-9069-7300d1787973';
const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const SEED_KEY = new PublicKey('Hrbu75aVwq5sMd7u1z7gGLs1BmHbyGca1gLg5EL87EZF');

async function main() {
  const conn = new Connection(RPC_URL, 'confirmed');

  console.log(`Searching for PDAs derived from: ${SEED_KEY.toBase58()}\n`);

  // Try common seed patterns
  const seedPatterns = [
    { name: 'protocol', seed: Buffer.from('protocol') },
    { name: 'channel_state', seed: Buffer.from('channel_state') },
    { name: 'channel', seed: Buffer.from('channel') },
    { name: 'passport', seed: Buffer.from('passport') },
    { name: 'epoch', seed: Buffer.from('epoch') },
  ];

  console.log('=== Checking derived PDAs ===\n');

  for (const pattern of seedPatterns) {
    try {
      const [pda, bump] = PublicKey.findProgramAddressSync(
        [pattern.seed, SEED_KEY.toBuffer()],
        PROGRAM_ID
      );

      const info = await conn.getAccountInfo(pda);
      if (info) {
        console.log(`✓ Found PDA with seed "${pattern.name}":`);
        console.log(`  Address: ${pda.toBase58()}`);
        console.log(`  Bump: ${bump}`);
        console.log(`  Lamports: ${(info.lamports / 1e9).toFixed(6)} SOL`);
        console.log(`  Owner: ${info.owner.toBase58()}`);
        console.log(`  Data size: ${info.data.length} bytes\n`);
      }
    } catch (e) {
      // Ignore
    }
  }

  // Query all ChannelState accounts
  console.log('=== Querying all ChannelState accounts ===\n');

  const CHANNEL_STATE_LEN = 1072; // 8 + 1 + 1 + 32 + 32 + 8 + 10*(8 + 32 + 2 + 1024)

  const accounts = await conn.getProgramAccounts(PROGRAM_ID, {
    filters: [
      {
        dataSize: CHANNEL_STATE_LEN,
      },
    ],
  });

  console.log(`Found ${accounts.length} ChannelState accounts total\n`);

  if (accounts.length === 0) {
    console.log('✓ No ChannelState accounts found (clean state)\n');
    return;
  }

  // Check if any have this key as streamer
  let matches = 0;
  const seedKeyHex = SEED_KEY.toBuffer().toString('hex');

  for (const acc of accounts) {
    const data = acc.account.data;
    if (data.length >= 74) {
      // Streamer is at offset 42 (after disc 8 + version 1 + bump 1 + mint 32)
      const streamerKey = data.slice(42, 74);
      if (streamerKey.toString('hex') === seedKeyHex) {
        matches++;
        console.log(`✓ ChannelState associated with this key:`);
        console.log(`  Address: ${acc.pubkey.toBase58()}`);
        console.log(`  Lamports: ${(acc.account.lamports / 1e9).toFixed(6)} SOL`);
        console.log(`  Status: Created during v3 or earlier\n`);
      }
    }
  }

  if (matches === 0) {
    console.log(`✗ No ChannelState accounts found for key ${SEED_KEY.toBase58()}\n`);
  } else {
    console.log(`Summary: Found ${matches} ChannelState account(s) associated with this key`);
  }
}

main().catch(console.error);
