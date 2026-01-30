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
  // Abstract vaults (6h lock)
  // -------------------------------------------------------------------------
  {
    name: "twzrd-247-6h",
    label: "vLOFI TWZRD 247",
    channelConfig: "84SxXryEL2dFT5rno9F1SGBAFvvkEDyp3wNQZyxT3hQ9",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "twzrd-1999-6h",
    label: "vLOFI TWZRD 1999",
    channelConfig: "7g1qkWgZkbhZNFgbEzxxvYxCJHt4NMb3fwE2RHyrygDL",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "twzrd-415-6h",
    label: "vLOFI TWZRD 415",
    channelConfig: "DqoM3QcGPbUD2Hic1fxsSLqZY1CaSDkiaNaas2ufZUpb",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "twzrd-3121-6h",
    label: "vLOFI TWZRD 3121",
    channelConfig: "EADvLuoe6ZXTfVBpVEKAMSfnFr1oZuHMxiButLVMnHuE",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "twzrd-69-6h",
    label: "vLOFI TWZRD 69",
    channelConfig: "HEa4KgAyuvRZPyAsUPmVTRXiTRuxVEkkGbmtEeybzGB9",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },

  // -------------------------------------------------------------------------
  // Playlist-bound vaults (6h lock)
  // -------------------------------------------------------------------------
  {
    name: "spotify-999",
    label: "vLOFI 999",
    channelConfig: "9G1MvnVq3dX6UwGFvhTC9bDArNt9TyvS5UimffTL1BAJ",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "spotify-212",
    label: "vLOFI 212",
    channelConfig: "Dg84d5BkSYxKSix9m6YgbLz1L7mEsSH81Svp24watxEC",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "spotify-247",
    label: "vLOFI 247",
    channelConfig: "GdrV9DjKZFePZadxuQANKEBvVaB7rM8aEhMEzMHWrFJE",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "spotify-1999",
    label: "vLOFI 1999",
    channelConfig: "8LCSiL2a4FjTAveMMn8SjLVxrYecWSfFDH48sdhzdbv",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "spotify-415",
    label: "vLOFI 415",
    channelConfig: "GxzK9iqyFJf3TRJG5XAQJD3eJtgKCivzkQtj7iPKrUsG",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "spotify-3121",
    label: "vLOFI 3121",
    channelConfig: "4JawzmsofxVCim7eDtFPCMwiP21NMcAQqsZRPT7k9uL1",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },
  {
    name: "spotify-69",
    label: "vLOFI 69",
    channelConfig: "2uGQDJMsGy3undJCT9NazdJXjSoCcXd71vgkvYzMt3eR",
    lockDurationSlots: 54_000,
    withdrawQueueSlots: 9_000,
  },
];
