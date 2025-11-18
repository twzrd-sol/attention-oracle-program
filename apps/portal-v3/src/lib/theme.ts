/**
 * TWZRD Portal Design System
 * Centralized theme tokens for colors, spacing, typography, shadows
 * Extracted from App.tsx, index.css, and component styles
 *
 * Usage: import { COLORS, SPACING, TYPOGRAPHY } from './theme'
 */

// ============================================================================
// COLORS
// ============================================================================

export const COLORS = {
  // Primary Brand
  primary: '#3b82f6',           // Tailwind blue-500
  primaryDark: '#1d4ed8',       // Tailwind blue-700
  primaryLight: '#dbeafe',      // Tailwind blue-100

  // Semantic Colors
  success: '#22c55e',           // Tailwind green-500
  successLight: '#f0fdf4',      // Tailwind green-50
  error: '#fca5a5',             // Tailwind red-300
  errorLight: '#fee2e2',        // Tailwind red-50
  warning: '#fcd34d',           // Tailwind amber-300
  warningDark: '#92400e',       // Tailwind amber-900
  warningLight: '#fef3c7',      // Tailwind amber-100

  // Grayscale
  gray50: '#f9fafb',            // Page background
  gray100: '#f3f4f6',           // Subtle background
  gray200: '#e5e7eb',           // Borders, dividers
  gray300: '#d1d5db',           // Disabled state
  gray400: '#9ca3af',           // Secondary text
  gray600: '#6b7280',           // Primary text (muted)
  gray800: '#1f2937',           // Primary text (bold)

  // Utility
  white: '#ffffff',
  black: '#000000',
  transparent: 'transparent',
};

// ============================================================================
// SPACING SCALE
// ============================================================================

export const SPACING = {
  xs: '0.25rem',   // 4px
  sm: '0.5rem',    // 8px
  md: '0.75rem',   // 12px
  lg: '1rem',      // 16px
  xl: '1.5rem',    // 24px
  '2xl': '2rem',   // 32px
  '3xl': '3rem',   // 48px
  '4xl': '4rem',   // 64px
};

// ============================================================================
// TYPOGRAPHY
// ============================================================================

export const TYPOGRAPHY = {
  h1: {
    fontSize: '2rem',
    fontWeight: 800,
    lineHeight: 1.2,
    letterSpacing: '-0.5px',
  } as const,

  h2: {
    fontSize: '1.5rem',
    fontWeight: 700,
    lineHeight: 1.3,
  } as const,

  h3: {
    fontSize: '1.25rem',
    fontWeight: 600,
    lineHeight: 1.4,
  } as const,

  body: {
    fontSize: '1rem',
    fontWeight: 400,
    lineHeight: 1.6,
  } as const,

  small: {
    fontSize: '0.875rem',
    fontWeight: 400,
    lineHeight: 1.5,
  } as const,

  xs: {
    fontSize: '0.75rem',
    fontWeight: 500,
    lineHeight: 1.4,
  } as const,

  label: {
    fontSize: '0.9rem',
    fontWeight: 500,
    lineHeight: 1.5,
  } as const,
};

export const FONT_FAMILY = "-apple-system, BlinkMacSystemFont, 'Segoe UI', 'Roboto', 'Oxygen', 'Ubuntu', 'Cantarell', 'Fira Sans', 'Droid Sans', 'Helvetica Neue', sans-serif";

// ============================================================================
// SHADOWS
// ============================================================================

export const SHADOWS = {
  none: 'none',
  sm: '0 1px 3px rgba(0, 0, 0, 0.1)',
  md: '0 4px 12px rgba(0, 0, 0, 0.15)',
  lg: '0 8px 24px rgba(0, 0, 0, 0.2)',
  xl: '0 12px 32px rgba(0, 0, 0, 0.25)',
};

// ============================================================================
// BORDERS & RADIUS
// ============================================================================

export const BORDERS = {
  radius: '6px',
  radiusLg: '8px',
  radiusXl: '12px',
  radiusPill: '9999px',
  width: '1px',
  widthMd: '2px',
};

// ============================================================================
// TRANSITIONS & ANIMATIONS
// ============================================================================

export const TRANSITIONS = {
  fast: '0.15s ease',
  normal: '0.3s ease',
  slow: '0.5s ease',
  slowest: '0.8s ease',
};

export const ANIMATIONS = {
  spin: '0.6s linear infinite',
  fadeIn: '0.3s ease',
  slideIn: '0.3s ease',
};

// ============================================================================
// TIER SYSTEM (Passport)
// ============================================================================

export const TIER_COLORS = {
  0: {
    bg: COLORS.gray100,
    text: COLORS.gray600,
    border: COLORS.gray300,
    label: 'Unverified',
    emoji: '⚪',
  },
  1: {
    bg: '#dbeafe',          // blue-100
    text: '#1e40af',        // blue-800
    border: '#93c5fd',      // blue-300
    label: 'Emerging',
    emoji: '🔵',
  },
  2: {
    bg: '#dcfce7',          // green-100
    text: '#15803d',        // green-700
    border: '#86efac',      // green-300
    label: 'Active',
    emoji: '🟢',
  },
  3: {
    bg: '#fef3c7',          // amber-100
    text: '#92400e',        // amber-900
    border: '#fcd34d',      // amber-300
    label: 'Established',
    emoji: '🟡',
  },
  4: {
    bg: '#e9d5ff',          // purple-100
    text: '#6b21a8',        // purple-800
    border: '#d8b4fe',      // purple-300
    label: 'Featured',
    emoji: '🟣',
  },
  5: {
    bg: '#fef08a',          // yellow-100
    text: '#854d0e',        // yellow-900
    border: '#fde047',      // yellow-300
    label: 'Elite',
    emoji: '⭐',
  },
};

export const TIER_MULTIPLIERS = {
  0: 0.0,
  1: 0.2,
  2: 0.4,
  3: 0.6,
  4: 0.8,
  5: 1.0,
} as const;

// ============================================================================
// STATUS BADGES
// ============================================================================

export const STATUS_COLORS = {
  confirmed: {
    bg: '#d1fae5',          // green-100
    text: '#065f46',        // green-900
    border: '#6ee7b7',      // green-400
  },
  pending: {
    bg: '#fef3c7',          // amber-100
    text: '#92400e',        // amber-900
    border: '#fcd34d',      // amber-300
  },
  failed: {
    bg: '#fee2e2',          // red-100
    text: '#991b1b',        // red-900
    border: '#fca5a5',      // red-300
  },
  info: {
    bg: '#dbeafe',          // blue-100
    text: '#1e40af',        // blue-800
    border: '#93c5fd',      // blue-300
  },
};

// ============================================================================
// RESPONSIVE BREAKPOINTS
// ============================================================================

export const BREAKPOINTS = {
  xs: '0px',
  sm: '640px',
  md: '768px',
  lg: '1024px',
  xl: '1280px',
  '2xl': '1536px',
};

// ============================================================================
// Z-INDEX LAYERS
// ============================================================================

export const Z_INDEX = {
  hide: -1,
  base: 0,
  dropdown: 1000,
  sticky: 1020,
  fixed: 1030,
  modalBackdrop: 1040,
  modal: 1050,
  popover: 1060,
  tooltip: 1070,
};

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/**
 * Get tier color by tier number (0-5+)
 */
export function getTierColor(tier: number) {
  const tierNum = Math.min(tier, 5) as keyof typeof TIER_COLORS;
  return TIER_COLORS[tierNum];
}

/**
 * Get tier multiplier by tier number
 */
export function getTierMultiplier(tier: number): number {
  const tierNum = Math.min(tier, 5) as keyof typeof TIER_MULTIPLIERS;
  return TIER_MULTIPLIERS[tierNum];
}

/**
 * Get status color by status string
 */
export function getStatusColor(status: 'confirmed' | 'pending' | 'failed' | 'info') {
  return STATUS_COLORS[status];
}

/**
 * Calculate fee amount given base amount and multiplier
 */
export function calculateFee(amount: number, multiplier: number, basisPoints: number = 10): number {
  return (amount * basisPoints * multiplier) / 10000;
}

/**
 * Calculate net amount after fee
 */
export function calculateNet(amount: number, multiplier: number, basisPoints: number = 10): number {
  return amount - calculateFee(amount, multiplier, basisPoints);
}
