import React, { useMemo } from 'react';
import {
  ConnectionProvider,
  WalletProvider as SolanaWalletProvider,
} from '@solana/wallet-adapter-react';
import { WalletModalProvider } from '@solana/wallet-adapter-react-ui';
import {
  PhantomWalletAdapter,
  SolflareWalletAdapter,
  TorusWalletAdapter,
} from '@solana/wallet-adapter-wallets';
import '@solana/wallet-adapter-react-ui/styles.css';

interface Props {
  children: React.ReactNode;
}

export const WalletProvider: React.FC<Props> = ({ children }) => {
  const network = (import.meta.env.VITE_SOLANA_NETWORK || 'mainnet-beta') as 'mainnet-beta' | 'devnet' | 'testnet';
  const endpoint = import.meta.env.VITE_SOLANA_RPC || 'https://api.mainnet-beta.solana.com';

  const wallets = useMemo(
    () => [
      new PhantomWalletAdapter(),
      new SolflareWalletAdapter(),
      new TorusWalletAdapter(),
    ],
    []
  );

  return (
    <ConnectionProvider endpoint={endpoint}>
      <SolanaWalletProvider wallets={wallets} autoConnect>
        <WalletModalProvider>{children}</WalletModalProvider>
      </SolanaWalletProvider>
    </ConnectionProvider>
  );
};

export default WalletProvider;
