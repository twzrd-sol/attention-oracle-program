'use client';

import React, { useState } from 'react';

export default function Home() {
  const [creator, setCreator] = useState('');
  const [loading, setLoading] = useState(false);
  const [response, setResponse] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);
  const [paymentSimulated, setPaymentSimulated] = useState(false);
  const [snapshot, setSnapshot] = useState<any>(null);
  const [snapshotErr, setSnapshotErr] = useState<string | null>(null);

  const fetchSnapshot = async () => {
    try {
      setSnapshotErr(null);
      const res = await fetch('/api/ops/summary', { cache: 'no-store' });
      const data = await res.json();
      setSnapshot(data);
      if (!data.ok) setSnapshotErr(data.error || 'Snapshot unavailable');
    } catch (e: any) {
      setSnapshotErr(e?.message || 'Snapshot error');
    }
  };

  // auto-load snapshot on mount
  React.useEffect(() => {
    fetchSnapshot();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const fetchAttentionScore = async (withPayment: boolean = false) => {
    setLoading(true);
    setError(null);
    setResponse(null);

    try {
      const headers: HeadersInit = {};

      if (withPayment) {
        // Simulate x402 payment header
        headers['Authorization'] = 'Bearer x402-mock-payment-token';
        headers['X-402-Payment'] = 'mock-payment-proof';
      }

      const res = await fetch(`/api/get-attention-score?creator=${encodeURIComponent(creator || 'anonymous')}`, {
        headers
      });

      const data = await res.json();

      if (!res.ok) {
        if (res.status === 402) {
          setError('Payment required - Click "Pay with x402" to access data');
        } else {
          setError(data.error || 'Request failed');
        }
      }

      setResponse(data);
    } catch (err) {
      setError('Network error: ' + (err instanceof Error ? err.message : 'Unknown error'));
    } finally {
      setLoading(false);
    }
  };

  const simulatePayment = async () => {
    setLoading(true);
    setError(null);

    // Simulate payment processing
    await new Promise(resolve => setTimeout(resolve, 1500));

    setPaymentSimulated(true);
    setLoading(false);

    // After payment, fetch with payment proof
    await fetchAttentionScore(true);
  };

  return (
    <div className="min-h-screen bg-gradient-to-b from-gray-900 to-gray-800 text-white p-8">
      <main className="max-w-4xl mx-auto">
        {/* System Snapshot */}
        <div className="bg-gray-800 rounded-lg p-6 mb-8 shadow">        
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-2xl font-semibold">System Snapshot</h2>
            <button onClick={fetchSnapshot} className="px-3 py-1 text-sm bg-gray-700 rounded hover:bg-gray-600">Refresh</button>
          </div>
          {!snapshot && !snapshotErr && (
            <p className="text-gray-400">Loading snapshot…</p>
          )}
          {snapshotErr && (
            <p className="text-red-400">{snapshotErr}</p>
          )}
          {snapshot?.ok && (
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
              <div className="bg-gray-900/60 p-3 rounded">
                <div className="text-gray-400">Latest Sealed</div>
                <div className="text-white font-semibold">{snapshot.sealed?.latest_sealed_at_ts || '—'}</div>
              </div>
              <div className="bg-gray-900/60 p-3 rounded">
                <div className="text-gray-400">Sealed (24h)</div>
                <div className="text-white font-semibold">{snapshot.sealed?.sealed_24h || '—'}</div>
              </div>
              <div className="bg-gray-900/60 p-3 rounded">
                <div className="text-gray-400">Events (1h)</div>
                <div className="text-white font-semibold">{snapshot.events?.events_last_hour || '—'}</div>
              </div>
              <div className="bg-gray-900/60 p-3 rounded">
                <div className="text-gray-400">Channels (24h)</div>
                <div className="text-white font-semibold">{snapshot.channels_24h?.channels_24h || '—'}</div>
              </div>
              <div className="bg-gray-900/60 p-3 rounded">
                <div className="text-gray-400">Participants</div>
                <div className="text-white font-semibold">{snapshot.participants?.sp_total || '—'}</div>
              </div>
              <div className="bg-gray-900/60 p-3 rounded">
                <div className="text-gray-400">Missing Usernames</div>
                <div className="text-white font-semibold">{snapshot.participants?.sp_null || '—'}</div>
              </div>
              <div className="bg-gray-900/60 p-3 rounded col-span-2">
                <div className="text-gray-400">Latest Epoch</div>
                <div className="text-white font-semibold">{snapshot.sealed?.latest_epoch || '—'}</div>
              </div>
            </div>
          )}
        </div>
        <h1 className="text-5xl font-bold mb-4 bg-gradient-to-r from-purple-400 to-pink-600 bg-clip-text text-transparent">
          Attention Oracle x402 Demo
        </h1>

        <p className="text-gray-300 mb-8">
          Verifiable Distribution Protocol - Token-2022 Program with x402 Payment Integration
        </p>

        <div className="bg-gray-800 rounded-lg p-6 mb-8">
          <h2 className="text-2xl font-semibold mb-4">How it Works</h2>
          <ol className="list-decimal list-inside space-y-2 text-gray-300">
            <li>Off-chain oracle aggregates streaming engagement data</li>
            <li>Merkle root is committed on-chain (Solana)</li>
            <li>AI agents access data via x402-protected API</li>
            <li>Viewers claim tokens with cryptographic proofs</li>
          </ol>
        </div>

        {/* x402 Demo */}
        <div className="bg-gray-800 rounded-lg p-6 mb-6 shadow">
          <h3 className="text-xl font-semibold mb-4">Try the API</h3>

          <div className="mb-4">
            <label className="block text-sm font-medium mb-2">Creator Name</label>
            <input
              type="text"
              value={creator}
              onChange={(e) => setCreator(e.target.value)}
              placeholder="Enter creator name"
              className="w-full p-3 rounded bg-gray-700 text-white border border-gray-600 focus:border-purple-500 focus:outline-none"
            />
          </div>

          <div className="flex gap-4">
            <button
              onClick={() => fetchAttentionScore(false)}
              disabled={loading}
              className="px-6 py-3 bg-gray-600 hover:bg-gray-500 disabled:bg-gray-700 disabled:cursor-not-allowed rounded font-semibold transition"
            >
              {loading ? 'Loading...' : 'Try Without Payment'}
            </button>

            <button
              onClick={simulatePayment}
              disabled={loading}
              className="px-6 py-3 bg-gradient-to-r from-purple-500 to-pink-600 hover:from-purple-600 hover:to-pink-700 disabled:from-gray-600 disabled:to-gray-700 disabled:cursor-not-allowed rounded font-semibold transition"
            >
              {loading ? 'Processing...' : paymentSimulated ? 'Access Granted ✓' : 'Pay with x402'}
            </button>
          </div>
        </div>

        {error && (
          <div className="bg-red-900/50 border border-red-600 rounded-lg p-4 mb-6">
            <p className="text-red-200">{error}</p>
          </div>
        )}

        {response && (
          <div className="bg-gray-800 rounded-lg p-6">
            <h3 className="text-xl font-semibold mb-4">
              {response.status === 'success' ? '✅ Data Retrieved' : '🔒 Payment Required'}
            </h3>
            <pre className="bg-gray-900 p-4 rounded overflow-x-auto text-sm">
              {JSON.stringify(response, null, 2)}
            </pre>
          </div>
        )}

        {/* API Quickstart & Program Details */}
        <div className="mt-12 grid md:grid-cols-2 gap-6">
          <div className="bg-gray-800 rounded-lg p-6 shadow">
            <h3 className="text-lg font-semibold mb-2">API Quickstart (curl)</h3>
            <pre className="bg-gray-900 p-4 rounded overflow-x-auto text-sm text-gray-200">
{`# 1) 402 challenge
curl -s "http://localhost:3000/api/get-attention-score?creator=example_user" | jq

# 2) Simulate payment
curl -s -H "Authorization: Bearer x402-mock-payment-token" \\
     -H "X-402-Payment: mock-proof" \\
     "http://localhost:3000/api/get-attention-score?creator=example_user" | jq

# 3) Switchboard (when fresh)
curl -s "http://localhost:3000/api/switchboard/price" | jq

# Health
curl -s "http://localhost:3000/api/healthz" | jq`}
            </pre>
          </div>
          <div className="bg-gray-800 rounded-lg p-6 shadow">
            <h3 className="text-lg font-semibold mb-2">Program Details</h3>
            <div className="text-sm text-gray-400 space-y-1">
              <p>On-chain Program: <code className="bg-gray-700 px-2 py-1 rounded">GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop</code></p>
              <p>Network: Solana Mainnet</p>
              <p>Standard: Token-2022 (Transfer Fees)</p>
              <p>GitHub: <a href="https://github.com/twzrd-sol/attention-oracle-program" className="text-purple-400 hover:underline">twzrd-sol/attention-oracle-program</a></p>
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}
