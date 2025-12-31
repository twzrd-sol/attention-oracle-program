import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, Connection, Keypair } from "@solana/web3.js";
import { readFileSync } from "fs";
import { homedir } from "os";
import { keccak_256 } from "@noble/hashes/sha3";

// Load the IDL
const IDL = JSON.parse(readFileSync("./target/idl/token_2022.json", "utf-8"));

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");

// Channels to migrate (all with prior publishes)
const CHANNELS = [
  "youtube_lofi",
  "twitch:emiru",
  "twitch:emilycc",
  "twitch:chilledcat_music",
  "twitch:chillhopradio",
  "twitch:lacy",
  "twitch:leekbeats",
  "twitch:lofigirl",
  "twitch:ninja",
  "twitch:relaxbeats",
  "youtube:cafe_bgm_piano",
  "youtube:chillhop_jazz",
  "youtube:college_music",
  "youtube:lofi_girl_synthwave",
  "youtube:lofi_girl_sleep",
  "youtube:steezyasfuck",
];

// Default creator wallet (treasury) - no fees for now
const DEFAULT_CREATOR = new PublicKey("2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD");
const DEFAULT_FEE_BPS = 0;

async function main() {
  const rpcUrl = process.env.SOLANA_RPC || "https://api.mainnet-beta.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");

  // Load wallet from default keypair
  const keypairPath = `${homedir()}/.config/solana/id.json`;
  const keypairData = JSON.parse(readFileSync(keypairPath, "utf-8"));
  const wallet = Keypair.fromSecretKey(Uint8Array.from(keypairData));

  console.log("Wallet:", wallet.publicKey.toBase58());
  console.log("RPC:", rpcUrl);

  const provider = new anchor.AnchorProvider(
    connection,
    new anchor.Wallet(wallet),
    { commitment: "confirmed" }
  );

  const program = new Program(IDL, provider);

  // Derive protocol state PDA
  const [protocolState] = PublicKey.findProgramAddressSync(
    [Buffer.from("protocol"), CCM_MINT.toBuffer()],
    PROGRAM_ID
  );

  console.log("Protocol state:", protocolState.toBase58());
  console.log("\nMigrating channels...\n");

  for (const channel of CHANNELS) {
    const subjectId = deriveSubjectId(channel);
    const [channelConfig] = PublicKey.findProgramAddressSync(
      [Buffer.from("channel_cfg_v2"), CCM_MINT.toBuffer(), subjectId.toBuffer()],
      PROGRAM_ID
    );

    console.log(`Channel: ${channel}`);
    console.log(`  Config PDA: ${channelConfig.toBase58()}`);

    try {
      const accountInfo = await connection.getAccountInfo(channelConfig);
      if (!accountInfo) {
        console.log(`  ⚠️ Account does not exist, skipping`);
        continue;
      }

      const currentLen = accountInfo.data.length;
      console.log(`  Current size: ${currentLen} bytes`);

      if (currentLen === 482) {
        console.log(`  ✅ Already migrated`);
        continue;
      }

      if (currentLen !== 442) {
        console.log(`  ⚠️ Unexpected size ${currentLen}, skipping`);
        continue;
      }

      console.log(`  🔄 Migrating...`);

      const tx = await program.methods
        .migrateChannelConfigV2(channel, DEFAULT_CREATOR, DEFAULT_FEE_BPS)
        .accounts({
          payer: wallet.publicKey,
          protocolState: protocolState,
          channelConfig: channelConfig,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .rpc();

      console.log(`  ✅ Migrated: ${tx}`);
    } catch (err: any) {
      console.log(`  ❌ Error: ${err.message}`);
    }

    console.log();
  }

  console.log("Done!");
}

function deriveSubjectId(channel: string): PublicKey {
  const lower = channel.toLowerCase();
  // Match Rust: keccak_hashv(&[b"channel:", lower.as_slice()])
  const prefix = Buffer.from("channel:");
  const channelBytes = Buffer.from(lower);
  const combined = Buffer.concat([prefix, channelBytes]);
  const hash = keccak_256(combined);
  return new PublicKey(hash);
}

main().catch(console.error);
