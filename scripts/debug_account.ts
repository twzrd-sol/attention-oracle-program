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
  
  if (!info) {
    console.log("Account not found");
    return;
  }
  
  console.log("Account data length:", info.data.length);
  console.log("Full account hex:", Buffer.from(info.data).toString("hex"));
  console.log("");
  
  // ChannelConfigV2 layout (from IDL):
  // discriminator: 8 bytes
  // version: u8 (1 byte)
  // bump: u8 (1 byte)
  // mint: Pubkey (32 bytes)
  // subject_id: [u8; 32]
  // cumulative_root: [u8; 32]
  // root_seq: u64 (8 bytes)
  // cutover_epoch: i64 (8 bytes)
  // curator: Pubkey (32 bytes)
  // curator_fee_bps: u16 (2 bytes)
  // padding/reserved...
  
  let offset = 0;
  
  const discriminator = info.data.slice(offset, offset + 8);
  console.log("Discriminator:", Buffer.from(discriminator).toString("hex"));
  offset += 8;
  
  const version = info.data[offset];
  console.log("Version:", version);
  offset += 1;
  
  const bump = info.data[offset];
  console.log("Bump:", bump);
  offset += 1;
  
  const mint = new PublicKey(info.data.slice(offset, offset + 32));
  console.log("Mint:", mint.toBase58());
  offset += 32;
  
  const subjectId = Buffer.from(info.data.slice(offset, offset + 32));
  console.log("Subject ID:", subjectId.toString("hex"));
  offset += 32;
  
  const cumulativeRoot = Buffer.from(info.data.slice(offset, offset + 32));
  console.log("Cumulative Root:", cumulativeRoot.toString("hex"));
  offset += 32;
  
  const rootSeq = info.data.readBigUInt64LE(offset);
  console.log("Root Seq:", rootSeq.toString());
  offset += 8;
  
  const cutoverEpoch = info.data.readBigInt64LE(offset);
  console.log("Cutover Epoch:", cutoverEpoch.toString());
  offset += 8;
  
  const curator = new PublicKey(info.data.slice(offset, offset + 32));
  console.log("Curator:", curator.toBase58());
  offset += 32;
  
  const curatorFeeBps = info.data.readUInt16LE(offset);
  console.log("Curator Fee BPS:", curatorFeeBps);
}
main().catch(console.error);
