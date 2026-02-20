import { Connection, PublicKey } from "@solana/web3.js";

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");

// Account sizes from Anchor discriminator + struct fields
// ChannelConfigV2: 8 + 1 + 1 + 32 + 32 + 32 + 8 + 8 + 32 + 2 + 6 + (80 * 4) = 482
// ClaimStateV2:    8 + 1 + 1 + 32 + 32 + 8 + 8 = 90
// ChannelStakePool: 8 + 1 + 32 + 32 + 32 + 8 + 8 + 8 + 16 + 8 + 8 + 1 = 162
// UserChannelStake: 8 + 1 + 32 + 32 + 8 + 8 + 8 + 8 + 32 + 16 + 8 = 161
// GlobalRootConfig: 8 + 1 + 1 + 32 + 8 + (80 * 4) = 370

const SIZES = {
  ChannelConfigV2: 482,
  ClaimStateV2: 90,
  ChannelStakePool: 162,
  UserChannelStake: 161,
  GlobalRootConfig: 370,
  ClaimStateGlobal: 90, // 8 + 1 + 1 + 32 + 32 + 8 + 8
};

async function main() {
  const rpcUrl = process.env.RPC_URL;
  if (!rpcUrl) {
    console.error("RPC_URL env var required");
    process.exit(1);
  }
  const conn = new Connection(rpcUrl, "confirmed");

  console.log("======================================================");
  console.log("  AO V2 State Enumeration — Blast Radius");
  console.log("======================================================");
  console.log(`Program: ${PROGRAM_ID.toBase58()}\n`);

  let grandTotalRent = 0;

  for (const [name, size] of Object.entries(SIZES)) {
    const accounts = await conn.getProgramAccounts(PROGRAM_ID, {
      filters: [{ dataSize: size }],
      dataSlice: { offset: 0, length: 0 }, // metadata only, save bandwidth
    });

    let totalRent = 0;
    for (const acc of accounts) {
      totalRent += acc.account.lamports;
    }
    grandTotalRent += totalRent;

    const solAmount = (totalRent / 1e9).toFixed(4);
    console.log(`${name.padEnd(20)} ${String(accounts.length).padStart(5)} accounts  ${solAmount.padStart(10)} SOL rent`);
  }

  console.log("------------------------------------------------------");
  console.log(`${"TOTAL".padEnd(20)} ${" ".padStart(5)}             ${(grandTotalRent / 1e9).toFixed(4).padStart(10)} SOL`);
  console.log("\nNote: ClaimStateV2 and ClaimStateGlobal share the same");
  console.log("      data size (90 bytes) — they are distinguished by");
  console.log("      PDA seeds, not size. Count above is combined.");
}

main().catch(console.error);
