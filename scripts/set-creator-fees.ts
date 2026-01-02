/**
 * ⚠️  BLOCKED: This script requires a program upgrade before use.
 *
 * The `update_channel_creator_fee` instruction exists in the repo but is
 * NOT YET DEPLOYED to mainnet. Running this script will fail with:
 *   "Transaction simulation failed: Error processing Instruction 0:
 *    custom program error: 0x..."
 *
 * Before using this script:
 *   1. Deploy the updated token_2022 program with `update_channel_creator_fee`
 *   2. Verify deployment via `solana program show GnGzNds...`
 *   3. Update docs/LIVE_STATUS.md with new deployment slot
 *
 * See: programs/token_2022/src/lib.rs:140-147
 */

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

// Channels to update
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

// 30% creator fee = 3000 bps
const CREATOR_FEE_BPS = 3000;

async function main() {
  const rpcUrl = process.env.SOLANA_RPC || "https://api.mainnet-beta.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");

  // Load wallet from default keypair
  const keypairPath = `${homedir()}/.config/solana/id.json`;
  const keypairData = JSON.parse(readFileSync(keypairPath, "utf-8"));
  const wallet = Keypair.fromSecretKey(Uint8Array.from(keypairData));

  console.log("Wallet:", wallet.publicKey.toBase58());
  console.log("RPC:", rpcUrl);
  console.log(`Setting creator fee to ${CREATOR_FEE_BPS} bps (${CREATOR_FEE_BPS / 100}%)\n`);

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
  console.log("\nUpdating creator fees...\n");

  for (const channel of CHANNELS) {
    const subjectId = deriveSubjectId(channel);
    const [channelConfig] = PublicKey.findProgramAddressSync(
      [Buffer.from("channel_cfg_v2"), CCM_MINT.toBuffer(), subjectId.toBuffer()],
      PROGRAM_ID
    );

    console.log(`Channel: ${channel}`);

    try {
      const tx = await program.methods
        .updateChannelCreatorFee(channel, CREATOR_FEE_BPS)
        .accounts({
          admin: wallet.publicKey,
          protocolState: protocolState,
          channelConfig: channelConfig,
        })
        .rpc();

      console.log(`  ✅ Updated: ${tx}`);
    } catch (err: any) {
      console.log(`  ❌ Error: ${err.message}`);
    }

    console.log();
  }

  console.log("Done!");
}

function deriveSubjectId(channel: string): PublicKey {
  const lower = channel.toLowerCase();
  const prefix = Buffer.from("channel:");
  const channelBytes = Buffer.from(lower);
  const combined = Buffer.concat([prefix, channelBytes]);
  const hash = keccak_256(combined);
  return new PublicKey(hash);
}

main().catch(console.error);
