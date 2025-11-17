import React from 'react';
import ReactDOM from 'react-dom/client';
import {
  ConnectionProvider,
  WalletProvider,
} from '@solana/wallet-adapter-react';
import { WalletModalProvider } from '@solana/wallet-adapter-react-ui';
import { WalletAdapterNetwork } from '@solana/wallet-adapter-base';
import {
  PhantomWalletAdapter,
  SolflareWalletAdapter,
  TorusWalletAdapter,
} from '@solana/wallet-adapter-wallets';
import { clusterApiUrl } from '@solana/web3.js';
import '@solana/wallet-adapter-react-ui/styles.css';

import App from './App';
import './index.css';

// Network configuration
const network = (process.env.VITE_SOLANA_NETWORK || 'mainnet-beta') as WalletAdapterNetwork;
const endpoint = process.env.VITE_SOLANA_RPC || clusterApiUrl(network);

// Wallet adapters
const wallets = [
  new PhantomWalletAdapter(),
  new SolflareWalletAdapter({ network }),
  new TorusWalletAdapter(),
];

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <ConnectionProvider endpoint={endpoint}>
      <WalletProvider wallets={wallets} autoConnect>
        <WalletModalProvider>
          <App />
        </WalletModalProvider>
      </WalletProvider>
    </ConnectionProvider>
  </React.StrictMode>
);
