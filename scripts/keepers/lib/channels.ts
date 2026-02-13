/**
 * Canonical channel registry for all TWZRD vaults.
 *
 * Single source of truth shared by deploy scripts and keepers.
 * Channel configs are on-chain Oracle ChannelConfigV2 accounts;
 * the vault PDA is derived from the channel config pubkey.
 */

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
}

export const CHANNELS: ChannelEntry[] = [
  // -------------------------------------------------------------------------
  // Audio lock-tier vaults: 3h (Quick) + 12h (Extended)
  // 7 playlists x 2 tiers = 14 active pools
  // Created by: scripts/admin/deploy-lock-variants.ts
  // -------------------------------------------------------------------------

  // 999
  {
    name: "audio-999-3h",
    label: "vLOFI 999 3h",
    channelConfig: "ABe3RusmP8jpe1EUt1fhK5Xiv5fUUB4z7PsVgAQh7LXy",
    lockDurationSlots: 27_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "audio-999-12h",
    label: "vLOFI 999 12h",
    channelConfig: "Cs5E2paQs9JMcaimQVeqA4DmcPXzggtH5eKdTi1zvyRw",
    lockDurationSlots: 108_000,
    withdrawQueueSlots: 9_000,
  },

  // 212
  {
    name: "audio-212-3h",
    label: "vLOFI 212 3h",
    channelConfig: "6CXFKAyeai83FqmwxSHtHrJMPWFyDw4LCEezVz6hCf8Q",
    lockDurationSlots: 27_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "audio-212-12h",
    label: "vLOFI 212 12h",
    channelConfig: "4WoGnVL3urQvuTNxHN1eCTJke9BG2LMP2eVFxsutXAS3",
    lockDurationSlots: 108_000,
    withdrawQueueSlots: 9_000,
  },

  // 247
  {
    name: "audio-247-3h",
    label: "vLOFI 247 3h",
    channelConfig: "STM8hwhyVQmSBPiveWitT6M8boh49Waqq9hx5yH3fYq",
    lockDurationSlots: 27_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "audio-247-12h",
    label: "vLOFI 247 12h",
    channelConfig: "KbpDdfbhq5rYD9oue2srpgEfDb9BWQ4dCKWMzMwep4X",
    lockDurationSlots: 108_000,
    withdrawQueueSlots: 9_000,
  },

  // 1999
  {
    name: "audio-1999-3h",
    label: "vLOFI 1999 3h",
    channelConfig: "CFg6W66Pth1CLxCNA7BtzrsAzckYQZXFfkQovuKGqLN5",
    lockDurationSlots: 27_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "audio-1999-12h",
    label: "vLOFI 1999 12h",
    channelConfig: "HtSj5LXEkbySCA2r1oR2BgKCPt1rJ9tWhCVqstuL1BL8",
    lockDurationSlots: 108_000,
    withdrawQueueSlots: 9_000,
  },

  // 415
  {
    name: "audio-415-3h",
    label: "vLOFI 415 3h",
    channelConfig: "2aX84WPoRjXwseBozHkeKcVqRvy3zi7QqVymUwTNej2Z",
    lockDurationSlots: 27_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "audio-415-12h",
    label: "vLOFI 415 12h",
    channelConfig: "7SbVNdmMFbdEKJNPygXRM6rPk6Vvt1FC5pieHMXcUFzf",
    lockDurationSlots: 108_000,
    withdrawQueueSlots: 9_000,
  },

  // 3121
  {
    name: "audio-3121-3h",
    label: "vLOFI 3121 3h",
    channelConfig: "4CTqFwo27h8CGJUFqoJfRtLC9aQ5zUgj261QAwidfX4g",
    lockDurationSlots: 27_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "audio-3121-12h",
    label: "vLOFI 3121 12h",
    channelConfig: "AdXbdEPAL9cUTaMK8ykmcSTe93Wv4sfQ6Uh5xPAAsCfW",
    lockDurationSlots: 108_000,
    withdrawQueueSlots: 9_000,
  },

  // 69
  {
    name: "audio-69-3h",
    label: "vLOFI 69 3h",
    channelConfig: "9WhHZXitc8rCfiZexqB7FtNKnDNLGXFaf7SbnYkvu8S4",
    lockDurationSlots: 27_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "audio-69-12h",
    label: "vLOFI 69 12h",
    channelConfig: "3K2wfUmg6hoJVWo8vygawpScFJdGumJ9KJJtErodFan4",
    lockDurationSlots: 108_000,
    withdrawQueueSlots: 9_000,
  },
];

/**
 * Derive the on-chain Oracle "channel" string used in ChannelConfigV2 PDA seeds.
 *
 * Important: `ChannelEntry.name` is a dash-safe identifier (env var / log friendly),
 * but the Oracle program derives PDAs from the original channel string (e.g. "audio:999:3h").
 *
 * If this mapping breaks, cutover epoch admin scripts will fail their PDA seed checks.
 */
export function oracleChannelName(entry: ChannelEntry): string {
  const m = entry.name.match(/^audio-(\d+)-(3h|12h)$/);
  if (m) return `audio:${m[1]}:${m[2]}`;
  throw new Error(
    `Unsupported channel name format: ${entry.name} (expected audio-<id>-<tier>)`,
  );
}
