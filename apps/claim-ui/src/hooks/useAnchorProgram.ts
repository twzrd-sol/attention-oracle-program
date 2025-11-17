import { useEffect, useState, useCallback } from 'react';
import { AnchorProvider, Program } from '@coral-xyz/anchor';
import { Connection, PublicKey } from '@solana/web3.js';
import { useWallet } from '@solana/wallet-adapter-react';
import IDL from '../../idl/token-2022.json';

// Type for the program (without full IDL typing for simplicity)
export interface Token2022Program {
  programId: PublicKey;
  methods: any;
  account: any;
}

export const useAnchorProgram = () => {
  const { wallet, publicKey, connected } = useWallet();
  const [program, setProgram] = useState<Token2022Program | null>(null);
  const [provider, setProvider] = useState<AnchorProvider | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const initializeProgram = useCallback(async () => {
    if (!wallet || !publicKey || !connected) {
      setError('Wallet not connected');
      return;
    }

    try {
      setLoading(true);
      setError(null);

      const rpcUrl = import.meta.env.VITE_SOLANA_RPC || 'https://api.mainnet-beta.solana.com';
      const connection = new Connection(rpcUrl, 'confirmed');

      // Create provider with wallet adapter
      const provider = new AnchorProvider(
        connection,
        wallet.adapter,
        { commitment: 'confirmed' }
      );

      // Create program from IDL
      const programId = new PublicKey(
        import.meta.env.VITE_PROGRAM_ID || 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop'
      );

      const program = new Program(IDL as any, programId, provider);

      setProvider(provider);
      setProgram(program as unknown as Token2022Program);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(`Failed to initialize program: ${message}`);
      console.error('Program initialization error:', err);
    } finally {
      setLoading(false);
    }
  }, [wallet, publicKey, connected]);

  // Re-initialize when wallet connection changes
  useEffect(() => {
    if (connected && wallet) {
      initializeProgram();
    }
  }, [connected, wallet, initializeProgram]);

  return {
    program,
    provider,
    loading,
    error,
    isReady: !!program && !!provider,
    refresh: initializeProgram,
  };
};

export default useAnchorProgram;
