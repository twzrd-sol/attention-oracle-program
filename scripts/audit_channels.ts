import { Connection, PublicKey } from "@solana/web3.js";
import { keccak_256 } from "@noble/hashes/sha3";

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");
const CHANNEL_CONFIG_V2_SEED = Buffer.from("channel_cfg_v2");

const channels = [
  "spotify:oracle",
  "taste:4D10VsAC2YfWFJWNLjQvKY",
  "taste:4NrBF4X5zg1TB9URmwR8DX",
  "taste:4wvioRMysmHlrOoXwccaip",
  "taste:4yorLKmHnwOC2JrQNbpGJK",
  "taste:6erCEtes9StREhj65N2NhM",
  "taste:7FUVgijEyR1infZsNBpnr4",
  "twitch:bobross",
  "twitch:chilledcat_music",
  "twitch:chillhopradio",
  "twitch:emilycc",
  "twitch:emiru",
  "twitch:lacy",
  "twitch:leekbeats",
  "twitch:lofigirl",
  "twitch:monstercat",
  "twitch:ninja",
  "twitch:relaxbeats",
  "twitch:streammelody",
  "twitch:tokyotones",
  "youtube:cafe_bgm_jazz",
  "youtube:cafe_bgm_piano",
  "youtube:chillhop_jazz",
  "youtube:college_music",
  "youtube_lofi",
  "youtube:lofi_girl_sleep",
  "youtube:lofi_girl_synthwave",
  "youtube:steezyasfuck"
];

interface ChannelConfigV2 {
  version: number;
  bump: number;
  mint: PublicKey;
  subject: PublicKey;
  authority: PublicKey;
  cutoverEpoch: bigint;
  latestRootSeq: bigint;
  creatorWallet: PublicKey;
  creatorFeeBps: number;
}

function deriveSubjectId(channel: string): Buffer {
  // Match Rust: keccak256("channel:" + lowercase(channel))
  const lower = channel.toLowerCase();
  const input = Buffer.concat([Buffer.from("channel:"), Buffer.from(lower)]);
  return Buffer.from(keccak_256(input));
}

function parseChannelConfig(data: Buffer): ChannelConfigV2 | null {
  if (data.length < 100) return null;

  // Skip 8-byte discriminator
  let offset = 8;

  const version = data.readUInt8(offset); offset += 1;
  const bump = data.readUInt8(offset); offset += 1;
  const mint = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const subject = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const authority = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const cutoverEpoch = data.readBigUInt64LE(offset); offset += 8;
  const latestRootSeq = data.readBigUInt64LE(offset); offset += 8;
  const creatorWallet = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const creatorFeeBps = data.readUInt16LE(offset);

  return { version, bump, mint, subject, authority, cutoverEpoch, latestRootSeq, creatorWallet, creatorFeeBps };
}

async function main() {
  const rpcUrl = process.env.SOLANA_RPC_URL || "https://api.mainnet-beta.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");

  console.log("=== On-Chain Channel Audit ===\n");

  let totalRent = 0;
  let activeChannels = 0;
  let inactiveChannels = 0;

  const results: { channel: string; status: string; rootSeq?: string; rent?: number; creatorFee?: string }[] = [];

  for (const channel of channels) {
    const subjectId = deriveSubjectId(channel);
    const [pda] = PublicKey.findProgramAddressSync(
      [CHANNEL_CONFIG_V2_SEED, CCM_MINT.toBuffer(), subjectId],
      PROGRAM_ID
    );

    const info = await connection.getAccountInfo(pda);

    if (info) {
      const rent = info.lamports / 1e9;
      totalRent += rent;
      activeChannels++;

      const config = parseChannelConfig(info.data);
      if (config) {
        results.push({
          channel,
          status: "ON-CHAIN",
          rootSeq: config.latestRootSeq.toString(),
          rent,
          creatorFee: (config.creatorFeeBps / 100).toFixed(1) + "%"
        });
      }
    } else {
      inactiveChannels++;
      results.push({ channel, status: "NOT ON-CHAIN" });
    }
  }

  // Print results
  console.log("Active Channels (on-chain):\n");
  for (const r of results.filter(r => r.status === "ON-CHAIN")) {
    console.log("  " + r.channel);
    console.log("    root_seq: " + r.rootSeq + ", rent: " + r.rent?.toFixed(4) + " SOL, creator_fee: " + r.creatorFee);
  }

  console.log("\n\nInactive Channels (DB only):\n");
  for (const r of results.filter(r => r.status !== "ON-CHAIN")) {
    console.log("  " + r.channel);
  }

  console.log("\n=== Summary ===");
  console.log("Active on-chain: " + activeChannels);
  console.log("DB only (legacy): " + inactiveChannels);
  console.log("Total rent held: " + totalRent.toFixed(4) + " SOL");
}

main().catch(console.error);
