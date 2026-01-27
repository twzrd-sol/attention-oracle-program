import { Connection, PublicKey } from "@solana/web3.js";
import { createHash } from "crypto";

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");
const CHANNEL_CONFIG_V2_SEED = Buffer.from("channel_cfg_v2");

const channels = [
  "twitch:lofigirl",
  "twitch:emilycc",
  "twitch:lacy",
  "twitch:bobross",
  "youtube_lofi",
  "twitch:chilledcat_music"
];

async function main() {
  const rpcUrl = process.env.SOLANA_RPC_URL || "https://api.mainnet-beta.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");

  console.log("=== Checking Legacy Channel PDAs ===\n");

  let totalRent = 0;

  for (const channel of channels) {
    // Derive subject_id = sha256(channel)
    const subjectId = createHash("sha256").update(channel).digest();

    const [pda] = PublicKey.findProgramAddressSync(
      [CHANNEL_CONFIG_V2_SEED, CCM_MINT.toBuffer(), subjectId],
      PROGRAM_ID
    );

    const info = await connection.getAccountInfo(pda);
    if (info) {
      const rent = info.lamports / 1e9;
      totalRent += rent;
      console.log(channel + ": " + pda.toBase58().slice(0,8) + "... EXISTS (" + rent.toFixed(4) + " SOL)");
    } else {
      console.log(channel + ": NOT ON-CHAIN");
    }
  }

  console.log("\n=== Summary ===");
  console.log("Total reclaimable rent: " + totalRent.toFixed(4) + " SOL");
}

main().catch(console.error);
