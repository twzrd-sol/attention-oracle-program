import { useCallback, useState } from 'react';
import { PublicKey } from '@solana/web3.js';

export interface MerkleProof {
  claimer: string;
  mint: string;
  channel: string;
  epoch: number;
  index: number;
  amount: string;
  id: string;
  root: string;
  proof: string[];
}

export interface ProofValidation {
  valid: boolean;
  errors: string[];
}

export const useMerkleProof = () => {
  const [proof, setProof] = useState<MerkleProof | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const validateProofStructure = useCallback((data: any): ProofValidation => {
    const errors: string[] = [];

    // Check required fields
    if (!data.claimer) errors.push('Missing claimer address');
    if (!data.mint) errors.push('Missing mint address');
    if (!data.channel) errors.push('Missing channel name');
    if (data.epoch === undefined || data.epoch === null) errors.push('Missing epoch number');
    if (data.index === undefined || data.index === null) errors.push('Missing index');
    if (!data.amount) errors.push('Missing amount');
    if (!data.id) errors.push('Missing claim ID');
    if (!data.root) errors.push('Missing merkle root');
    if (!Array.isArray(data.proof) || data.proof.length === 0) errors.push('Missing or invalid merkle proof');

    // Validate field formats
    if (data.claimer && !isValidPublicKey(data.claimer)) {
      errors.push('Invalid claimer public key format');
    }
    if (data.mint && !isValidPublicKey(data.mint)) {
      errors.push('Invalid mint public key format');
    }
    if (data.epoch && typeof data.epoch !== 'number') {
      errors.push('Epoch must be a number');
    }
    if (data.index && typeof data.index !== 'number') {
      errors.push('Index must be a number');
    }
    if (data.amount) {
      try {
        BigInt(data.amount);
      } catch {
        errors.push('Amount must be a valid number');
      }
    }
    if (data.root && !isValidHex(data.root)) {
      errors.push('Root must be valid hex');
    }
    if (Array.isArray(data.proof)) {
      for (let i = 0; i < data.proof.length; i++) {
        if (!isValidHex(data.proof[i])) {
          errors.push(`Proof node ${i} is not valid hex`);
          break;
        }
      }
    }

    return {
      valid: errors.length === 0,
      errors,
    };
  }, []);

  const parseProofJSON = useCallback((jsonString: string): ProofValidation => {
    try {
      const data = JSON.parse(jsonString);
      return validateProofStructure(data);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      return {
        valid: false,
        errors: [`Invalid JSON: ${msg}`],
      };
    }
  }, [validateProofStructure]);

  const loadProofFromFile = useCallback(async (file: File): Promise<void> => {
    setLoading(true);
    setError(null);

    try {
      const content = await file.text();
      const validation = parseProofJSON(content);

      if (!validation.valid) {
        setError(validation.errors.join('; '));
        setProof(null);
        return;
      }

      const data = JSON.parse(content) as MerkleProof;
      setProof(data);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(`Failed to load proof: ${msg}`);
      setProof(null);
    } finally {
      setLoading(false);
    }
  }, [parseProofJSON]);

  const loadProofFromJSON = useCallback((jsonString: string): void => {
    setLoading(true);
    setError(null);

    try {
      const validation = parseProofJSON(jsonString);

      if (!validation.valid) {
        setError(validation.errors.join('; '));
        setProof(null);
        setLoading(false);
        return;
      }

      const data = JSON.parse(jsonString) as MerkleProof;
      setProof(data);
      setError(null);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(`Failed to parse proof: ${msg}`);
      setProof(null);
    } finally {
      setLoading(false);
    }
  }, [parseProofJSON]);

  const clearProof = useCallback(() => {
    setProof(null);
    setError(null);
  }, []);

  const getProofBytes = useCallback((): Uint8Array[] => {
    if (!proof) return [];
    return proof.proof.map(hex => new Uint8Array(Buffer.from(hex.replace('0x', ''), 'hex')));
  }, [proof]);

  const getSummary = useCallback(() => {
    if (!proof) return null;
    return {
      channel: proof.channel,
      epoch: proof.epoch,
      claimer: proof.claimer,
      amount: proof.amount,
      proofDepth: proof.proof.length,
    };
  }, [proof]);

  return {
    proof,
    loading,
    error,
    loadProofFromFile,
    loadProofFromJSON,
    clearProof,
    getProofBytes,
    getSummary,
    isLoaded: !!proof,
  };
};

// Helper functions
function isValidPublicKey(key: string): boolean {
  try {
    new PublicKey(key);
    return true;
  } catch {
    return false;
  }
}

function isValidHex(hex: string): boolean {
  const hexString = hex.replace('0x', '');
  return /^[0-9a-fA-F]*$/.test(hexString) && hexString.length % 2 === 0;
}

export default useMerkleProof;
