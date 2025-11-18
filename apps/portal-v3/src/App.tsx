import React from 'react';
import { WalletMultiButton } from '@solana/wallet-adapter-react-ui';
import ClaimCLS from './components/ClaimCLS';
import { PasswordProtect } from './components/PasswordProtect';
import { ErrorBoundary } from './components/ErrorBoundary';
import { getClusterName, isMainnet } from './lib/solana';

export const App: React.FC = () => {
  return (
    <PasswordProtect>
      <div className="min-h-screen flex flex-col bg-gray-50">
        <header className="bg-white border-b border-gray-200 shadow-sm">
          <div className="max-w-7xl mx-auto px-6 py-4 flex justify-between items-center">
            <div className="flex flex-col gap-1">
              <h1 className="text-3xl font-extrabold text-gray-900 tracking-tight">TWZRD</h1>
              <p className="text-xs text-gray-500 font-medium">Attention Oracle Portal</p>
            </div>
            <WalletMultiButton />
          </div>
          {!isMainnet() && (
            <div className="bg-amber-200 text-amber-800 py-2 text-center text-sm font-medium">
              ⚠️ {getClusterName()} Network
            </div>
          )}
        </header>

        <main className="flex-1 max-w-7xl mx-auto w-full px-6 py-8">
          <ErrorBoundary>
            <ClaimCLS />
          </ErrorBoundary>
        </main>

        <footer className="bg-white border-t border-gray-200 py-8 mt-auto">
          <div className="max-w-7xl mx-auto text-center">
            <p className="text-sm text-gray-600 mb-2">
              Learn more about{' '}
              <a
                href="https://github.com/twzrd-sol/attention-oracle-program"
                target="_blank"
                rel="noopener noreferrer"
                className="text-blue-600 font-medium hover:underline"
              >
                Attention Oracle
              </a>{' '}
              | Join the{' '}
              <a
                href="https://discord.gg/twzrd"
                target="_blank"
                rel="noopener noreferrer"
                className="text-blue-600 font-medium hover:underline"
              >
                Discord Community
              </a>
            </p>
            <p className="text-xs text-gray-400">
              Running on <strong>{getClusterName()}</strong>
            </p>
          </div>
        </footer>
      </div>
    </PasswordProtect>
  );
};

export default App;
