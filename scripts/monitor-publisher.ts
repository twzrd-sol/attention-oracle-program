
import { Connection, PublicKey, Keypair } from '@solana/web3.js';
// ...
  // Provider with dummy wallet (read-only)
  const provider = new AnchorProvider(connection, new Wallet(Keypair.generate()), {});
  const program = new Program(IDL, PROGRAM_ID, provider);

  // Derive PDA
  const subjectId = deriveSubjectId(TARGET_CHANNEL);
  const [channelConfigPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("channel_cfg_v2"), MINT.toBuffer(), subjectId.toBuffer()],
    PROGRAM_ID
  );

  console.log(`Checking channel: ${TARGET_CHANNEL}`);
  console.log(`PDA: ${channelConfigPda.toBase58()}`);

  try {
    // @ts-ignore
    const acc = await program.account.channelConfigV2.fetch(channelConfigPda);
    const lastSeq = acc.latestRootSeq.toNumber();
    
    // Note: We can't easily get the *timestamp* of the last update from the account data alone 
    // without fetching the slot time or storing a timestamp in the account (which we don't).
    // PROPOSAL: Add `updated_at` to ChannelConfigV2 in next upgrade.
    
    // For now, we print the sequence. External monitor should track if this number changes.
    console.log(`✅ Channel Active`);
    console.log(`   Latest Root Seq: ${lastSeq}`);
    console.log(`   Cutover Epoch: ${acc.cutoverEpoch.toNumber()}`);
    
    // Output JSON for external tools
    console.log(JSON.stringify({
        status: "ok",
        channel: TARGET_CHANNEL,
        seq: lastSeq,
        pda: channelConfigPda.toBase58()
    }));

  } catch (e: any) {
    console.error(`❌ FAILED to fetch channel state: ${e.message}`);
    process.exit(1);
  }
}

main();
