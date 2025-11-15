#!/usr/bin/env tsx
/**
 * Sanity Check for Multi-Category CLS Channels
 *
 * Checks discovered channels for:
 * - Bot-like username patterns
 * - Suspicious viewer/uptime ratios
 * - Known bad actors
 * - Category appropriateness
 *
 * Outputs: go/no-go report
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const MANIFEST_FILE = path.resolve(__dirname, '../../config/cls-all-channels.json');

type ChannelInfo = {
  username: string;
  viewer_count: number;
  uptime_minutes: number;
};

type CategoryData = {
  count: number;
  channels: ChannelInfo[];
};

type Manifest = {
  discovered_at: number;
  total_channels: number;
  categories: Record<string, CategoryData>;
};

// Suspicious patterns
const BOT_PATTERNS = [
  /^user\d{3,}$/i,          // user123, user4567
  /^test\d+$/i,             // test1, test999
  /^bot\d+$/i,              // bot1, bot42
  /^[a-z]{8}$/,             // random8chars
  /\d{6,}/,                 // 6+ consecutive digits
];

const SUSPICIOUS_KEYWORDS = [
  'fake', 'scam', 'bot', 'spam', 'hack', 'exploit',
  'n3on', 'threadguy', 'thread_guy', 'counterparty'
];

function checkBotPattern(username: string): boolean {
  return BOT_PATTERNS.some(pattern => pattern.test(username));
}

function checkSuspiciousKeywords(username: string): boolean {
  const lower = username.toLowerCase();
  return SUSPICIOUS_KEYWORDS.some(keyword => lower.includes(keyword));
}

function checkViewerUptimeRatio(channel: ChannelInfo): string | null {
  // Suspicious if very low viewers for very long uptime
  // (could indicate view botting that stopped)
  const hoursLive = channel.uptime_minutes / 60;

  if (hoursLive > 24 && channel.viewer_count < 15) {
    return `Low viewers (${channel.viewer_count}) for long stream (${hoursLive.toFixed(1)}h)`;
  }

  if (hoursLive > 40) {
    return `Extremely long stream (${hoursLive.toFixed(1)}h) - possible 24/7 restream`;
  }

  return null;
}

function analyzeCategory(categoryId: string, data: CategoryData): {
  clean: ChannelInfo[];
  flagged: Array<{ channel: ChannelInfo; reason: string }>;
} {
  const clean: ChannelInfo[] = [];
  const flagged: Array<{ channel: ChannelInfo; reason: string }> = [];

  for (const channel of data.channels) {
    const reasons: string[] = [];

    if (checkBotPattern(channel.username)) {
      reasons.push('Bot-like username pattern');
    }

    if (checkSuspiciousKeywords(channel.username)) {
      reasons.push('Contains suspicious keyword');
    }

    const ratioIssue = checkViewerUptimeRatio(channel);
    if (ratioIssue) {
      reasons.push(ratioIssue);
    }

    if (reasons.length > 0) {
      flagged.push({ channel, reason: reasons.join('; ') });
    } else {
      clean.push(channel);
    }
  }

  return { clean, flagged };
}

function generateReport(manifest: Manifest): void {
  console.log('🔍 Multi-Category CLS Channel Sanity Check\n');
  console.log(`Discovered at: ${new Date(manifest.discovered_at).toISOString()}`);
  console.log(`Total channels: ${manifest.total_channels}\n`);

  let totalClean = 0;
  let totalFlagged = 0;

  for (const [categoryId, data] of Object.entries(manifest.categories)) {
    const { clean, flagged } = analyzeCategory(categoryId, data);

    totalClean += clean.length;
    totalFlagged += flagged.length;

    console.log(`\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`);
    console.log(`📁 Category: ${categoryId.toUpperCase()}`);
    console.log(`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`);
    console.log(`Total: ${data.count} | Clean: ${clean.length} | Flagged: ${flagged.length}`);

    if (flagged.length > 0) {
      console.log(`\n⚠️  FLAGGED CHANNELS:`);
      flagged.forEach(({ channel, reason }) => {
        console.log(`  • ${channel.username.padEnd(25)} → ${reason}`);
      });
    }

    if (clean.length > 0) {
      console.log(`\n✅ CLEAN CHANNELS (sample):`);
      clean.slice(0, 5).forEach(ch => {
        console.log(`  • ${ch.username.padEnd(25)} ${ch.viewer_count}v | ${Math.floor(ch.uptime_minutes / 60)}h ${ch.uptime_minutes % 60}m`);
      });
      if (clean.length > 5) {
        console.log(`  ... and ${clean.length - 5} more`);
      }
    }
  }

  console.log(`\n\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`);
  console.log(`📊 FINAL REPORT`);
  console.log(`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`);
  console.log(`Total Clean: ${totalClean} (${((totalClean / manifest.total_channels) * 100).toFixed(1)}%)`);
  console.log(`Total Flagged: ${totalFlagged} (${((totalFlagged / manifest.total_channels) * 100).toFixed(1)}%)`);

  // Go/No-Go decision
  const flagRate = totalFlagged / manifest.total_channels;
  console.log(`\n🎯 RECOMMENDATION:\n`);

  if (flagRate < 0.1) {
    console.log(`✅ GO - Flag rate ${(flagRate * 100).toFixed(1)}% is acceptable (<10%)`);
    console.log(`   Safe to proceed with multi-category CLS launch.`);
  } else if (flagRate < 0.25) {
    console.log(`⚠️  PROCEED WITH CAUTION - Flag rate ${(flagRate * 100).toFixed(1)}% is elevated (10-25%)`);
    console.log(`   Review flagged channels manually before launch.`);
  } else {
    console.log(`❌ NO-GO - Flag rate ${(flagRate * 100).toFixed(1)}% is too high (>25%)`);
    console.log(`   Adjust discovery filters and re-run discovery.`);
  }

  console.log(`\n📋 NEXT STEPS:`);
  if (flagRate < 0.1) {
    console.log(`  1. Review flagged channels manually (optional)`);
    console.log(`  2. Add any confirmed bad actors to category blocklists`);
    console.log(`  3. Update aggregator to handle category:* channels`);
    console.log(`  4. Deploy multi-category CLS`);
  } else {
    console.log(`  1. Review ALL flagged channels manually`);
    console.log(`  2. Add confirmed bad actors to blocklists`);
    console.log(`  3. Re-run discovery with adjusted filters`);
    console.log(`  4. Run sanity check again before launch`);
  }

  console.log(`\n`);
}

function main(): void {
  try {
    const raw = fs.readFileSync(MANIFEST_FILE, 'utf8');
    const manifest = JSON.parse(raw) as Manifest;
    generateReport(manifest);
  } catch (err: any) {
    console.error(`Failed to read manifest from ${MANIFEST_FILE}:`, err?.message ?? err);
    process.exit(1);
  }
}

main();
