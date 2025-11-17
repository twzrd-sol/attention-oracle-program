import React from 'react';
import { useWallet as useSolanaWallet } from '@solana/wallet-adapter-react';
import { useWalletModal } from '@solana/wallet-adapter-react-ui';
import { useWallet } from '@hooks';

interface WalletConnectProps {
  onConnected?: () => void;
  proofClaimerAddress?: string;
}

export const WalletConnect: React.FC<WalletConnectProps> = ({ onConnected, proofClaimerAddress }) => {
  const { connected, connecting, shortAddress, connect, disconnect, error } = useWallet();
  const { setVisible } = useWalletModal();
  const { publicKey } = useSolanaWallet();

  const handleConnect = () => {
    setVisible(true);
  };

  const handleDisconnect = async () => {
    await disconnect();
  };

  React.useEffect(() => {
    if (connected && publicKey && onConnected) {
      onConnected();
    }
  }, [connected, publicKey, onConnected]);

  const isAddressMismatch = proofClaimerAddress && publicKey && proofClaimerAddress !== publicKey.toBase58();

  return (
    <div style={styles.container}>
      <div style={styles.card}>
        <h2 style={styles.title}>2. Connect Wallet</h2>
        <p style={styles.subtitle}>Connect your wallet to sign the claim transaction.</p>

        {!connected ? (
          <div>
            <p style={styles.description}>
              Select a wallet to connect. We support Phantom, Solflare, and Torus.
            </p>
            <button
              onClick={handleConnect}
              disabled={connecting}
              style={{
                ...styles.button,
                ...styles.buttonPrimary,
                opacity: connecting ? 0.7 : 1,
                cursor: connecting ? 'not-allowed' : 'pointer',
              }}
            >
              {connecting ? 'Connecting...' : 'Connect Wallet'}
            </button>

            {error && (
              <div style={styles.errorBox}>
                <strong>❌ Error:</strong> {error}
              </div>
            )}
          </div>
        ) : (
          <div>
            {/* Connected State */}
            <div style={styles.connectedBox}>
              <div style={styles.connectedHeader}>
                <div style={styles.connectedIcon}>✓</div>
                <span style={styles.connectedText}>Wallet Connected</span>
              </div>

              <div style={styles.addressBox}>
                <div style={styles.addressLabel}>Connected Address</div>
                <div style={styles.addressValue}>{shortAddress}</div>
                <div style={styles.addressFull}>{publicKey?.toBase58()}</div>
              </div>

              {/* Address Mismatch Warning */}
              {isAddressMismatch && (
                <div style={styles.warningBox}>
                  <strong>⚠️ Address Mismatch</strong>
                  <p style={styles.warningText}>
                    This proof is for {proofClaimerAddress?.slice(0, 8)}...{proofClaimerAddress?.slice(-6)},
                    but you connected {shortAddress}. You will not be able to claim with this wallet.
                  </p>
                  <button
                    onClick={handleDisconnect}
                    style={{ ...styles.button, ...styles.buttonSecondary }}
                  >
                    Disconnect & Use Different Wallet
                  </button>
                </div>
              )}

              {/* Success State */}
              {!isAddressMismatch && (
                <div style={styles.successMessage}>
                  ✅ Address matches proof. Ready to review claim.
                </div>
              )}

              {/* Disconnect Button */}
              {!isAddressMismatch && (
                <button
                  onClick={handleDisconnect}
                  style={{ ...styles.button, ...styles.buttonSecondary, marginTop: '1rem' }}
                >
                  Disconnect Wallet
                </button>
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

  description: {
    fontSize: '0.95rem',
    color: '#4b5563',
    marginBottom: '1rem',
    lineHeight: '1.5',
  } as React.CSSProperties,

  button: {
    padding: '0.75rem 1.5rem',
    borderRadius: '6px',
    border: 'none',
    fontSize: '0.95rem',
    fontWeight: '500',
    cursor: 'pointer',
    transition: 'all 0.2s',
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

  connectedBox: {
    padding: '1rem',
    backgroundColor: '#f0fdf4',
    border: '2px solid #22c55e',
    borderRadius: '6px',
    color: '#166534',
  } as React.CSSProperties,

  connectedHeader: {
    display: 'flex',
    alignItems: 'center',
    gap: '0.75rem',
    marginBottom: '1rem',
  } as React.CSSProperties,

  connectedIcon: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    width: '24px',
    height: '24px',
    borderRadius: '50%',
    backgroundColor: '#22c55e',
    color: 'white',
    fontWeight: 'bold',
    fontSize: '0.9rem',
  } as React.CSSProperties,

  connectedText: {
    fontSize: '1rem',
    fontWeight: '600',
  } as React.CSSProperties,

  addressBox: {
    padding: '1rem',
    backgroundColor: 'rgba(255,255,255,0.5)',
    borderRadius: '4px',
    marginBottom: '1rem',
    fontFamily: 'monospace',
    fontSize: '0.85rem',
  } as React.CSSProperties,

  addressLabel: {
    fontSize: '0.8rem',
    fontWeight: '500',
    color: '#6b7280',
    marginBottom: '0.25rem',
  } as React.CSSProperties,

  addressValue: {
    fontSize: '1rem',
    fontWeight: '600',
    color: '#166534',
    marginBottom: '0.25rem',
  } as React.CSSProperties,

  addressFull: {
    fontSize: '0.75rem',
    color: '#999',
    wordBreak: 'break-all',
  } as React.CSSProperties,

  warningBox: {
    padding: '1rem',
    backgroundColor: '#fef3c7',
    border: '1px solid #fcd34d',
    borderRadius: '6px',
    color: '#92400e',
    marginBottom: '1rem',
  } as React.CSSProperties,

  warningText: {
    fontSize: '0.9rem',
    margin: '0.5rem 0 1rem 0',
    lineHeight: '1.5',
  } as React.CSSProperties,

  errorBox: {
    padding: '1rem',
    backgroundColor: '#fee2e2',
    border: '1px solid #fca5a5',
    borderRadius: '6px',
    color: '#991b1b',
    fontSize: '0.9rem',
    marginTop: '1rem',
  } as React.CSSProperties,

  successMessage: {
    padding: '1rem',
    backgroundColor: 'rgba(255,255,255,0.7)',
    borderRadius: '4px',
    fontSize: '0.95rem',
    fontWeight: '500',
  } as React.CSSProperties,
};

export default WalletConnect;
