import React, { useState, useEffect } from 'react';
import { getEpochs } from '../lib/api';
import type { Epoch } from '../lib/api';

interface EpochTableProps {
  onSelectEpoch?: (epochId: number) => void;
}

export const EpochTable: React.FC<EpochTableProps> = ({ onSelectEpoch }) => {
  const [epochs, setEpochs] = useState<Epoch[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(0);
  const [total, setTotal] = useState(0);

  const limit = 10;

  useEffect(() => {
    const fetchEpochs = async () => {
      try {
        setLoading(true);
        const response = await getEpochs(limit, page * limit);
        setEpochs(response.epochs);
        setTotal(response.total);
        setError(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load epochs');
        setEpochs([]);
      } finally {
        setLoading(false);
      }
    };

    fetchEpochs();
  }, [page]);

  const totalPages = Math.ceil(total / limit);

  if (error) {
    return (
      <div className="mt-12">
        <h2 className="text-2xl font-bold text-gray-900 mb-6">Available Epochs</h2>
        <div className="p-6 bg-red-50 border border-red-200 rounded-xl text-red-800">
          {error}
        </div>
      </div>
    );
  }

  return (
    <div className="mt-12">
      <h2 className="text-2xl font-bold text-gray-900 mb-8">Available Epochs</h2>

      {loading ? (
        <div className="text-center py-12 text-gray-600">Loading epochs...</div>
      ) : epochs.length === 0 ? (
        <div className="text-center py-12 text-gray-600 text-lg">
          No epochs available yet. Check back soon!
        </div>
      ) : (
        <>
          <div className="overflow-x-auto rounded-xl border border-gray-200 shadow-sm">
            <table className="w-full">
              <thead className="bg-gray-50">
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-semibold text-gray-900">Epoch ID</th>
                  <th className="px-6 py-4 text-left text-sm font-semibold text-gray-900">Merkle Root</th>
                  <th className="px-6 py-4 text-left text-sm font-semibold text-gray-900">Status</th>
                  <th className="px-6 py-4 text-left text-sm font-semibold text-gray-900">Claimers</th>
                  <th className="px-6 py-4 text-left text-sm font-semibold text-gray-900">Total Amount</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-200 bg-white">
                {epochs.map((epoch) => (
                  <tr
                    key={epoch.epoch_id}
                    onClick={() => onSelectEpoch?.(epoch.epoch_id)}
                    className="hover:bg-gray-50 cursor-pointer transition"
                  >
                    <td className="px-6 py-4 font-medium">#{epoch.epoch_id}</td>
                    <td className="px-6 py-4">
                      <code className="text-sm bg-gray-100 px-2 py-1 rounded font-mono">
                        {epoch.merkle_root.substring(0, 10)}...
                      </code>
                    </td>
                    <td className="px-6 py-4">
                      <span className={`inline-block px-3 py-1 rounded-full text-sm font-medium ${
                        epoch.is_open
                          ? 'bg-emerald-100 text-emerald-800'
                          : 'bg-gray-200 text-gray-600'
                      }`}>
                        {epoch.is_open ? '✓ Open' : '✗ Closed'}
                      </span>
                    </td>
                    <td className="px-6 py-4">{epoch.total_claimers.toLocaleString()}</td>
                    <td className="px-6 py-4">{Number(epoch.total_amount).toLocaleString()} CCM</td>
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
                Page {page + 1} of {totalPages} ({total} total)
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

export default EpochTable;
