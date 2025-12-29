#!/usr/bin/env ts-node
/**
 * audit-channels.ts
 *
 * Audits all ChannelState (V1) and ChannelConfigV2 (V2) accounts.
 * Tries to identify them against a known list of channel names.
 *
 * Usage:
 *   ts-node scripts/audit-channels.ts
 */

import { Connection, PublicKey } from "@solana/web3.js";
import pkg from "js-sha3";
const { keccak256 } = pkg;
import { config } from "dotenv";

// Load env if present
config({ path: ".env.ccm-v3" });
config();

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");

// Known candidates
const CANDIDATE_CHANNELS = [
  "youtube_lofi",
  "spotify_lofi",
  "twitch_lofi",
  "apple_lofi",
  "soundcloud_lofi",
  "test_channel",
  "dev_channel",
];

// Discriminators
const CHANNEL_STATE_DISC = Buffer.from([0xa3, 0x3b, 0x8a, 0x22, 0x01, 0x84, 0x24, 0x1f]); // derived approx
import { createHash } from "crypto";
function getDiscriminator(name: string): Buffer {
  return createHash("sha256").update(`account:${name}`).digest().slice(0, 8);
}
const DISC_CHANNEL_STATE = getDiscriminator("ChannelState");
const DISC_CHANNEL_CONFIG_V2 = getDiscriminator("ChannelConfigV2");

function deriveSubjectId(channel: string): string {
  const lower = channel.toLowerCase();
  const input = Buffer.concat([Buffer.from("channel:"), Buffer.from(lower)]);
  return new PublicKey(Buffer.from(keccak256(input), "hex")).toBase58();
}

async function main() {
  const rpcUrl = process.env.RPC_URL || process.env.SOLANA_RPC || process.env.ANCHOR_PROVIDER_URL;
  if (!rpcUrl) {
    console.error("❌ Missing RPC_URL. Please set RPC_URL in .env or environment.");
    process.exit(1);
  }

  const connection = new Connection(rpcUrl, "confirmed");

  console.log("=== Channel Audit ===\n");
  console.log(`Program: ${PROGRAM_ID.toString()}`);
  console.log(`RPC: ${rpcUrl.slice(0, 15)}...\n`);

  // ... rest of the script ...

  const subjectMap = new Map<string, string>();
  for (const name of CANDIDATE_CHANNELS) {
    const subject = deriveSubjectId(name);
    subjectMap.set(subject, name);
    // console.log(`Candidate: ${name} -> ${subject}`);
  }

  console.log("Fetching accounts...");
  const accounts = await connection.getProgramAccounts(PROGRAM_ID);
  console.log(`Total accounts: ${accounts.length}\n`);

  const v1Accounts: any[] = [];
  const v2Accounts: any[] = [];
  const unknownAccounts: any[] = [];

  for (const { pubkey, account } of accounts) {
    const data = account.data;
    if (data.length < 8) continue;
    const disc = data.slice(0, 8);

    if (disc.equals(DISC_CHANNEL_STATE)) {
      // V1 ChannelState
      // Layout: disc(8) + version(1) + bump(1) + mint(32) + subject(32)
      const subject = new PublicKey(data.slice(42, 74)).toBase58();
      const mint = new PublicKey(data.slice(10, 42)).toBase58();
      v1Accounts.push({
        pubkey: pubkey.toString(),
        subject,
        mint,
        size: data.length,
        lamports: account.lamports,
        name: subjectMap.get(subject) || "UNKNOWN",
      });
    } else if (disc.equals(DISC_CHANNEL_CONFIG_V2)) {
      // V2 ChannelConfigV2
      // Layout: disc(8) + version(1) + bump(1) + mint(32) + subject(32)
      const subject = new PublicKey(data.slice(42, 74)).toBase58();
      const mint = new PublicKey(data.slice(10, 42)).toBase58();
      v2Accounts.push({
        pubkey: pubkey.toString(),
        subject,
        mint,
        size: data.length,
        lamports: account.lamports,
        name: subjectMap.get(subject) || "UNKNOWN",
      });
    } else {
      // Filter out known other types (ProtocolState, etc) if needed, or just list generic
      // We'll skip small accounts likely to be protocol state / config
      if (data.length > 200) { 
        unknownAccounts.push({ pubkey: pubkey.toString(), size: data.length });
      }
    }
  }

  console.log("=== V1 ChannelState (LEGACY / DEPRECATED) ===");
  if (v1Accounts.length === 0) console.log("None found.");
  for (const acc of v1Accounts) {
    console.log(`[${acc.name}]`);
    console.log(`  PDA: ${acc.pubkey}`);
    console.log(`  Subject: ${acc.subject}`);
    console.log(`  Mint: ${acc.mint}`);
    console.log(`  Size: ${acc.size} bytes`);
    console.log(`  Rent: ${(acc.lamports / 1e9).toFixed(4)} SOL`);
    console.log("");
  }

  console.log("=== V2 ChannelConfigV2 (ACTIVE) ===");
  if (v2Accounts.length === 0) console.log("None found.");
  for (const acc of v2Accounts) {
    console.log(`[${acc.name}]`);
    console.log(`  PDA: ${acc.pubkey}`);
    console.log(`  Subject: ${acc.subject}`);
    console.log(`  Rent: ${(acc.lamports / 1e9).toFixed(4)} SOL`);
    console.log("");
  }

  const totalRentV1 = v1Accounts.reduce((sum, a) => sum + a.lamports, 0);
  console.log(`Total Reclaimable Rent (V1): ${(totalRentV1 / 1e9).toFixed(4)} SOL`);
}

main().catch(console.error);
