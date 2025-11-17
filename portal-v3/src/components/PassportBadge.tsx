import React from 'react';
import { getTierColor, getTierMultiplier, SPACING, TYPOGRAPHY, BORDERS, TRANSITIONS } from '../lib/theme';

interface PassportBadgeProps {
  tier: number;
  score?: number;
  nextTierScore?: number;
}

/**
 * PassportBadge Component
 *
 * Displays user's passport tier, multiplier, and progress to next tier.
 * Color-coded by tier (0-5+).
 *
 * Example:
 * <PassportBadge tier={3} score={8450} nextTierScore={10000} />
 */
export const PassportBadge: React.FC<PassportBadgeProps> = ({
  tier,
  score = 0,
  nextTierScore = 10000,
}) => {
  const tierColor = getTierColor(tier);
  const multiplier = getTierMultiplier(Math.min(tier, 5));
  const progress = score > 0 && nextTierScore > 0 ? Math.min((score / nextTierScore) * 100, 100) : 0;

  const styles = {
    container: {
      backgroundColor: tierColor.bg,
      border: `${BORDERS.widthMd} solid ${tierColor.border}`,
      borderRadius: BORDERS.radiusLg,
      padding: SPACING.lg,
      marginBottom: SPACING.xl,
      color: tierColor.text,
    } as React.CSSProperties,

    header: {
      display: 'flex',
      alignItems: 'center',
      gap: SPACING.md,
      marginBottom: SPACING.md,
      ...TYPOGRAPHY.h3,
    } as React.CSSProperties,

    emoji: {
      fontSize: '1.5rem',
      lineHeight: 1,
    } as React.CSSProperties,

    titleText: {
      flex: 1,
    } as React.CSSProperties,

    multiplierBadge: {
      backgroundColor: tierColor.text,
      color: tierColor.bg,
      padding: `${SPACING.sm} ${SPACING.md}`,
      borderRadius: BORDERS.radius,
      ...TYPOGRAPHY.label,
      fontWeight: 600,
      whiteSpace: 'nowrap' as const,
    } as React.CSSProperties,

    progressContainer: {
      marginTop: SPACING.lg,
    } as React.CSSProperties,

    progressLabel: {
      fontSize: '0.85rem',
      marginBottom: SPACING.sm,
      display: 'flex',
      justifyContent: 'space-between',
      fontWeight: 500,
    } as React.CSSProperties,

    progressBar: {
      width: '100%',
      height: '8px',
      backgroundColor: 'rgba(0,0,0,0.1)',
      borderRadius: BORDERS.radius,
      overflow: 'hidden',
    } as React.CSSProperties,

    progressFill: {
      height: '100%',
      backgroundColor: tierColor.text,
      width: `${progress}%`,
      transition: `width ${TRANSITIONS.normal}`,
    } as React.CSSProperties,

    nextTierText: {
      marginTop: SPACING.sm,
      fontSize: '0.85rem',
      fontWeight: 500,
    } as React.CSSProperties,
  };

  return (
    <div style={styles.container}>
      <div style={styles.header}>
        <span style={styles.emoji}>{tierColor.emoji}</span>
        <span style={styles.titleText}>
          Tier {tier}: {tierColor.label}
        </span>
        <span style={styles.multiplierBadge}>{multiplier.toFixed(1)}x fee</span>
      </div>

      {score !== null && nextTierScore && nextTierScore > 0 && (
        <div style={styles.progressContainer}>
          <div style={styles.progressLabel}>
            <span>Engagement Score</span>
            <span>{score.toLocaleString()} / {nextTierScore.toLocaleString()}</span>
          </div>
          <div style={styles.progressBar}>
            <div style={styles.progressFill} />
          </div>
          {tier < 5 && (
            <div style={styles.nextTierText}>
              {Math.max(0, nextTierScore - score).toLocaleString()} points to Tier {tier + 1}
            </div>
          )}
          {tier >= 5 && (
            <div style={styles.nextTierText}>
              🎉 You've reached Elite tier! No further progression.
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export default PassportBadge;
