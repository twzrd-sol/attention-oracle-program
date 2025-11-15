/**
 * Multi-Category CLS Support
 *
 * Maps discovered channels to their categories:
 * - category:crypto → Crypto & Web3
 * - category:music → Music Production
 * - category:science → Science & Technology
 * - category:makers → Makers & Crafting
 *
 * Also supports legacy MILO channels
 */

import fs from 'node:fs';
import path from 'node:path';

export type CategoryId = 'crypto' | 'music' | 'science' | 'makers';

export const CATEGORY_IDS: CategoryId[] = ['crypto', 'music', 'science', 'makers'];

const CATEGORY_NAMES: Record<CategoryId, string> = {
  crypto: 'Crypto & Web3',
  music: 'Music Production',
  science: 'Science & Technology',
  makers: 'Makers & Crafting',
};

const CATEGORY_SPLITS: Record<CategoryId, { viewer_ratio: number; streamer_ratio: number }> = {
  crypto: { viewer_ratio: 0.7, streamer_ratio: 0.3 },
  music: { viewer_ratio: 0.7, streamer_ratio: 0.3 },
  science: { viewer_ratio: 0.7, streamer_ratio: 0.3 },
  makers: { viewer_ratio: 0.7, streamer_ratio: 0.3 },
};

class CategoryManager {
  private categoryChannels: Map<CategoryId, Set<string>> = new Map();
  private channelToCategory: Map<string, CategoryId> = new Map();

  constructor(configDir: string = './config') {
    // Load all category channel lists
    for (const categoryId of CATEGORY_IDS) {
      const filePath = path.join(configDir, `cls-${categoryId}-channels.json`);
      try {
        const raw = fs.readFileSync(filePath, 'utf8');
        const channels = JSON.parse(raw) as string[];
        const channelSet = new Set(channels.map((c) => c.toLowerCase()));

        this.categoryChannels.set(categoryId, channelSet);

        // Build reverse lookup
        for (const channel of channelSet) {
          this.channelToCategory.set(channel, categoryId);
        }

        console.log(`[Categories] Loaded ${channels.length} channels for category:${categoryId}`);
      } catch (err) {
        console.warn(`[Categories] Warning: Failed to load cls-${categoryId}-channels.json`, err);
      }
    }
  }

  /**
   * Get the category for a given channel
   * Returns null if channel is not in any category
   */
  getCategoryForChannel(channel: string): CategoryId | null {
    const lower = channel.toLowerCase();
    return this.channelToCategory.get(lower) ?? null;
  }

  /**
   * Get all channels in a category
   */
  getChannelsInCategory(categoryId: CategoryId): string[] {
    return Array.from(this.categoryChannels.get(categoryId) ?? []);
  }

  /**
   * Check if a channel belongs to a category
   */
  isChannelInCategory(channel: string, categoryId: CategoryId): boolean {
    const lower = channel.toLowerCase();
    return this.categoryChannels.get(categoryId)?.has(lower) ?? false;
  }

  /**
   * Get all active category IDs (with channels loaded)
   */
  getActiveCategories(): CategoryId[] {
    return CATEGORY_IDS.filter((id) => (this.categoryChannels.get(id)?.size ?? 0) > 0);
  }

  /**
   * Get category display name
   */
  getCategoryName(categoryId: CategoryId): string {
    return CATEGORY_NAMES[categoryId];
  }

  /**
   * Get category payout splits
   */
  getCategorySplits(categoryId: CategoryId): { viewer_ratio: number; streamer_ratio: number } {
    return CATEGORY_SPLITS[categoryId];
  }

  /**
   * Get total channel count across all categories
   */
  getTotalChannelCount(): number {
    let total = 0;
    for (const channelSet of this.categoryChannels.values()) {
      total += channelSet.size;
    }
    return total;
  }
}

// Singleton instance
let manager: CategoryManager | null = null;

/**
 * Initialize the category manager (call once at startup)
 */
export function initializeCategoryManager(configDir: string = './config'): CategoryManager {
  manager = new CategoryManager(configDir);
  return manager;
}

/**
 * Get the category manager instance
 */
export function getCategoryManager(): CategoryManager {
  if (!manager) {
    manager = new CategoryManager();
  }
  return manager;
}

/**
 * Resolve a channel to its category-prefixed form for internal use
 * E.g., "lofigirl" → "category:music"
 */
export function resolveChannelCategory(channel: string): string {
  const categoryId = getCategoryManager().getCategoryForChannel(channel);
  if (!categoryId) {
    return channel; // Not in any category, return as-is
  }
  return `category:${categoryId}`;
}
