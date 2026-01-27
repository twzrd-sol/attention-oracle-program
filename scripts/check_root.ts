import { Connection, PublicKey } from "@solana/web3.js";
import { keccak_256 } from "@noble/hashes/sha3";

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");

async function main() {
  const channel = "attention:audio";
  const input = Buffer.concat([Buffer.from("channel:"), Buffer.from(channel.toLowerCase())]);
  const hash = Buffer.from(keccak_256(input));
  
  const [pda] = PublicKey.findProgramAddressSync(
    [Buffer.from("channel_cfg_v2"), CCM_MINT.toBuffer(), hash],
    PROGRAM_ID
  );
  
  const rpcUrl = process.env.SOLANA_RPC_URL || "https://api.mainnet-beta.solana.com";
  const conn = new Connection(rpcUrl);
  const info = await conn.getAccountInfo(pda);
  
  console.log("Channel:", channel);
  console.log("PDA:", pda.toBase58());
  console.log("Exists:", !!info);
  
  if (info) {
    // Parse ChannelConfigV2 - skip discriminator (8), version (1), bump (1), mint (32)
    let offset = 8 + 1 + 1 + 32;
    // subject_id: [u8; 32]
    offset += 32;
    // cumulative_root: [u8; 32]
    const root = info.data.slice(offset, offset + 32);
    console.log("On-chain root:", Buffer.from(root).toString("hex"));
  }
}
main().catch(console.error);
