import { useCallback, useState } from 'react';
import { useWallet as useSolanaWallet } from '@solana/wallet-adapter-react';
import { PublicKey } from '@solana/web3.js';

export interface WalletInfo {
  address: PublicKey | null;
  shortAddress: string | null;
  connected: boolean;
  connecting: boolean;
  disconnecting: boolean;
}

export const useWallet = () => {
  const { publicKey, connected, connecting, disconnecting, select, disconnect } = useSolanaWallet();
  const [error, setError] = useState<string | null>(null);

  const shortAddress = publicKey ? publicKey.toBase58().slice(0, 8) + '...' + publicKey.toBase58().slice(-6) : null;

  const connect = useCallback(async (walletName?: string) => {
    try {
      setError(null);
      if (walletName) {
        select(walletName);
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(`Connection failed: ${message}`);
      console.error('Wallet connection error:', err);
    }
  }, [select]);

  const disconnectWallet = useCallback(async () => {
    try {
      setError(null);
      await disconnect();
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(`Disconnection failed: ${message}`);
      console.error('Wallet disconnection error:', err);
    }
  }, [disconnect]);

  const walletInfo: WalletInfo = {
    address: publicKey,
    shortAddress,
    connected,
    connecting,
    disconnecting,
  };

  return {
    ...walletInfo,
    connect,
    disconnect: disconnectWallet,
    error,
    isReady: connected && !connecting && !disconnecting,
  };
};

export default useWallet;
