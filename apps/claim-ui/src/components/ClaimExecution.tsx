import React, { useState } from 'react';
import { useAnchorProgram, useWallet, useMerkleProof } from '@hooks';
import { buildClaimWithRingInstruction, submitClaimTransaction, deriveStreamerKey, EXPLORER_URL } from '@lib';
import { PublicKey } from '@solana/web3.js';

interface ClaimExecutionProps {
  onSuccess?: (signature: string) => void;
  onError?: (error: string) => void;
}

interface ExecutionState {
  status: 'idle' | 'building' | 'signing' | 'submitting' | 'confirming' | 'success' | 'error';
  signature?: string;
  error?: string;
  confirmations?: number;
}

export const ClaimExecution: React.FC<ClaimExecutionProps> = ({ onSuccess, onError }) => {
  const { program, provider, isReady, error: programError } = useAnchorProgram();
  const { address: walletAddress } = useWallet();
  const { proof } = useMerkleProof();

  const [executionState, setExecutionState] = useState<ExecutionState>({ status: 'idle' });
  const [showDetails, setShowDetails] = useState(false);

  const canExecute = isReady && walletAddress && proof && executionState.status === 'idle';

  const handleClaim = async () => {
    if (!canExecute || !program || !provider || !walletAddress || !proof) {
      setExecutionState({
        status: 'error',
        error: 'Missing required program, provider, wallet, or proof data',
      });
      return;
    }

    try {
      // Update status: building instruction
      setExecutionState({ status: 'building' });

      // Derive streamer key from channel
      const streamerKey = deriveStreamerKey(proof.channel);

      // Build the claim instruction
      const tx = await buildClaimWithRingInstruction(
        program,
        proof,
        walletAddress,
        streamerKey
      );

      // Update status: signing
      setExecutionState({ status: 'signing' });

      // Submit transaction (includes signing and confirmation)
      const signature = await submitClaimTransaction(provider, tx);

      // Update status: success
      setExecutionState({
        status: 'success',
        signature,
        confirmations: 1,
      });

      // Trigger callback
      onSuccess?.(signature);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);

      setExecutionState({
        status: 'error',
        error: errorMessage,
      });

      // Trigger error callback
      onError?.(errorMessage);
    }
  };

  const getStatusMessage = (): string => {
    switch (executionState.status) {
      case 'building':
        return 'Building claim instruction...';
      case 'signing':
        return 'Waiting for wallet signature...';
      case 'submitting':
        return 'Submitting transaction...';
      case 'confirming':
        return `Confirming transaction (${executionState.confirmations || 0} confirmations)...`;
      case 'success':
        return 'Claim successful! Transaction confirmed.';
      case 'error':
        return `Error: ${executionState.error}`;
      default:
        return 'Ready to claim';
    }
  };

  const isLoading = ['building', 'signing', 'submitting', 'confirming'].includes(executionState.status);
  const isError = executionState.status === 'error';
  const isSuccess = executionState.status === 'success';

  return (
    <div style={styles.container}>
      <div style={styles.card}>
        <h2 style={styles.title}>4. Execute Claim</h2>
        <p style={styles.subtitle}>Sign and submit your claim transaction to the blockchain.</p>

        {/* Status Box */}
        <div
          style={{
            ...styles.statusBox,
            ...(isSuccess && styles.successBox),
            ...(isError && styles.errorBox),
            ...(!isLoading && !isSuccess && !isError && styles.readyBox),
          }}
        >
          <div style={styles.statusMessage}>
            {isLoading && <div style={styles.spinner} />}
            <span>{getStatusMessage()}</span>
          </div>

          {/* Transaction Details */}
          {executionState.signature && (
            <div style={styles.transactionDetails}>
              <div style={styles.detailRow}>
                <span style={styles.detailLabel}>Transaction Signature</span>
                <span
                  style={{
                    ...styles.detailValue,
                    cursor: 'pointer',
                    textDecoration: 'underline',
                    color: '#3b82f6',
                  }}
                  onClick={() =>
                    window.open(
                      `${EXPLORER_URL}tx/${executionState.signature}?cluster=mainnet-beta`,
                      '_blank'
                    )
                  }
                >
                  {executionState.signature.slice(0, 20)}...
                </span>
              </div>
              {executionState.confirmations && (
                <div style={styles.detailRow}>
                  <span style={styles.detailLabel}>Confirmations</span>
                  <span style={styles.detailValue}>{executionState.confirmations}</span>
                </div>
              )}
            </div>
          )}
        </div>

        {/* Claim Summary Before Execution */}
        {!isSuccess && proof && (
          <div style={styles.section}>
            <h3 style={styles.sectionTitle}>Claim Summary</h3>
            <div style={styles.summaryBox}>
              <div style={styles.summaryRow}>
                <span style={styles.summaryLabel}>Channel</span>
                <span style={styles.summaryValue}>{proof.channel}</span>
              </div>
              <div style={styles.summaryRow}>
                <span style={styles.summaryLabel}>Amount</span>
                <span style={styles.summaryValue}>{Number(proof.amount).toLocaleString()} tokens</span>
              </div>
              <div style={styles.summaryRow}>
                <span style={styles.summaryLabel}>Claimer</span>
                <span style={styles.summaryValue}>{walletAddress?.toBase58().slice(0, 8)}...{walletAddress?.toBase58().slice(-6)}</span>
              </div>
            </div>
          </div>
        )}

        {/* Success Message with Next Steps */}
        {isSuccess && (
          <div style={styles.successMessage}>
            <h3 style={styles.successTitle}>✅ Claim Successful!</h3>
            <p style={styles.successText}>
              Your claim has been processed and confirmed on the blockchain. The tokens have been
              transferred to your wallet.
            </p>
            <div style={styles.nextSteps}>
              <p style={styles.nextStepsTitle}>Next Steps:</p>
              <ul style={styles.stepsList}>
                <li>Check your wallet for the new tokens</li>
                <li>
                  View the transaction on{' '}
                  <a
                    href={`${EXPLORER_URL}tx/${executionState.signature}?cluster=mainnet-beta`}
                    target="_blank"
                    rel="noopener noreferrer"
                    style={styles.explorerLink}
                  >
                    Solscan
                  </a>
                </li>
                <li>Consider swapping tokens on a DEX or holding for future appreciation</li>
              </ul>
            </div>
          </div>
        )}

        {/* Error Details */}
        {isError && (
          <div style={styles.errorDetails}>
            <h3 style={styles.errorTitle}>Transaction Failed</h3>
            <p style={styles.errorDescription}>{executionState.error}</p>
            <p style={styles.errorHint}>Please check your wallet and try again, or contact support if the issue persists.</p>
          </div>
        )}

        {/* Program Connection Status */}
        {!isReady && (
          <div style={styles.warningBox}>
            <strong>⚠️ Program Not Ready</strong>
            <p style={styles.warningText}>
              The Anchor program is still initializing. Please wait a moment before attempting to claim.
              {programError && ` Error: ${programError}`}
            </p>
          </div>
        )}

        {/* Action Buttons */}
        {!isSuccess && (
          <div style={styles.buttonGroup}>
            <button
              onClick={handleClaim}
              disabled={!canExecute}
              style={{
                ...styles.button,
                ...styles.buttonPrimary,
                opacity: canExecute ? 1 : 0.5,
                cursor: canExecute ? 'pointer' : 'not-allowed',
              }}
            >
              {isLoading ? (
                <>
                  <span style={styles.spinnerSmall} /> Claiming...
                </>
              ) : (
                'Execute Claim'
              )}
            </button>

            {/* Show Details Toggle */}
            <button
              onClick={() => setShowDetails(!showDetails)}
              style={{ ...styles.button, ...styles.buttonSecondary }}
            >
              {showDetails ? 'Hide Details' : 'Show Details'}
            </button>
          </div>
        )}

        {/* Technical Details (Optional) */}
        {showDetails && (
          <div style={styles.detailsBox}>
            <h4 style={styles.detailsTitle}>Technical Details</h4>
            <div style={styles.detailsContent}>
              <p>
                <strong>Wallet Address:</strong> {walletAddress?.toBase58() || 'Not connected'}
              </p>
              <p>
                <strong>Program Ready:</strong> {isReady ? 'Yes' : 'No'}
              </p>
              <p>
                <strong>Proof Loaded:</strong> {proof ? 'Yes' : 'No'}
              </p>
              <p>
                <strong>Execution Status:</strong> {executionState.status}
              </p>
              {executionState.signature && (
                <p>
                  <strong>Transaction:</strong> {executionState.signature}
                </p>
              )}
            </div>
          </div>
        )}
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

  statusBox: {
    padding: '1.5rem',
    backgroundColor: '#f9fafb',
    border: '1px solid #e5e7eb',
    borderRadius: '6px',
    marginBottom: '1.5rem',
  } as React.CSSProperties,

  readyBox: {
    backgroundColor: '#f0fdf4',
    border: '2px solid #22c55e',
  } as React.CSSProperties,

  successBox: {
    backgroundColor: '#f0fdf4',
    border: '2px solid #22c55e',
    color: '#166534',
  } as React.CSSProperties,

  errorBox: {
    backgroundColor: '#fee2e2',
    border: '2px solid #fca5a5',
    color: '#991b1b',
  } as React.CSSProperties,

  statusMessage: {
    display: 'flex',
    alignItems: 'center',
    gap: '0.75rem',
    fontSize: '1rem',
    fontWeight: '500',
  } as React.CSSProperties,

  spinner: {
    display: 'inline-block',
    width: '16px',
    height: '16px',
    borderRadius: '50%',
    border: '2px solid rgba(0,0,0,0.1)',
    borderTopColor: '#3b82f6',
    animation: 'spin 0.6s linear infinite',
  } as React.CSSProperties,

  spinnerSmall: {
    display: 'inline-block',
    width: '12px',
    height: '12px',
    borderRadius: '50%',
    border: '2px solid rgba(255,255,255,0.3)',
    borderTopColor: 'white',
    marginRight: '0.5rem',
  } as React.CSSProperties,

  transactionDetails: {
    marginTop: '1rem',
    paddingTop: '1rem',
    borderTop: '1px solid rgba(0,0,0,0.1)',
  } as React.CSSProperties,

  detailRow: {
    display: 'flex',
    justifyContent: 'space-between',
    padding: '0.5rem 0',
    fontSize: '0.9rem',
  } as React.CSSProperties,

  detailLabel: {
    fontWeight: '500',
    color: '#6b7280',
  } as React.CSSProperties,

  detailValue: {
    fontFamily: 'monospace',
    fontSize: '0.85rem',
    color: '#1f2937',
    wordBreak: 'break-all',
  } as React.CSSProperties,

  section: {
    marginBottom: '1.5rem',
  } as React.CSSProperties,

  sectionTitle: {
    fontSize: '1rem',
    fontWeight: '600',
    color: '#1f2937',
    margin: '0 0 0.75rem 0',
  } as React.CSSProperties,

  summaryBox: {
    backgroundColor: '#f9fafb',
    border: '1px solid #e5e7eb',
    borderRadius: '6px',
    padding: '1rem',
  } as React.CSSProperties,

  summaryRow: {
    display: 'flex',
    justifyContent: 'space-between',
    padding: '0.5rem 0',
    borderBottom: '1px solid #e5e7eb',
  } as React.CSSProperties,

  summaryLabel: {
    fontWeight: '500',
    color: '#6b7280',
    fontSize: '0.9rem',
  } as React.CSSProperties,

  summaryValue: {
    fontWeight: '600',
    color: '#1f2937',
    fontFamily: 'monospace',
  } as React.CSSProperties,

  successMessage: {
    padding: '1.5rem',
    backgroundColor: '#f0fdf4',
    border: '2px solid #22c55e',
    borderRadius: '6px',
    marginBottom: '1.5rem',
    color: '#166534',
  } as React.CSSProperties,

  successTitle: {
    fontSize: '1.2rem',
    fontWeight: '600',
    margin: '0 0 0.75rem 0',
  } as React.CSSProperties,

  successText: {
    fontSize: '0.95rem',
    lineHeight: '1.5',
    margin: '0 0 1rem 0',
  } as React.CSSProperties,

  nextSteps: {
    marginTop: '1rem',
    paddingTop: '1rem',
    borderTop: '1px solid rgba(34, 197, 94, 0.2)',
  } as React.CSSProperties,

  nextStepsTitle: {
    fontSize: '0.9rem',
    fontWeight: '600',
    margin: '0 0 0.5rem 0',
  } as React.CSSProperties,

  stepsList: {
    fontSize: '0.9rem',
    lineHeight: '1.8',
    paddingLeft: '1.5rem',
    margin: '0',
  } as React.CSSProperties,

  explorerLink: {
    color: '#166534',
    fontWeight: '600',
    textDecoration: 'underline',
  } as React.CSSProperties,

  errorDetails: {
    padding: '1rem',
    backgroundColor: '#fee2e2',
    border: '1px solid #fca5a5',
    borderRadius: '6px',
    marginBottom: '1.5rem',
    color: '#991b1b',
  } as React.CSSProperties,

  errorTitle: {
    fontSize: '1rem',
    fontWeight: '600',
    margin: '0 0 0.5rem 0',
  } as React.CSSProperties,

  errorDescription: {
    fontSize: '0.9rem',
    margin: '0 0 0.5rem 0',
    wordBreak: 'break-word',
  } as React.CSSProperties,

  errorHint: {
    fontSize: '0.85rem',
    color: '#7c2d12',
    margin: '0',
  } as React.CSSProperties,

  warningBox: {
    padding: '1rem',
    backgroundColor: '#fef3c7',
    border: '1px solid #fcd34d',
    borderRadius: '6px',
    color: '#92400e',
    marginBottom: '1.5rem',
  } as React.CSSProperties,

  warningText: {
    fontSize: '0.9rem',
    margin: '0.5rem 0 0 0',
    lineHeight: '1.5',
  } as React.CSSProperties,

  buttonGroup: {
    display: 'flex',
    gap: '1rem',
    marginTop: '1.5rem',
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

  buttonSecondary: {
    backgroundColor: '#f3f4f6',
    color: '#374151',
    border: '1px solid #d1d5db',
  } as React.CSSProperties,

  detailsBox: {
    padding: '1rem',
    backgroundColor: '#f3f4f6',
    border: '1px solid #d1d5db',
    borderRadius: '6px',
    marginTop: '1.5rem',
    fontSize: '0.85rem',
    fontFamily: 'monospace',
  } as React.CSSProperties,

  detailsTitle: {
    fontSize: '0.95rem',
    fontWeight: '600',
    margin: '0 0 0.75rem 0',
    color: '#1f2937',
  } as React.CSSProperties,

  detailsContent: {
    lineHeight: '1.8',
  } as React.CSSProperties,
};

export default ClaimExecution;
