import React, { useState, useEffect } from 'react';
import { useWallet } from '@solana/wallet-adapter-react';
import { getClaimHistory, ClaimRecord } from '../lib/api';
import {
  COLORS,
  SPACING,
  TYPOGRAPHY,
  SHADOWS,
  TRANSITIONS,
  BORDERS,
  STATUS_COLORS,
} from '../lib/theme';

/**
 * ClaimHistory Component
 *
 * Display user's claim history with status and transaction links.
 * Only shows if wallet is connected.
 * Pagination built-in (10 per page).
 *
 * Example:
 * <ClaimHistory />  (works with @solana/wallet-adapter-react context)
 */
export const ClaimHistory: React.FC = () => {
  const { publicKey } = useWallet();
  const [claims, setClaims] = useState<ClaimRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(0);
  const [total, setTotal] = useState(0);

  const limit = 10;
  const offset = page * limit;

  useEffect(() => {
    if (!publicKey) {
      setClaims([]);
      setLoading(false);
      return;
    }

    const fetchHistory = async () => {
      try {
        setLoading(true);
        const response = await getClaimHistory(publicKey.toString(), limit, offset);
        setClaims(response.claims);
        setTotal(response.total);
        setError(null);
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Failed to load history';
        setError(message);
        setClaims([]);
      } finally {
        setLoading(false);
      }
    };

    fetchHistory();
  }, [publicKey, page]);

  if (!publicKey) {
    return null; // Don't show history if not connected
  }

  const totalPages = Math.ceil(total / limit);

  const styles = {
    container: {
      marginTop: SPACING.xl,
      marginBottom: SPACING.xl,
    } as React.CSSProperties,

    title: {
      ...TYPOGRAPHY.h2,
      marginBottom: SPACING.lg,
      color: COLORS.gray800,
    } as React.CSSProperties,

    table: {
      width: '100%',
      borderCollapse: 'collapse' as const,
      boxShadow: SHADOWS.sm,
      borderRadius: BORDERS.radiusLg,
      overflow: 'hidden',
      border: `1px solid ${COLORS.gray200}`,
    } as React.CSSProperties,

    thead: {
      backgroundColor: COLORS.gray100,
    } as React.CSSProperties,

    th: {
      backgroundColor: COLORS.gray100,
      color: COLORS.gray800,
      padding: SPACING.lg,
      textAlign: 'left' as const,
      fontSize: TYPOGRAPHY.label.fontSize,
      fontWeight: 600,
      borderBottom: `1px solid ${COLORS.gray200}`,
    } as React.CSSProperties,

    td: {
      padding: SPACING.lg,
      borderBottom: `1px solid ${COLORS.gray200}`,
      ...TYPOGRAPHY.body,
    } as React.CSSProperties,

    tbody: {} as React.CSSProperties,

    link: {
      color: COLORS.primary,
      textDecoration: 'none',
      cursor: 'pointer',
      transition: `color ${TRANSITIONS.fast}`,
      fontWeight: 600,
    } as React.CSSProperties,

    pagination: {
      display: 'flex',
      justifyContent: 'center',
      alignItems: 'center',
      gap: SPACING.md,
      marginTop: SPACING.xl,
      flexWrap: 'wrap' as const,
    } as React.CSSProperties,

    paginationButton: {
      padding: `${SPACING.sm} ${SPACING.md}`,
      backgroundColor: COLORS.primary,
      color: 'white',
      border: 'none',
      borderRadius: BORDERS.radius,
      cursor: 'pointer',
      transition: `all ${TRANSITIONS.fast}`,
      fontSize: TYPOGRAPHY.label.fontSize,
      fontWeight: 600,
    } as React.CSSProperties,

    paginationButtonDisabled: {
      opacity: 0.5,
      cursor: 'not-allowed',
    } as React.CSSProperties,

    paginationInfo: {
      color: COLORS.gray600,
      fontSize: TYPOGRAPHY.small.fontSize,
      lineHeight: TYPOGRAPHY.small.lineHeight,
    } as React.CSSProperties,

    loading: {
      textAlign: 'center' as const,
      padding: SPACING.xl,
      color: COLORS.gray600,
      ...TYPOGRAPHY.body,
    } as React.CSSProperties,

    empty: {
      textAlign: 'center' as const,
      padding: SPACING.xl,
      color: COLORS.gray600,
      fontSize: TYPOGRAPHY.body.fontSize,
      lineHeight: TYPOGRAPHY.body.lineHeight,
    } as React.CSSProperties,
  };

  const getStatusStyle = (status: string) => {
    const colors =
      status === 'confirmed'
        ? STATUS_COLORS.confirmed
        : status === 'pending'
          ? STATUS_COLORS.pending
          : STATUS_COLORS.failed;

    return {
      backgroundColor: colors.bg,
      color: colors.text,
      padding: `${SPACING.sm} ${SPACING.md}`,
      borderRadius: BORDERS.radius,
      fontSize: '0.85rem',
      fontWeight: 600,
      display: 'inline-block',
      border: `1px solid ${colors.border}`,
    } as React.CSSProperties;
  };

  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr);
    return date.toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: date.getFullYear() !== new Date().getFullYear() ? '2-digit' : undefined,
    });
  };

  const getStatusLabel = (status: string) => {
    switch (status) {
      case 'confirmed':
        return '✓ Confirmed';
      case 'pending':
        return '⏳ Pending';
      case 'failed':
        return '✗ Failed';
      default:
        return status;
    }
  };

  if (error) {
    return (
      <div style={styles.container}>
        <h2 style={styles.title}>Your Claims</h2>
        <div
          style={{
            backgroundColor: '#fee2e2',
            color: '#991b1b',
            padding: SPACING.lg,
            borderRadius: BORDERS.radiusLg,
            ...TYPOGRAPHY.small,
          }}
        >
          {error}
        </div>
      </div>
    );
  }

  return (
    <div style={styles.container}>
      <h2 style={styles.title}>
        Your Claims {total > 0 && `(${total} total)`}
      </h2>

      {loading ? (
        <div style={styles.loading}>⏳ Loading your claim history...</div>
      ) : claims.length === 0 ? (
        <div style={styles.empty}>
          📝 No claims yet. Claim your first tokens above!
        </div>
      ) : (
        <>
          <table style={styles.table}>
            <thead style={styles.thead}>
              <tr>
                <th style={styles.th}>Epoch</th>
                <th style={styles.th}>Amount</th>
                <th style={styles.th}>Status</th>
                <th style={styles.th}>Date</th>
                <th style={styles.th}>Transaction</th>
              </tr>
            </thead>
            <tbody style={styles.tbody}>
              {claims.map((claim, idx) => (
                <tr key={idx}>
                  <td style={styles.td}>
                    <strong>#{claim.epoch_id}</strong>
                  </td>
                  <td style={styles.td}>
                    {Number(claim.amount).toLocaleString()} CCM
                  </td>
                  <td style={styles.td}>
                    <span style={getStatusStyle(claim.status)}>
                      {getStatusLabel(claim.status)}
                    </span>
                  </td>
                  <td style={styles.td}>{formatDate(claim.claimed_at)}</td>
                  <td style={styles.td}>
                    {claim.tx_signature ? (
                      <a
                        href={`https://solscan.io/tx/${claim.tx_signature}`}
                        target="_blank"
                        rel="noopener noreferrer"
                        style={styles.link}
                      >
                        View on Solscan →
                      </a>
                    ) : (
                      <span style={{ color: COLORS.gray400 }}>—</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>

          {totalPages > 1 && (
            <div style={styles.pagination}>
              <button
                style={{
                  ...styles.paginationButton,
                  ...(page === 0 ? styles.paginationButtonDisabled : {}),
                }}
                onClick={() => setPage(Math.max(0, page - 1))}
                disabled={page === 0}
              >
                ← Previous
              </button>
              <span style={styles.paginationInfo}>
                Page {page + 1} of {totalPages}
              </span>
              <button
                style={{
                  ...styles.paginationButton,
                  ...(page >= totalPages - 1
                    ? styles.paginationButtonDisabled
                    : {}),
                }}
                onClick={() => setPage(Math.min(totalPages - 1, page + 1))}
                disabled={page >= totalPages - 1}
              >
                Next →
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
};

export default ClaimHistory;
