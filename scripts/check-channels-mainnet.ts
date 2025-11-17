import { Connection, PublicKey } from '@solana/web3.js';

async function main() {
  const connection = new Connection('https://api.mainnet-beta.solana.com', 'confirmed');

  // Current deployed program
  const programId = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
  const mint = new PublicKey('AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5');

  console.log('🔍 Checking mainnet PDA state...\n');
  console.log(`Program ID: ${programId.toBase58()}`);
  console.log(`Mint: ${mint.toBase58()}\n`);

  // Check Protocol State
  const [protocolState] = PublicKey.findProgramAddressSync(
    [Buffer.from('protocol'), mint.toBuffer()],
    programId
  );
  console.log('Protocol State PDA:', protocolState.toBase58());

  const protocolInfo = await connection.getAccountInfo(protocolState);
  if (protocolInfo) {
    console.log('  ✅ EXISTS (size: ' + protocolInfo.data.length + ' bytes)');
  } else {
    console.log('  ❌ Does not exist');
  }

  // Try to find ChannelState PDAs by querying program accounts
  console.log('\n📦 Scanning for ChannelState PDAs...');
  try {
    const accounts = await connection.getProgramAccounts(programId);
    console.log(`Found ${accounts.length} total program accounts\n`);

    // Filter for likely channel states (have "channel" in the seed)
    // ChannelState structure: [discriminator(8) + data]
    // Most will be larger than protocol state

    const channelStates = accounts.filter(acc => {
      // ChannelState is likely 300-400+ bytes
      // Skip small accounts (protocol state, config, etc.)
      return acc.account.data.length > 200;
    });

    console.log(`Potential ChannelState accounts: ${channelStates.length}\n`);

    if (channelStates.length > 0) {
      console.log('Channel State PDAs:');
      channelStates.forEach((acc, i) => {
        console.log(`  [${i + 1}] ${acc.pubkey.toBase58()} (${acc.account.data.length} bytes)`);
      });
    }

    // Also show all small accounts (might be protocol state or other config)
    const smallAccounts = accounts.filter(acc => acc.account.data.length <= 200);
    console.log(`\nSmall config accounts: ${smallAccounts.length}`);
    if (smallAccounts.length > 0 && smallAccounts.length <= 10) {
      smallAccounts.forEach((acc, i) => {
        console.log(`  [${i + 1}] ${acc.pubkey.toBase58()} (${acc.account.data.length} bytes)`);
      });
    }

  } catch (err) {
    console.error('Error querying program accounts:', err);
  }

  // Specifically check if claim-0001-test channel exists
  console.log('\n🔎 Checking for specific channel: claim-0001-test');
  try {
    const [channelState] = PublicKey.findProgramAddressSync(
      [
        Buffer.from('channel_state'),
        Buffer.from('claim-0001-test'),
        mint.toBuffer()
      ],
      programId
    );
    console.log(`Channel PDA: ${channelState.toBase58()}`);

    const channelInfo = await connection.getAccountInfo(channelState);
    if (channelInfo) {
      console.log('  ✅ EXISTS (size: ' + channelInfo.data.length + ' bytes)');
    } else {
      console.log('  ❌ Does not exist yet - needs initialization');
    }
  } catch (err) {
    console.error('Error checking channel:', err);
  }
}

main().catch(console.error);
