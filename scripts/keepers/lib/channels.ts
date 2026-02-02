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
  // Lofi vaults (tiered lock durations)
  // -------------------------------------------------------------------------
  {
    name: "lofi-vault-3h",
    label: "vLOFI Lofi 3h",
    channelConfig: "J3HAT4NbL6REyyNqbW1BDGF9BXXc3FYuQ1fr6NbCQaoW",
    lockDurationSlots: 27_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "lofi-vault-6h",
    label: "vLOFI Lofi 6h",
    channelConfig: "dJvatt5bga4ak64ghTLEtxs1jxfLX4TNoZuvfiDCcGy",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "lofi-vault-9h",
    label: "vLOFI Lofi 9h",
    channelConfig: "2TWM1H1gHWrA6Ta6A9tH3E1TTTRbPpmSL2Xg7KdHwxCM",
    lockDurationSlots: 81_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "lofi-vault-12h",
    label: "vLOFI Lofi 12h",
    channelConfig: "GZL7vAo9vxdNbsmrreVueVd1Xm9oWmatkQauFcxhq8qP",
    lockDurationSlots: 108_000,
    withdrawQueueSlots: 9_000,
  },

  // -------------------------------------------------------------------------
  // TWZRD tribute vault (active)
  // -------------------------------------------------------------------------
  {
    name: "twzrd-247-6h",
    label: "vLOFI TWZRD 247",
    channelConfig: "84SxXryEL2dFT5rno9F1SGBAFvvkEDyp3wNQZyxT3hQ9",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },
  // NOTE: twzrd-1999-6h, twzrd-415-6h, twzrd-3121-6h, twzrd-69-6h
  // shut down (proposal #XX) — 0 stakers, consolidating lock tiers

  // -------------------------------------------------------------------------
  // Audio:listening vaults (6h lock)
  // -------------------------------------------------------------------------
  {
    name: "audio-999",
    label: "vLOFI 999",
    channelConfig: "9G1MvnVq3dX6UwGFvhTC9bDArNt9TyvS5UimffTL1BAJ",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "audio-212",
    label: "vLOFI 212",
    channelConfig: "Dg84d5BkSYxKSix9m6YgbLz1L7mEsSH81Svp24watxEC",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "audio-247",
    label: "vLOFI 247",
    channelConfig: "GdrV9DjKZFePZadxuQANKEBvVaB7rM8aEhMEzMHWrFJE",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "audio-1999",
    label: "vLOFI 1999",
    channelConfig: "8LCSiL2a4FjTAveMMn8SjLVxrYecWSfFDH48sdhzdbv",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "audio-415",
    label: "vLOFI 415",
    channelConfig: "GxzK9iqyFJf3TRJG5XAQJD3eJtgKCivzkQtj7iPKrUsG",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "audio-3121",
    label: "vLOFI 3121",
    channelConfig: "4JawzmsofxVCim7eDtFPCMwiP21NMcAQqsZRPT7k9uL1",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "audio-69",
    label: "vLOFI 69",
    channelConfig: "2uGQDJMsGy3undJCT9NazdJXjSoCcXd71vgkvYzMt3eR",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },

  // -------------------------------------------------------------------------
  // Audio lock-tier variants: 3h (Quick) + 12h (Extended)
  // Existing 7.5h pools above are the "Standard" tier.
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
