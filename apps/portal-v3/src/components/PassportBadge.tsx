import React from 'react';
import { getTierColor, getTierMultiplier } from '../lib/theme';

interface PassportBadgeProps {
  tier: number;
  score?: number;
  nextTierScore?: number;
}

export const PassportBadge: React.FC<PassportBadgeProps> = ({
  tier,
  score = 0,
  nextTierScore = 10000,
}) => {
  const { bg, text, border, emoji, label } = getTierColor(tier);
  const multiplier = getTierMultiplier(Math.min(tier, 5));
  const progress = score > 0 && nextTierScore > 0 ? Math.min((score / nextTierScore) * 100, 100) : 0;

  return (
    <div className={`rounded-2xl border-2 ${border} ${bg} ${text} p-6 mb-10`}>
      <div className="flex items-center gap-4 mb-4 text-lg font-semibold">
        <span className="text-3xl">{emoji}</span>
        <span className="flex-1">
          Tier {tier}: {label}
        </span>
        <span className={`px-4 py-1.5 rounded-full ${bg} text-white font-bold text-sm`}>
          {multiplier.toFixed(1)}x fee
        </span>
      </div>

      {score != null && nextTierScore > 0 && (
        <div className="mt-6">
          <div className="flex justify-between text-sm font-medium mb-2">
            <span>Engagement Score</span>
            <span>{score.toLocaleString()} / {nextTierScore.toLocaleString()}</span>
          </div>
          <div className="w-full h-2 bg-black/10 rounded-full overflow-hidden">
            <div
              className={`h-full ${text} transition-all duration-500`}
              style={{ width: `${progress}%` }}
            />
          </div>
          <div className="mt-2 text-sm font-medium">
            {tier < 5 ? (
              <>+{Math.max(0, nextTierScore - score).toLocaleString()} points to Tier {tier + 1}</>
            ) : (
              <>You've reached Elite tier! No further progression.</>
            )}
          </div>
        </div>
      )}
    </div>
  );
};

export default PassportBadge;
