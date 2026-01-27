import { Connection, PublicKey } from "@solana/web3.js";
import { keccak_256 } from "@noble/hashes/sha3";

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");

async function main() {
  const channel = "spotify:oracle";
  const lower = channel.toLowerCase();
  const input = Buffer.concat([Buffer.from("channel:"), Buffer.from(lower)]);
  const hash = Buffer.from(keccak_256(input));

  const [pda] = PublicKey.findProgramAddressSync(
    [Buffer.from("channel_cfg_v2"), CCM_MINT.toBuffer(), hash],
    PROGRAM_ID
  );

  const rpcUrl = process.env.SOLANA_RPC_URL || "https://api.mainnet-beta.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");
  const info = await connection.getAccountInfo(pda);

  console.log("Channel: " + channel);
  console.log("PDA: " + pda.toBase58());
  console.log("Exists: " + (info ? "YES" : "NO (CLOSED)"));
  if (info) console.log("Rent: " + (info.lamports / 1e9) + " SOL");
}

main().catch(console.error);
