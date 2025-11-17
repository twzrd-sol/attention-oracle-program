import React, { useState, useEffect } from 'react';
import { getEpochs, Epoch } from '../lib/api';
import {
  COLORS,
  SPACING,
  TYPOGRAPHY,
  SHADOWS,
  TRANSITIONS,
  BORDERS,
} from '../lib/theme';

interface EpochTableProps {
  onSelectEpoch?: (epochId: number) => void;
}

/**
 * EpochTable Component
 *
 * Browse and select available epochs for claiming.
 * Displays: Epoch ID, Merkle Root (truncated), Status, Claimers, Total Amount
 * Pagination built-in (10 per page).
 *
 * Example:
 * <EpochTable onSelectEpoch={(epochId) => console.log(epochId)} />
 */
export const EpochTable: React.FC<EpochTableProps> = ({ onSelectEpoch }) => {
  const [epochs, setEpochs] = useState<Epoch[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(0);
  const [total, setTotal] = useState(0);

  const limit = 10;
  const offset = page * limit;

  useEffect(() => {
    const fetchEpochs = async () => {
      try {
        setLoading(true);
        const response = await getEpochs(limit, offset);
        setEpochs(response.epochs);
        setTotal(response.total);
        setError(null);
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Failed to load epochs';
        setError(message);
        setEpochs([]);
      } finally {
        setLoading(false);
      }
    };

    fetchEpochs();
  }, [page]);

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

    row: {
      cursor: 'pointer',
      transition: `background-color ${TRANSITIONS.fast}`,
    } as React.CSSProperties,

    statusOpen: {
      backgroundColor: '#d1fae5',
      color: '#065f46',
      padding: `${SPACING.sm} ${SPACING.md}`,
      borderRadius: BORDERS.radius,
      fontSize: '0.85rem',
      fontWeight: 600,
      display: 'inline-block',
    } as React.CSSProperties,

    statusClosed: {
      backgroundColor: COLORS.gray200,
      color: COLORS.gray600,
      padding: `${SPACING.sm} ${SPACING.md}`,
      borderRadius: BORDERS.radius,
      fontSize: '0.85rem',
      fontWeight: 600,
      display: 'inline-block',
    } as React.CSSProperties,

    rootCode: {
      fontFamily: 'monospace',
      fontSize: '0.85rem',
      backgroundColor: COLORS.gray100,
      padding: `${SPACING.xs} ${SPACING.sm}`,
      borderRadius: BORDERS.radius,
      color: COLORS.gray800,
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

    error: {
      backgroundColor: '#fee2e2',
      color: '#991b1b',
      padding: SPACING.lg,
      borderRadius: BORDERS.radiusLg,
      marginBottom: SPACING.lg,
      border: `1px solid #fca5a5`,
      ...TYPOGRAPHY.small,
    } as React.CSSProperties,

    empty: {
      textAlign: 'center' as const,
      padding: SPACING.xl,
      color: COLORS.gray600,
      fontSize: TYPOGRAPHY.body.fontSize,
      lineHeight: TYPOGRAPHY.body.lineHeight,
    } as React.CSSProperties,
  };

  if (error) {
    return (
      <div style={styles.container}>
        <h2 style={styles.title}>Available Epochs</h2>
        <div style={styles.error}>{error}</div>
      </div>
    );
  }

  return (
    <div style={styles.container}>
      <h2 style={styles.title}>Available Epochs</h2>

      {loading ? (
        <div style={styles.loading}>⏳ Loading epochs...</div>
      ) : epochs.length === 0 ? (
        <div style={styles.empty}>No epochs available yet. Check back soon!</div>
      ) : (
        <>
          <table style={styles.table}>
            <thead style={styles.thead}>
              <tr>
                <th style={styles.th}>Epoch ID</th>
                <th style={styles.th}>Merkle Root</th>
                <th style={styles.th}>Status</th>
                <th style={styles.th}>Claimers</th>
                <th style={styles.th}>Total Amount</th>
              </tr>
            </thead>
            <tbody style={styles.tbody}>
              {epochs.map((epoch) => (
                <tr
                  key={epoch.epoch_id}
                  style={styles.row}
                  onMouseEnter={(e) => {
                    (e.currentTarget as HTMLTableRowElement).style.backgroundColor =
                      COLORS.gray50;
                  }}
                  onMouseLeave={(e) => {
                    (e.currentTarget as HTMLTableRowElement).style.backgroundColor =
                      'transparent';
                  }}
                  onClick={() => onSelectEpoch?.(epoch.epoch_id)}
                >
                  <td style={styles.td}>
                    <strong>#{epoch.epoch_id}</strong>
                  </td>
                  <td style={styles.td}>
                    <code style={styles.rootCode}>
                      {epoch.merkle_root.substring(0, 10)}...
                    </code>
                  </td>
                  <td style={styles.td}>
                    <span
                      style={
                        epoch.is_open
                          ? styles.statusOpen
                          : styles.statusClosed
                      }
                    >
                      {epoch.is_open ? '✓ Open' : '✗ Closed'}
                    </span>
                  </td>
                  <td style={styles.td}>{epoch.total_claimers.toLocaleString()}</td>
                  <td style={styles.td}>
                    {Number(epoch.total_amount).toLocaleString()} CCM
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
                Page {page + 1} of {totalPages} ({total} total)
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

export default EpochTable;
