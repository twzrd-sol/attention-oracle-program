import React, { useState, useEffect } from 'react';
import { useWallet, useConnection } from '@solana/wallet-adapter-react';
import { Transaction } from '@solana/web3.js';
import { requestClaimTransaction, bindWalletWithTwitch, fetchBoundWallet } from '@/lib/api';
import { getExplorerUrl } from '@/lib/solana';
import { PassportBadge } from './PassportBadge';
import { EpochTable } from './EpochTable';
import { ClaimHistory } from './ClaimHistory';
import {
  buildTwitchAuthUrl,
  extractTokenFromHash,
  storeTwitchToken,
  getStoredTwitchToken,
  removeTokenFromUrl,
  clearTwitchToken,
  isValidSolanaAddress,
} from '@/lib/twitch';

interface ClaimState {
  status: 'idle' | 'loading' | 'claiming' | 'confirming' | 'success' | 'error';
  error?: string;
  signature?: string;
}

export const ClaimCLS: React.FC = () => {
  const { publicKey, sendTransaction, connected } = useWallet();
  const { connection } = useConnection();

  const [state, setState] = useState<ClaimState>({ status: 'idle' });
  const [epochId, setEpochId] = useState(0);
  const [twitchToken, setTwitchToken] = useState<string | null>(null);
  const [bindingState, setBindingState] = useState<'idle' | 'checking' | 'binding' | 'bound' | 'error'>('idle');
  const [boundWallet, setBoundWallet] = useState<string | null>(null);
  const [bindingError, setBindingError] = useState<string | null>(null);

  // Twitch OAuth handling
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

  // Check bound wallet when token changes
  useEffect(() => {
    if (!twitchToken) {
      setBoundWallet(null);
      setBindingState('idle');
      return;
    }

    let cancelled = false;
    setBindingState('checking');

    fetchBoundWallet(twitchToken)
      .then((result) => {
        if (cancelled) return;
        const wallet = isValidSolanaAddress(result.wallet) ? result.wallet : null;
        setBoundWallet(wallet);
        setBindingState(wallet ? 'bound' : 'idle');
        setBindingError(null);
      })
      .catch((err) => {
        if (cancelled) return;
        setBindingState('error');
        setBindingError(err instanceof Error ? err.message : 'Failed to load binding');
      });

    return () => { cancelled = true; };
  }, [twitchToken]);

  const handleClaim = async () => {
    if (!publicKey || epochId === 0) return;

    try {
      setState({ status: 'claiming' });
      const { transaction } = await requestClaimTransaction(publicKey.toBase58(), epochId);

      const tx = Transaction.from(Buffer.from(transaction, 'base64'));

      setState({ status: 'confirming' });
      const signature = await sendTransaction(tx, connection);

      await connection.confirmTransaction(signature, 'confirmed');

      setState({ status: 'success', signature });
    } catch (err) {
      setState({
        status: 'error',
        error: err instanceof Error ? err.message : 'Claim failed',
      });
    }
  };

  const handleTwitchConnect = () => window.location.href = buildTwitchAuthUrl();
  const handleTwitchDisconnect = () => {
    clearTwitchToken();
    setTwitchToken(null);
    setBoundWallet(null);
    setBindingState('idle');
  };

  const handleBindWallet = async () => {
    if (!publicKey || !twitchToken) return;
    try {
      setBindingState('binding');
      await bindWalletWithTwitch(twitchToken, publicKey.toBase58());
      setBoundWallet(publicKey.toBase58());
      setBindingState('bound');
    } catch (err) {
      setBindingError(err instanceof Error ? err.message : 'Binding failed');
      setBindingState('error');
    }
  };

  const isWalletBound = boundWallet === publicKey?.toBase58();
  const canClaim = connected && epochId > 0 && isWalletBound && state.status === 'idle';

  return (
    <div className="max-w-2xl mx-auto p-6">
      {/* Passport Badge */}
      {connected && <PassportBadge tier={0} score={0} nextTierScore={10000} />}

      <div className="bg-white rounded-xl border border-gray-200 shadow-sm p-8 mt-8">
        <h2 className="text-3xl font-bold text-gray-900">Claim CLS Tokens</h2>
        <p className="text-gray-600 mt-2">Bind your Twitch account to claim creator rewards</p>

        {/* Twitch Binding */}
        <div className="mt-10">
          <h3 className="text-lg font-semibold text-gray-900">Twitch Identity Binding</h3>
          <div className="mt-4 flex items-center justify-between flex-wrap gap-4">
            <div>
              <span className="font-medium">Status:</span>{' '}
              <span className={twitchToken ? 'text-green-600' : 'text-gray-500'}>
                {twitchToken ? 'Connected' : 'Not Connected'}
              </span>
              {boundWallet && (
                <span className="text-sm text-gray-500 ml-2">
                  — {boundWallet.slice(0, 4)}…{boundWallet.slice(-4)}
                </span>
              )}
            </div>

            {twitchToken ? (
              <button onClick={handleTwitchDisconnect} className="px-4 py-2 bg-gray-200 rounded-full font-medium hover:bg-gray-300 transition">
                Disconnect
              </button>
            ) : (
              <button onClick={handleTwitchConnect} className="px-6 py-3 bg-violet-600 text-white rounded-full font-semibold hover:bg-violet-700 transition">
                Connect Twitch
              </button>
            )}
          </div>

          {twitchToken && (
            <div className="mt-6">
              <button
                onClick={handleBindWallet}
                disabled={bindingState === 'binding' || bindingState === 'checking' || !publicKey}
                className={`px-6 py-3 rounded-full font-semibold transition ${
                  isWalletBound
                    ? 'bg-green-600 text-white'
                    : 'bg-blue-600 text-white hover:bg-blue-700'
                } ${bindingState === 'binding' || bindingState === 'checking' ? 'opacity-60 cursor-not-allowed' : ''}`}
              >
                {bindingState === 'binding' ? 'Binding…' : isWalletBound ? 'Wallet Bound ✓' : 'Bind This Wallet'}
              </button>
              {bindingError && <p className="text-red-600 text-sm mt-3">{bindingError}</p>}
            </div>
          )}
        </div>

        {/* Epoch Selection */}
        {connected && (
          <>
            <div className="mt-10">
              <EpochTable onSelectEpoch={(id) => {
                setEpochId(id);
                document.getElementById('claim-section')?.scrollIntoView({ behavior: 'smooth' });
              }} />
            </div>

            <div className="mt-10" id="claim-section">
              <label className="block text-sm font-medium text-gray-700">Epoch ID</label>
              <input
                type="number"
                value={epochId}
                onChange={(e) => setEpochId(Math.max(0, parseInt(e.target.value) || 0))}
                className="mt-2 w-full px-4 py-3 border border-gray-300 rounded-lg font-mono"
                placeholder="0"
              />
            </div>
          </>
        )}

        {/* Status Messages */}
        {state.status === 'error' && (
          <div className="mt-8 p-4 bg-red-50 border border-red-200 rounded-lg text-red-800">
            <strong>Error:</strong> {state.error}
          </div>
        )}

        {state.status === 'success' && state.signature && (
          <div className="mt-8 p-6 bg-green-50 border-2 border-green-500 rounded-lg text-green-800">
            <div className="text-xl font-bold">✅ Claim Successful!</div>
            <p className="mt-2">Transaction signature:</p>
            <a
              href={getExplorerUrl(state.signature)}
              target="_blank"
              rel="noopener noreferrer"
              className="block mt-2 font-mono text-sm break-all underline"
            >
              {state.signature}
            </a>
          </div>
        )}

        {/* Claim Button */}
        <div className="mt-10">
          <button
            onClick={handleClaim}
            disabled={!canClaim}
            className={`w-full py-4 rounded-xl font-bold text-lg transition ${
              canClaim
                ? 'bg-blue-600 text-white hover:bg-blue-700'
                : 'bg-gray-300 text-gray-500 cursor-not-allowed'
            }`}
          >
            {state.status === 'claiming' || state.status === 'confirming' ? 'Claiming...' : 'Claim CLS Tokens'}
          </button>
          {!connected && <p className="text-center text-gray-500 mt-4">Connect wallet to continue</p>}
        </div>
      </div>

      {/* Claim History */}
      {connected && publicKey && (
        <>
          <div className="h-px bg-gray-200 my-12" />
          <ClaimHistory />
        </>
      )}
    </div>
  );
};

export default ClaimCLS;
