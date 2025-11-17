import React, { useState, useEffect, useCallback } from 'react';
import { useWallet, useConnection } from '@solana/wallet-adapter-react';
import { Transaction, VersionedTransaction } from '@solana/web3.js';
import { getVerificationStatus, requestClaimTransaction, getTwitterUrl, getDiscordInviteUrl, type VerificationStatus, bindWalletWithTwitch, fetchBoundWallet } from '@/lib/api';
import { getExplorerUrl } from '@/lib/solana';
import { PassportBadge } from './PassportBadge';
import { EpochTable } from './EpochTable';
import { ClaimHistory } from './ClaimHistory';
import { buildTwitchAuthUrl, extractTokenFromHash, storeTwitchToken, getStoredTwitchToken, removeTokenFromUrl, clearTwitchToken } from '@/lib/twitch';

interface ClaimState {
  status: 'idle' | 'loading' | 'verifying' | 'claiming' | 'confirming' | 'success' | 'error';
  error?: string;
  signature?: string;
  verification?: VerificationStatus;
}

export const ClaimCLS: React.FC = () => {
  const { publicKey, sendTransaction, connected } = useWallet();
  const { connection } = useConnection();

  const [state, setState] = useState<ClaimState>({ status: 'idle' });
  const [epochId, setEpochId] = useState(0);
  const [refreshing, setRefreshing] = useState(false);
  const [twitchToken, setTwitchToken] = useState<string | null>(null);
  const [bindingState, setBindingState] = useState<'idle' | 'checking' | 'binding' | 'bound' | 'error'>('idle');
  const [boundWallet, setBoundWallet] = useState<string | null>(null);
  const [bindingError, setBindingError] = useState<string | null>(null);

  /**
   * Fetch and update verification status
   */
  const fetchVerificationStatus = useCallback(async () => {
    if (!publicKey) {
      setState(prev => ({ ...prev, verification: undefined }));
      return;
    }

    try {
      setRefreshing(true);
      const status = await getVerificationStatus(publicKey.toBase58());
      setState(prev => ({
        ...prev,
        verification: status,
        status: 'idle',
      }));
    } catch (err) {
      const error = err instanceof Error ? err.message : 'Failed to fetch verification status';
      setState(prev => ({
        ...prev,
        status: 'error',
        error,
      }));
    } finally {
      setRefreshing(false);
    }
  }, [publicKey]);

  /**
   * Auto-fetch verification status when wallet connects
   */
  useEffect(() => {
    if (connected) {
      fetchVerificationStatus();
    }
  }, [connected, publicKey, fetchVerificationStatus]);

  useEffect(() => {
    const tokenFromHash = extractTokenFromHash(window.location.hash);
    if (tokenFromHash) {
      storeTwitchToken(tokenFromHash);
      removeTokenFromUrl();
      setTwitchToken(tokenFromHash);
      return;
    }
    setTwitchToken(getStoredTwitchToken());
  }, []);

  useEffect(() => {
    if (!twitchToken) {
      setBoundWallet(null);
      setBindingState('idle');
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        setBindingState('checking');
        const result = await fetchBoundWallet(twitchToken);
        if (cancelled) return;
        setBoundWallet(result.wallet ?? null);
        setBindingState(result.wallet ? 'bound' : 'idle');
        setBindingError(null);
      } catch (err) {
        if (cancelled) return;
        setBindingState('error');
        setBindingError(err instanceof Error ? err.message : 'Failed to load Twitch binding');
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [twitchToken]);

  /**
   * Handle claim transaction
   */
  const handleClaim = async () => {
    if (!publicKey) {
      setState(prev => ({
        ...prev,
        status: 'error',
        error: 'Wallet not connected',
      }));
      return;
    }

    if (!state.verification?.twitterFollowed || !state.verification?.discordJoined) {
      setState(prev => ({
        ...prev,
        status: 'error',
        error: 'Please complete all verification steps before claiming',
      }));
      return;
    }

    try {
      setState({ status: 'claiming' });

      // Request claim transaction from backend
      const claimResponse = await requestClaimTransaction(publicKey.toBase58(), epochId);

      // Decode base64 transaction
      const transactionBuffer = Buffer.from(claimResponse.transaction, 'base64');
      const transaction = Transaction.from(transactionBuffer);

      // Send transaction
      setState({ status: 'confirming' });
      const signature = await sendTransaction(transaction, connection);

      // Confirm transaction
      const confirmation = await connection.confirmTransaction(signature, 'confirmed');

      if (confirmation.value.err) {
        throw new Error(`Transaction failed: ${JSON.stringify(confirmation.value.err)}`);
      }

      setState({
        status: 'success',
        signature,
      });
    } catch (err) {
      const error = err instanceof Error ? err.message : 'Unknown error during claim';
      setState({
        status: 'error',
        error,
      });
    }
  };

  const handleTwitchConnect = () => {
    try {
      window.location.href = buildTwitchAuthUrl();
    } catch (err) {
      setBindingState('error');
      setBindingError(err instanceof Error ? err.message : 'Unable to start Twitch auth');
    }
  };

  const handleTwitchDisconnect = () => {
    clearTwitchToken();
    setTwitchToken(null);
    setBoundWallet(null);
    setBindingError(null);
    setBindingState('idle');
  };

  const handleBindWallet = async () => {
    if (!publicKey) {
      setBindingState('error');
      setBindingError('Connect a Solana wallet first');
      return;
    }
    if (!twitchToken) {
      setBindingState('error');
      setBindingError('Connect your Twitch account first');
      return;
    }
    try {
      setBindingState('binding');
      setBindingError(null);
      await bindWalletWithTwitch(twitchToken, publicKey.toBase58());
      setBoundWallet(publicKey.toBase58());
      setBindingState('bound');
    } catch (err) {
      setBindingState('error');
      setBindingError(err instanceof Error ? err.message : 'Failed to bind wallet');
    }
  };

  const canClaim =
    connected &&
    state.verification?.twitterFollowed &&
    state.verification?.discordJoined &&
    state.status === 'idle';

  const isLoading = ['claiming', 'confirming'].includes(state.status);
  const twitchConnected = Boolean(twitchToken);
  const twitchStatusLabel = twitchConnected ? 'Connected' : 'Not Connected';
  const isWalletBound = Boolean(publicKey && boundWallet && boundWallet === publicKey.toBase58());
  const bindingBusy = bindingState === 'binding' || bindingState === 'checking';

  return (
    <div style={styles.container}>
      {/* Passport Badge - Show tier and multiplier */}
      {connected && publicKey && state.verification && (
        <PassportBadge
          tier={state.verification.passportTier ?? 0}
          score={0}
          nextTierScore={10000}
        />
      )}

      {/* Main Content */}
      <div style={styles.card}>
        <h2 style={styles.title}>Claim CLS Tokens</h2>
        <p style={styles.subtitle}>
          Verify your identity and claim your creator tokens from Twitch channel rewards.
        </p>

        {/* Verification Status Section */}
        <div style={styles.section}>
          <h3 style={styles.sectionTitle}>Twitch Identity Binding</h3>
          <p style={styles.sectionDescription}>
            Bind your Twitch identity to this Solana wallet to enable ring claims. You’ll be redirected to Twitch to approve access.
          </p>
          <div style={styles.twitchRow}>
            <div>
              <strong>Status:</strong> {twitchStatusLabel}
              {twitchConnected && boundWallet && (
                <span style={styles.twitchSubtext}>
                  {' '}— Bound wallet: {boundWallet.slice(0, 4)}…{boundWallet.slice(-4)}
                </span>
              )}
            </div>
            <div>
              {!twitchConnected ? (
                <button style={styles.twitchButton} onClick={handleTwitchConnect}>
                  Connect Twitch
                </button>
              ) : (
                <button style={styles.secondaryButton} onClick={handleTwitchDisconnect}>
                  Disconnect
                </button>
              )}
            </div>
          </div>
          {twitchConnected && (
            <div style={styles.twitchActions}>
              <button
                style={{
                  ...styles.twitchButton,
                  opacity: bindingBusy ? 0.6 : 1,
                  cursor: bindingBusy ? 'not-allowed' : 'pointer',
                }}
                disabled={bindingBusy || !publicKey}
                onClick={handleBindWallet}
              >
                {bindingBusy ? 'Binding…' : isWalletBound ? 'Wallet Bound' : 'Bind This Wallet'}
              </button>
              {!publicKey && <p style={styles.twitchSubtext}>Connect your Solana wallet to bind.</p>}
              {bindingError && <p style={styles.errorText}>{bindingError}</p>}
            </div>
          )}
        </div>

        {/* Verification Status Section */}
        {connected && (
          <div style={styles.section}>
            <h3 style={styles.sectionTitle}>Verification Status</h3>

            {/* Twitter Verification Tile */}
            <VerificationTile
              icon="𝕏"
              label="Follow on X"
              verified={state.verification?.twitterFollowed || false}
              url={getTwitterUrl()}
              onOpen={() => window.open(getTwitterUrl(), '_blank')}
            />

            {/* Discord Verification Tile */}
            <VerificationTile
              icon="💬"
              label="Join Discord"
              verified={state.verification?.discordJoined || false}
              url={getDiscordInviteUrl()}
              onOpen={() => window.open(getDiscordInviteUrl(), '_blank')}
            />

            {/* Refresh Verification Button */}
            <button
              onClick={fetchVerificationStatus}
              disabled={refreshing}
              style={{
                ...styles.refreshButton,
                opacity: refreshing ? 0.6 : 1,
                cursor: refreshing ? 'not-allowed' : 'pointer',
              }}
            >
              {refreshing ? 'Refreshing...' : 'Refresh Verification Status'}
            </button>
          </div>
        )}

        {/* Epoch Browser - Browse and select epochs */}
        {connected && (
          <div style={styles.section}>
            <EpochTable onSelectEpoch={(selectedEpochId) => {
              setEpochId(selectedEpochId);
              // Scroll to claim button
              const claimSection = document.getElementById('claim-section');
              claimSection?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
            }} />
          </div>
        )}

        {/* Epoch Selector Section - Keep manual input as fallback */}
        {connected && (
          <div style={styles.section} id="claim-section">
            <h3 style={styles.sectionTitle}>Selected Epoch</h3>
            <div style={styles.epochInputGroup}>
              <label style={styles.label}>Epoch ID</label>
              <input
                type="number"
                min="0"
                value={epochId}
                onChange={e => setEpochId(Math.max(0, parseInt(e.target.value) || 0))}
                disabled={isLoading}
                style={styles.input}
              />
              <p style={styles.hint}>Selected from table above, or enter manually.</p>
            </div>
          </div>
        )}

        {/* Error State */}
        {state.status === 'error' && (
          <div style={styles.errorBox}>
            <strong>❌ Error:</strong>
            <p style={styles.errorText}>{state.error}</p>
            <button
              onClick={() => setState({ status: 'idle' })}
              style={{ ...styles.button, ...styles.buttonSecondary, marginTop: '0.75rem' }}
            >
              Dismiss
            </button>
          </div>
        )}

        {/* Success State */}
        {state.status === 'success' && state.signature && (
          <div style={styles.successBox}>
            <div style={styles.successTitle}>✅ Claim Successful!</div>
            <p style={styles.successText}>Your tokens have been claimed and transferred to your wallet.</p>
            <div style={styles.signatureBox}>
              <p style={styles.signatureLabel}>Transaction Signature</p>
              <a
                href={getExplorerUrl(state.signature)}
                target="_blank"
                rel="noopener noreferrer"
                style={styles.explorerLink}
              >
                {state.signature.slice(0, 20)}...{state.signature.slice(-20)}
              </a>
            </div>
            <button
              onClick={() => setState({ status: 'idle' })}
              style={{ ...styles.button, ...styles.buttonPrimary, marginTop: '1rem' }}
            >
              Claim Again
            </button>
          </div>
        )}

        {/* Claim Button */}
        {state.status !== 'success' && (
          <div style={styles.buttonGroup}>
            <button
              onClick={handleClaim}
              disabled={!canClaim}
              style={{
                ...styles.button,
                ...styles.buttonPrimary,
                opacity: canClaim ? 1 : 0.5,
                cursor: canClaim ? 'pointer' : 'not-allowed',
              }}
            >
              {isLoading ? (
                <>
                  <span style={styles.spinner} /> Claiming...
                </>
              ) : (
                'Claim CLS Tokens'
              )}
            </button>

            {!connected && (
              <p style={styles.connectionHint}>
                💡 Connect your wallet to get started
              </p>
            )}

            {connected && (!state.verification?.twitterFollowed || !state.verification?.discordJoined) && (
              <p style={styles.verificationHint}>
                ⚠️ Complete all verification steps to claim
              </p>
            )}
          </div>
        )}
      </div>

      {/* Claim History - Show past claims */}
      {connected && publicKey && (
        <>
          <div style={styles.divider} />
          <ClaimHistory />
        </>
      )}
    </div>
  );
};

/**
 * Verification Tile Component
 */
interface VerificationTileProps {
  icon: string;
  label: string;
  verified: boolean;
  url: string;
  onOpen: () => void;
}

const VerificationTile: React.FC<VerificationTileProps> = ({
  icon,
  label,
  verified,
  url,
  onOpen,
}) => {
  return (
    <div
      style={{
        ...styles.verificationTile,
        ...(verified && styles.verificationTileVerified),
      }}
    >
      <div style={styles.verificationHeader}>
        <span style={styles.icon}>{icon}</span>
        <span style={styles.verificationLabel}>{label}</span>
        <span
          style={{
            ...styles.badge,
            ...(verified ? styles.badgeVerified : styles.badgeUnverified),
          }}
        >
          {verified ? '✓ Verified' : 'Not Verified'}
        </span>
      </div>
      <button onClick={onOpen} style={{ ...styles.button, ...styles.buttonSecondary, width: '100%', marginTop: '0.75rem' }}>
        {verified ? 'Already Joined' : 'Complete'}
      </button>
    </div>
  );
};

/**
 * Styles
 */
const styles = {
  container: {
    width: '100%',
    maxWidth: '600px',
    margin: '0 auto',
    padding: '1.5rem',
  } as React.CSSProperties,

  card: {
    backgroundColor: '#ffffff',
    border: '1px solid #e5e7eb',
    borderRadius: '8px',
    padding: '2rem',
    boxShadow: '0 1px 3px rgba(0,0,0,0.1)',
  } as React.CSSProperties,

  title: {
    fontSize: '1.75rem',
    fontWeight: '700',
    margin: '0 0 0.5rem 0',
    color: '#1f2937',
  } as React.CSSProperties,

  subtitle: {
    fontSize: '1rem',
    color: '#6b7280',
    margin: '0 0 1.5rem 0',
    lineHeight: '1.5',
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

  sectionDescription: {
    fontSize: '0.95rem',
    color: '#4b5563',
    margin: '0 0 0.75rem 0',
  } as React.CSSProperties,

  twitchRow: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    gap: '1rem',
    flexWrap: 'wrap' as const,
  } as React.CSSProperties,

  twitchSubtext: {
    color: '#6b7280',
    fontSize: '0.9rem',
  } as React.CSSProperties,

  twitchButton: {
    backgroundColor: '#7c3aed',
    color: '#fff',
    border: 'none',
    padding: '0.5rem 1rem',
    borderRadius: '9999px',
    fontWeight: 600,
  } as React.CSSProperties,

  secondaryButton: {
    backgroundColor: '#e5e7eb',
    color: '#111827',
    border: 'none',
    padding: '0.5rem 1rem',
    borderRadius: '9999px',
    fontWeight: 600,
  } as React.CSSProperties,

  twitchActions: {
    marginTop: '0.75rem',
  } as React.CSSProperties,


  verificationTile: {
    padding: '1rem',
    backgroundColor: '#f9fafb',
    border: '1px solid #e5e7eb',
    borderRadius: '6px',
    marginBottom: '0.75rem',
    transition: 'all 0.2s',
  } as React.CSSProperties,

  verificationTileVerified: {
    backgroundColor: '#f0fdf4',
    border: '1px solid #86efac',
  } as React.CSSProperties,

  verificationHeader: {
    display: 'flex',
    alignItems: 'center',
    gap: '0.75rem',
  } as React.CSSProperties,

  icon: {
    fontSize: '1.5rem',
  } as React.CSSProperties,

  verificationLabel: {
    fontWeight: '600',
    color: '#1f2937',
    flex: 1,
  } as React.CSSProperties,

  badge: {
    padding: '0.25rem 0.75rem',
    borderRadius: '9999px',
    fontSize: '0.75rem',
    fontWeight: '600',
  } as React.CSSProperties,

  badgeVerified: {
    backgroundColor: '#22c55e',
    color: 'white',
  } as React.CSSProperties,

  badgeUnverified: {
    backgroundColor: '#f3f4f6',
    color: '#6b7280',
  } as React.CSSProperties,

  epochInputGroup: {
    marginBottom: '1rem',
  } as React.CSSProperties,

  label: {
    display: 'block',
    fontSize: '0.9rem',
    fontWeight: '500',
    color: '#374151',
    marginBottom: '0.5rem',
  } as React.CSSProperties,

  input: {
    display: 'block',
    width: '100%',
    padding: '0.75rem',
    border: '1px solid #d1d5db',
    borderRadius: '6px',
    fontSize: '1rem',
    fontFamily: 'monospace',
    boxSizing: 'border-box',
  } as React.CSSProperties,

  hint: {
    fontSize: '0.85rem',
    color: '#6b7280',
    margin: '0.5rem 0 0 0',
  } as React.CSSProperties,

  buttonGroup: {
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
  } as React.CSSProperties,

  buttonPrimary: {
    backgroundColor: '#3b82f6',
    color: 'white',
    width: '100%',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    gap: '0.5rem',
  } as React.CSSProperties,

  buttonSecondary: {
    backgroundColor: '#f3f4f6',
    color: '#374151',
    border: '1px solid #d1d5db',
  } as React.CSSProperties,

  refreshButton: {
    padding: '0.5rem 1rem',
    fontSize: '0.85rem',
    color: '#3b82f6',
    background: 'none',
    border: 'none',
    cursor: 'pointer',
    textDecoration: 'underline',
    marginTop: '1rem',
  } as React.CSSProperties,

  spinner: {
    display: 'inline-block',
    width: '14px',
    height: '14px',
    border: '2px solid rgba(255,255,255,0.3)',
    borderTopColor: 'white',
    borderRadius: '50%',
    animation: 'spin 0.6s linear infinite',
  } as React.CSSProperties,

  connectionHint: {
    fontSize: '0.9rem',
    color: '#6b7280',
    margin: '1rem 0 0 0',
    textAlign: 'center' as const,
  } as React.CSSProperties,

  verificationHint: {
    fontSize: '0.9rem',
    color: '#f59e0b',
    margin: '1rem 0 0 0',
    textAlign: 'center' as const,
  } as React.CSSProperties,

  errorBox: {
    padding: '1rem',
    backgroundColor: '#fee2e2',
    border: '1px solid #fca5a5',
    borderRadius: '6px',
    color: '#991b1b',
    marginBottom: '1.5rem',
  } as React.CSSProperties,

  errorText: {
    fontSize: '0.9rem',
    margin: '0.5rem 0 0 0',
  } as React.CSSProperties,

  successBox: {
    padding: '1.5rem',
    backgroundColor: '#f0fdf4',
    border: '2px solid #22c55e',
    borderRadius: '6px',
    color: '#166534',
    marginBottom: '1.5rem',
  } as React.CSSProperties,

  successTitle: {
    fontSize: '1.2rem',
    fontWeight: '600',
    margin: '0 0 0.5rem 0',
  } as React.CSSProperties,

  successText: {
    fontSize: '0.95rem',
    margin: '0 0 1rem 0',
  } as React.CSSProperties,

  signatureBox: {
    padding: '0.75rem',
    backgroundColor: 'rgba(255,255,255,0.5)',
    borderRadius: '4px',
    marginTop: '0.75rem',
  } as React.CSSProperties,

  signatureLabel: {
    fontSize: '0.8rem',
    fontWeight: '500',
    color: '#6b7280',
    margin: '0 0 0.25rem 0',
  } as React.CSSProperties,

  explorerLink: {
    color: '#166534',
    fontWeight: '600',
    fontSize: '0.9rem',
    fontFamily: 'monospace',
    wordBreak: 'break-all' as const,
  } as React.CSSProperties,

  divider: {
    height: '1px',
    width: '100%',
    backgroundColor: '#e5e7eb',
    margin: '2rem 0',
  } as React.CSSProperties,
};

export default ClaimCLS;
