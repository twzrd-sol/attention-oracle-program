import React, { useState, useEffect } from 'react';
import { useWallet } from '@solana/wallet-adapter-react';
import { getClaimHistory } from '../lib/api';
import type { ClaimRecord } from '../lib/api';

export const ClaimHistory: React.FC = () => {
  const { publicKey } = useWallet();
  const [claims, setClaims] = useState<ClaimRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(0);
  const [total, setTotal] = useState(0);

  const limit = 10;

  useEffect(() => {
    if (!publicKey) {
      setClaims([]);
      setLoading(false);
      return;
    }

    const fetchHistory = async () => {
      try {
        setLoading(true);
        const response = await getClaimHistory(publicKey.toString(), limit, page * limit);
        setClaims(response.claims);
        setTotal(response.total);
        setError(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load history');
        setClaims([]);
      } finally {
        setLoading(false);
      }
    };

    fetchHistory();
  }, [publicKey, page]);

  if (!publicKey) {
    return null;
  }

  const totalPages = Math.ceil(total / limit);

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'confirmed':
        return 'bg-emerald-100 text-emerald-800 border-emerald-300';
      case 'pending':
        return 'bg-amber-100 text-amber-800 border-amber-300';
      case 'failed':
        return 'bg-red-100 text-red-800 border-red-300';
      default:
        return 'bg-gray-100 text-gray-800 border-gray-300';
    }
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

  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr);
    return date.toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: date.getFullYear() !== new Date().getFullYear() ? '2-digit' : undefined,
    });
  };

  if (error) {
    return (
      <div className="mt-12">
        <h2 className="text-2xl font-bold text-gray-900 mb-6">Your Claims</h2>
        <div className="p-6 bg-red-50 border border-red-200 rounded-xl text-red-800">
          {error}
        </div>
      </div>
    );
  }

  return (
    <div className="mt-12">
      <h2 className="text-2xl font-bold text-gray-900 mb-8">
        Your Claims {total > 0 && `(${total} total)`}
      </h2>

      {loading ? (
        <div className="text-center py-12 text-gray-600">⏳ Loading your claim history...</div>
      ) : claims.length === 0 ? (
        <div className="text-center py-12 text-gray-600 text-lg">
          📝 No claims yet. Claim your first tokens above!
        </div>
      ) : (
        <>
          <div className="overflow-x-auto rounded-xl border border-gray-200 shadow-sm">
            <table className="w-full">
              <thead className="bg-gray-50">
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-semibold text-gray-900">Epoch</th>
                  <th className="px-6 py-4 text-left text-sm font-semibold text-gray-900">Amount</th>
                  <th className="px-6 py-4 text-left text-sm font-semibold text-gray-900">Status</th>
                  <th className="px-6 py-4 text-left text-sm font-semibold text-gray-900">Date</th>
                  <th className="px-6 py-4 text-left text-sm font-semibold text-gray-900">Transaction</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-200 bg-white">
                {claims.map((claim, idx) => (
                  <tr key={idx} className="hover:bg-gray-50 transition">
                    <td className="px-6 py-4 font-medium">#{claim.epoch_id}</td>
                    <td className="px-6 py-4">{Number(claim.amount).toLocaleString()} CCM</td>
                    <td className="px-6 py-4">
                      <span className={`inline-block px-3 py-1 rounded-full text-sm font-medium border ${getStatusColor(claim.status)}`}>
                        {getStatusLabel(claim.status)}
                      </span>
                    </td>
                    <td className="px-6 py-4">{formatDate(claim.claimed_at)}</td>
                    <td className="px-6 py-4">
                      {claim.tx_signature ? (
                        <a
                          href={`https://solscan.io/tx/${claim.tx_signature}`}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-blue-600 font-medium hover:underline"
                        >
                          View on Solscan →
                        </a>
                      ) : (
                        <span className="text-gray-400">—</span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {totalPages > 1 && (
            <div className="flex justify-center items-center gap-4 mt-10 flex-wrap">
              <button
                onClick={() => setPage(p => Math.max(0, p - 1))}
                disabled={page === 0}
                className="px-5 py-2.5 bg-blue-600 text-white rounded-lg font-medium disabled:opacity-50 disabled:cursor-not-allowed hover:bg-blue-700 transition"
              >
                ← Previous
              </button>
              <span className="text-gray-600">
                Page {page + 1} of {totalPages}
              </span>
              <button
                onClick={() => setPage(p => Math.min(totalPages - 1, p + 1))}
                disabled={page >= totalPages - 1}
                className="px-5 py-2.5 bg-blue-600 text-white rounded-lg font-medium disabled:opacity-50 disabled:cursor-not-allowed hover:bg-blue-700 transition"
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
