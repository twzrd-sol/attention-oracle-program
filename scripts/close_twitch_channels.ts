import * as anchor from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import fs from "fs";
import { keccak_256 } from "@noble/hashes/sha3";

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const MINT = new PublicKey("ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe");
const RPC_URL = "https://mainnet.helius-rpc.com/?api-key=4323eb4e-974c-49da-bbb9-ea11b1165a25";

// Twitch channels from logs
const TWITCH_CHANNELS = [
  "quin69", "kato_junichi0817", "eslcs", "jynxzi", "sasavot", "xqc",
  "jasontheween", "fps_shaka", "moonmoon", "summit1g", "k4sen", "alanzoka",
  "traytonlol", "stableronaldo", "rubius", "zarbex", "yourragegaming",
  "hasanabi", "lirik", "adapt", "lacy", "dakillzor", "caseoh_", "nmplol",
  "emiru", "zackrawrr"
];

function deriveSubjectId(channel: string): PublicKey {
  const lower = channel.toLowerCase();
  const data = Buffer.concat([Buffer.from("channel:"), Buffer.from(lower)]);
  const hash = keccak_256(data);
  return new PublicKey(hash);
}

async function closeChannel(connection: Connection, admin: Keypair, channel: string) {
  const subjectId = deriveSubjectId(channel);

  // Legacy Twitch channels were created without mint in seeds
  const [channelState] = PublicKey.findProgramAddressSync(
    [Buffer.from("channel_state"), subjectId.toBuffer()],
    PROGRAM_ID
  );

  console.log("Closing " + channel + "...");
  console.log("  Subject: " + subjectId.toBase58());
  console.log("  Channel State: " + channelState.toBase58());

  try {
    // Discriminator for "force_close_channel_state_legacy"
    const discriminator = Buffer.from([0x8a, 0x5e, 0xfd, 0xff, 0x72, 0x7c, 0x27, 0x71]);
    const data = Buffer.concat([discriminator, subjectId.toBuffer()]);

    const tx = new anchor.web3.Transaction().add(
      new anchor.web3.TransactionInstruction({
        programId: PROGRAM_ID,
        keys: [
          { pubkey: admin.publicKey, isSigner: true, isWritable: true },
          { pubkey: channelState, isSigner: false, isWritable: true },
        ],
        data: data,
      })
    );

    const sig = await anchor.web3.sendAndConfirmTransaction(connection, tx, [admin], {
      commitment: "confirmed",
    });

    console.log("✅ Closed " + channel + ": " + sig);
    return sig;
  } catch (err: any) {
    console.error("❌ Failed to close " + channel + ": " + err.message);
    return null;
  }
}

async function main() {
  const connection = new Connection(RPC_URL, "confirmed");
  const adminKeypair = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync("/home/twzrd/.config/solana/id.json", "utf-8")))
  );

  console.log("Admin: " + adminKeypair.publicKey.toBase58());
  console.log("Closing " + TWITCH_CHANNELS.length + " Twitch channels...\n");

  let closed = 0;
  for (const channel of TWITCH_CHANNELS) {
    const sig = await closeChannel(connection, adminKeypair, channel);
    if (sig) closed++;
    await new Promise(r => setTimeout(r, 1000));
  }

  console.log("\n✅ Closed " + closed + "/" + TWITCH_CHANNELS.length + " channels");
  console.log("💰 Recovered ~" + (closed * 0.04).toFixed(2) + " SOL in rent");
}

main().catch(console.error);
