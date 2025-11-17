import React, { useMemo } from 'react';
import { useMerkleProof } from '@hooks';
import { EXPLORER_URL } from '@lib';

interface ClaimReviewProps {
  onProceed?: () => void;
  walletAddress?: string;
}

export const ClaimReview: React.FC<ClaimReviewProps> = ({ onProceed, walletAddress }) => {
  const { proof, getSummary } = useMerkleProof();
  const summary = getSummary();

  const feeBreakdown = useMemo(() => {
    if (!proof) return null;

    const gross = BigInt(proof.amount);
    const treasuryFee = (gross * BigInt(5)) / BigInt(10000); // 0.05%
    const creatorFeeBps = 5; // 0.05%
    const tierMultiplier = 100; // 1.0x (elite tier)
    const creatorFee = (gross * BigInt(creatorFeeBps * tierMultiplier)) / BigInt(1000000);
    const totalFee = treasuryFee + creatorFee;
    const net = gross - totalFee;

    return {
      gross,
      treasuryFee,
      creatorFee,
      totalFee,
      net,
      feePercentage: (Number(totalFee) / Number(gross) * 100).toFixed(2),
    };
  }, [proof]);

  const formatAmount = (amount: bigint): string => {
    return amount.toLocaleString();
  };

  if (!proof) {
    return (
      <div style={styles.container}>
        <div style={styles.card}>
          <div style={styles.emptyState}>
            <p>No proof loaded. Please load a proof first.</p>
          </div>
        </div>
      </div>
    );
  }

  const canProceed = walletAddress === proof.claimer;

  return (
    <div style={styles.container}>
      <div style={styles.card}>
        <h2 style={styles.title}>3. Review Claim</h2>
        <p style={styles.subtitle}>Verify your claim details before submitting.</p>

        {/* Claim Summary */}
        <div style={styles.section}>
          <h3 style={styles.sectionTitle}>Claim Details</h3>
          <div style={styles.grid}>
            <div style={styles.gridItem}>
              <div style={styles.label}>Channel</div>
              <div style={styles.value}>{proof.channel}</div>
            </div>
            <div style={styles.gridItem}>
              <div style={styles.label}>Epoch</div>
              <div style={styles.value}>{proof.epoch}</div>
            </div>
            <div style={styles.gridItem}>
              <div style={styles.label}>Claim Index</div>
              <div style={styles.value}>{proof.index}</div>
            </div>
            <div style={styles.gridItem}>
              <div style={styles.label}>Claimer</div>
              <div style={styles.value}>{proof.claimer.slice(0, 8)}...{proof.claimer.slice(-6)}</div>
            </div>
          </div>
        </div>

        {/* Amount Breakdown */}
        <div style={styles.section}>
          <h3 style={styles.sectionTitle}>Amount Breakdown</h3>
          <div style={styles.amountTable}>
            <div style={styles.tableRow}>
              <span style={styles.tableLabel}>Gross Amount</span>
              <span style={styles.tableValue}>{feeBreakdown ? formatAmount(feeBreakdown.gross) : '—'} tokens</span>
            </div>
            <div style={{ ...styles.tableRow, borderTop: '1px solid #e5e7eb', paddingTop: '0.75rem' }}>
              <span style={styles.tableLabel}>Treasury Fee (0.05%)</span>
              <span style={styles.tableValue}>{feeBreakdown ? formatAmount(feeBreakdown.treasuryFee) : '—'} tokens</span>
            </div>
            <div style={styles.tableRow}>
              <span style={styles.tableLabel}>Creator Fee (0.05%)</span>
              <span style={styles.tableValue}>{feeBreakdown ? formatAmount(feeBreakdown.creatorFee) : '—'} tokens</span>
            </div>
            <div style={{ ...styles.tableRow, borderTop: '1px solid #e5e7eb', paddingTop: '0.75rem' }}>
              <span style={styles.tableLabel}>Total Fees</span>
              <span style={styles.tableValue}>{feeBreakdown ? formatAmount(feeBreakdown.totalFee) : '—'} tokens ({feeBreakdown?.feePercentage}%)</span>
            </div>
          </div>

          {/* Net Amount Highlight */}
          <div style={styles.netAmountBox}>
            <div style={styles.netAmountLabel}>Net Amount to Receive</div>
            <div style={styles.netAmountValue}>{feeBreakdown ? formatAmount(feeBreakdown.net) : '—'}</div>
            <div style={styles.netAmountUnit}>tokens</div>
          </div>
        </div>

        {/* Proof Details */}
        <div style={styles.section}>
          <h3 style={styles.sectionTitle}>Proof Information</h3>
          <div style={styles.proofDetails}>
            <div style={styles.detailRow}>
              <span style={styles.detailLabel}>Root Hash</span>
              <span style={styles.detailValue}>{proof.root.slice(0, 16)}...{proof.root.slice(-16)}</span>
            </div>
            <div style={styles.detailRow}>
              <span style={styles.detailLabel}>Proof Depth</span>
              <span style={styles.detailValue}>{proof.proof.length} merkle nodes</span>
            </div>
            <div style={styles.detailRow}>
              <span style={styles.detailLabel}>Claim ID</span>
              <span style={styles.detailValue}>{proof.id.slice(0, 20)}...</span>
            </div>
          </div>
        </div>

        {/* Address Check */}
        <div style={styles.section}>
          <h3 style={styles.sectionTitle}>Address Verification</h3>
          {walletAddress === proof.claimer ? (
            <div style={styles.successBox}>
              ✅ Wallet address matches proof claimer. Ready to proceed.
            </div>
          ) : (
            <div style={styles.errorBox}>
              ❌ Wallet address does not match proof. Please connect the correct wallet.
            </div>
          )}
        </div>

        {/* Action Buttons */}
        <div style={styles.buttonGroup}>
          <button
            onClick={onProceed}
            disabled={!canProceed}
            style={{
              ...styles.button,
              ...styles.buttonPrimary,
              opacity: canProceed ? 1 : 0.5,
              cursor: canProceed ? 'pointer' : 'not-allowed',
            }}
          >
            Proceed to Claim
          </button>
        </div>
      </div>
    </div>
  );
};

const styles = {
  container: {
    padding: '1.5rem',
  } as React.CSSProperties,

  card: {
    backgroundColor: '#ffffff',
    border: '1px solid #e5e7eb',
    borderRadius: '8px',
    padding: '1.5rem',
    boxShadow: '0 1px 3px rgba(0,0,0,0.1)',
  } as React.CSSProperties,

  title: {
    fontSize: '1.5rem',
    fontWeight: '600',
    margin: '0 0 0.5rem 0',
    color: '#1f2937',
  } as React.CSSProperties,

  subtitle: {
    fontSize: '0.9rem',
    color: '#6b7280',
    margin: '0 0 1.5rem 0',
  } as React.CSSProperties,

  emptyState: {
    padding: '2rem',
    textAlign: 'center',
    color: '#6b7280',
  } as React.CSSProperties,

  section: {
    marginBottom: '2rem',
  } as React.CSSProperties,

  sectionTitle: {
    fontSize: '1.1rem',
    fontWeight: '600',
    color: '#1f2937',
    margin: '0 0 1rem 0',
  } as React.CSSProperties,

  grid: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))',
    gap: '1rem',
  } as React.CSSProperties,

  gridItem: {
    padding: '1rem',
    backgroundColor: '#f9fafb',
    borderRadius: '6px',
    border: '1px solid #e5e7eb',
  } as React.CSSProperties,

  label: {
    fontSize: '0.8rem',
    fontWeight: '500',
    color: '#6b7280',
    marginBottom: '0.5rem',
  } as React.CSSProperties,

  value: {
    fontSize: '1rem',
    fontWeight: '600',
    color: '#1f2937',
    fontFamily: 'monospace',
    wordBreak: 'break-all',
  } as React.CSSProperties,

  amountTable: {
    backgroundColor: '#f9fafb',
    border: '1px solid #e5e7eb',
    borderRadius: '6px',
    padding: '1rem',
    marginBottom: '1rem',
  } as React.CSSProperties,

  tableRow: {
    display: 'flex',
    justifyContent: 'space-between',
    padding: '0.75rem 0',
    fontSize: '0.95rem',
  } as React.CSSProperties,

  tableLabel: {
    fontWeight: '500',
    color: '#374151',
  } as React.CSSProperties,

  tableValue: {
    fontWeight: '600',
    color: '#1f2937',
    fontFamily: 'monospace',
  } as React.CSSProperties,

  netAmountBox: {
    padding: '1.5rem',
    backgroundColor: '#f0fdf4',
    border: '2px solid #22c55e',
    borderRadius: '6px',
    textAlign: 'center',
  } as React.CSSProperties,

  netAmountLabel: {
    fontSize: '0.9rem',
    fontWeight: '500',
    color: '#6b7280',
    marginBottom: '0.5rem',
  } as React.CSSProperties,

  netAmountValue: {
    fontSize: '2rem',
    fontWeight: '700',
    color: '#22c55e',
    fontFamily: 'monospace',
  } as React.CSSProperties,

  netAmountUnit: {
    fontSize: '0.9rem',
    color: '#6b7280',
  } as React.CSSProperties,

  proofDetails: {
    backgroundColor: '#f9fafb',
    border: '1px solid #e5e7eb',
    borderRadius: '6px',
    padding: '1rem',
    fontFamily: 'monospace',
    fontSize: '0.85rem',
  } as React.CSSProperties,

  detailRow: {
    display: 'flex',
    justifyContent: 'space-between',
    padding: '0.75rem 0',
    borderBottom: '1px solid #e5e7eb',
  } as React.CSSProperties,

  detailLabel: {
    fontWeight: '500',
    color: '#6b7280',
  } as React.CSSProperties,

  detailValue: {
    color: '#1f2937',
    wordBreak: 'break-all',
  } as React.CSSProperties,

  successBox: {
    padding: '1rem',
    backgroundColor: '#f0fdf4',
    border: '1px solid #86efac',
    borderRadius: '6px',
    color: '#166534',
    fontSize: '0.95rem',
  } as React.CSSProperties,

  errorBox: {
    padding: '1rem',
    backgroundColor: '#fee2e2',
    border: '1px solid #fca5a5',
    borderRadius: '6px',
    color: '#991b1b',
    fontSize: '0.95rem',
  } as React.CSSProperties,

  buttonGroup: {
    display: 'flex',
    gap: '1rem',
    marginTop: '2rem',
  } as React.CSSProperties,

  button: {
    padding: '0.75rem 1.5rem',
    borderRadius: '6px',
    border: 'none',
    fontSize: '0.95rem',
    fontWeight: '500',
    cursor: 'pointer',
    transition: 'all 0.2s',
    flex: 1,
  } as React.CSSProperties,

  buttonPrimary: {
    backgroundColor: '#3b82f6',
    color: 'white',
  } as React.CSSProperties,
};

export default ClaimReview;
