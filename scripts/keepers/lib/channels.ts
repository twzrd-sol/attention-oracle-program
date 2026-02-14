/**
 * Canonical channel registry for tracked TWZRD vaults.
 *
 * Single source of truth shared by deploy scripts and keepers.
 * Channel configs are on-chain Oracle ChannelConfigV2 accounts;
 * the vault PDA is derived from the channel config pubkey.
 *
 * IMPORTANT: This module is intentionally env-driven so the public repo does
 * not hardcode product channel naming or the live vault set.
 *
 * Provide exactly the channels you intend to operate via:
 * - `TWZRD_CHANNELS_JSON` (preferred; JSON array)
 * - `TWZRD_CHANNELS_PATH` (path to JSON file; useful for local runs)
 */

import fs from "fs";
import { PublicKey } from "@solana/web3.js";

export interface ChannelEntry {
  /** Short name (used in logs and env var prefixes) */
  name: string;
  /** Human-readable label for vLOFI metadata */
  label: string;
  /** Base58 pubkey of the ChannelConfigV2 account */
  channelConfig: string;
  /** Lock duration in slots for the vault's Oracle stake */
  lockDurationSlots: number;
  /** Withdrawal queue duration in slots */
  withdrawQueueSlots: number;
  /** On-chain Oracle channel string used as the `channel` argument (PDA seed). */
  oracleChannel: string;
}

function isAscii(s: string): boolean {
  for (let i = 0; i < s.length; i++) {
    if (s.charCodeAt(i) > 0x7f) return false;
  }
  return true;
}

function requireString(value: unknown, field: string): string {
  if (typeof value !== "string") throw new Error(`${field} must be a string`);
  const trimmed = value.trim();
  if (!trimmed) throw new Error(`${field} must be non-empty`);
  return trimmed;
}

function requireU64ish(value: unknown, field: string): number {
  const num =
    typeof value === "number" ? value :
    typeof value === "string" ? Number(value) :
    NaN;
  if (!Number.isSafeInteger(num) || num < 0) {
    throw new Error(`${field} must be a non-negative integer (got ${JSON.stringify(value)})`);
  }
  return num;
}

function parseChannelEntry(value: unknown, idx: number): ChannelEntry {
  if (!value || typeof value !== "object") {
    throw new Error(`channels[${idx}] must be an object`);
  }

  const obj = value as Record<string, unknown>;

  const name = requireString(obj.name, `channels[${idx}].name`);
  const label = requireString(obj.label, `channels[${idx}].label`);
  const channelConfigRaw = requireString(obj.channelConfig, `channels[${idx}].channelConfig`);

  let channelConfig: string;
  try {
    channelConfig = new PublicKey(channelConfigRaw).toBase58();
  } catch {
    throw new Error(`channels[${idx}].channelConfig must be a valid pubkey`);
  }

  const oracleChannel = requireString(obj.oracleChannel, `channels[${idx}].oracleChannel`);
  // This string is used directly as a Solana PDA seed. Seeds are limited to 32 bytes.
  // We also require ASCII so bytes==chars and configs are deterministic across tooling.
  if (!isAscii(oracleChannel) || oracleChannel.length > 32) {
    throw new Error(`channels[${idx}].oracleChannel must be <=32 ASCII chars (PDA seed limit)`);
  }

  const lockDurationSlots = requireU64ish(obj.lockDurationSlots, `channels[${idx}].lockDurationSlots`);
  const withdrawQueueSlots = requireU64ish(obj.withdrawQueueSlots, `channels[${idx}].withdrawQueueSlots`);

  return {
    name,
    label,
    channelConfig,
    lockDurationSlots,
    withdrawQueueSlots,
    oracleChannel,
  };
}

function loadChannelsConfig(): ChannelEntry[] {
  const rawJson = process.env.TWZRD_CHANNELS_JSON;
  const jsonPath = process.env.TWZRD_CHANNELS_PATH;

  let raw: string | undefined;
  if (rawJson && rawJson.trim()) {
    raw = rawJson;
  } else if (jsonPath && jsonPath.trim()) {
    raw = fs.readFileSync(jsonPath.trim(), "utf-8");
  }

  if (!raw) {
    throw new Error(
      "Missing TWZRD_CHANNELS_JSON (or TWZRD_CHANNELS_PATH).\n" +
      "Example:\n" +
      "  TWZRD_CHANNELS_JSON='[\n" +
      "    {\"name\":\"stream-tv\",\"label\":\"vLOFI Stream TV\",\"channelConfig\":\"<pubkey>\"," +
      "\"lockDurationSlots\":54000,\"withdrawQueueSlots\":9000,\"oracleChannel\":\"stream:tv\"}\n" +
      "  ]'\n"
    );
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (e) {
    throw new Error(`Invalid TWZRD_CHANNELS_JSON: ${(e as Error).message}`);
  }

  if (!Array.isArray(parsed)) {
    throw new Error("TWZRD_CHANNELS_JSON must be a JSON array");
  }

  const channels = parsed.map((v, i) => parseChannelEntry(v, i));

  const seenNames = new Set<string>();
  const seenConfigs = new Set<string>();
  for (const ch of channels) {
    if (seenNames.has(ch.name)) throw new Error(`Duplicate channel name: ${ch.name}`);
    if (seenConfigs.has(ch.channelConfig)) throw new Error(`Duplicate channelConfig: ${ch.channelConfig}`);
    seenNames.add(ch.name);
    seenConfigs.add(ch.channelConfig);
  }

  if (channels.length === 0) {
    throw new Error("TWZRD_CHANNELS_JSON must contain at least 1 channel");
  }

  return channels;
}

export const CHANNELS: ChannelEntry[] = loadChannelsConfig();

/**
 * Derive the on-chain Oracle "channel" string used in ChannelConfigV2 PDA seeds.
 *
 * Important: `ChannelEntry.name` is a dash-safe identifier (env var / log friendly),
 * but the Oracle program derives PDAs from the original channel string (e.g. "stream:tv").
 *
 * If this mapping breaks, cutover epoch admin scripts will fail their PDA seed checks.
 */
export function oracleChannelName(entry: ChannelEntry): string {
  return entry.oracleChannel;
}
