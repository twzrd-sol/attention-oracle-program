import { useEffect, useState } from 'react';
import {
  Connection,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from '@solana/web3.js';
import { getAssociatedTokenAddressSync, TOKEN_2022_PROGRAM_ID, getAccount } from '@solana/spl-token';
import { keccak_256 } from 'js-sha3';
import './App.css';

// CLS Program ID (mainnet deployment)
const PROGRAM_ID = new PublicKey(import.meta.env.VITE_PROGRAM_ID || 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const RPC_URL = import.meta.env.VITE_SOLANA_RPC || 'https://api.mainnet-beta.solana.com';
const ASSOCIATED_TOKEN_PROGRAM_ID = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');

type ClaimPhase = 'idle' | 'connecting' | 'claiming' | 'claimed' | 'error';

type ClaimStep = 'proof' | 'wallet' | 'review' | 'claim';

interface ClaimProof {
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

const TRANSFER_FEE_BPS = 100; // 1% transfer fee

async function discriminator(name: string): Promise<Buffer> {
  const data = new TextEncoder().encode(`global:${name}`);
  const hash = await crypto.subtle.digest('SHA-256', data);
  return Buffer.from(hash).slice(0, 8);
}

function deriveStreamerKey(channel: string): PublicKey {
  const lower = channel.toLowerCase();
  const hash = keccak_256.update('channel:').update(lower).digest();
  return new PublicKey(Buffer.from(hash));
}

function serializeClaimWithRing(args: {
  epoch: bigint;
  index: number;
  amount: bigint;
  proof: Uint8Array[];
  id: string;
  streamer_key: Uint8Array;
}): Buffer {
  const buffers: Buffer[] = [];

  const epochBuf = Buffer.alloc(8);
  epochBuf.writeBigUInt64LE(args.epoch);
  buffers.push(epochBuf);

  const indexBuf = Buffer.alloc(4);
  indexBuf.writeUInt32LE(args.index);
  buffers.push(indexBuf);

  const amountBuf = Buffer.alloc(8);
  amountBuf.writeBigUInt64LE(args.amount);
  buffers.push(amountBuf);

  const proofLenBuf = Buffer.alloc(4);
  proofLenBuf.writeUInt32LE(args.proof.length);
  buffers.push(proofLenBuf);
  args.proof.forEach((node) => buffers.push(Buffer.from(node)));

  const idBytes = Buffer.from(args.id, 'utf8');
  const idLenBuf = Buffer.alloc(4);
  idLenBuf.writeUInt32LE(idBytes.length);
  buffers.push(idLenBuf);
  buffers.push(idBytes);

  buffers.push(Buffer.from(args.streamer_key));

  return Buffer.concat(buffers);
}

function parseProofJSON(json: string): ClaimProof {
  const data = JSON.parse(json);
  if (!data.claimer || !data.mint || !data.channel || data.epoch === undefined || data.index === undefined || !data.amount || !data.id || !data.root || !Array.isArray(data.proof)) {
    throw new Error('Invalid proof JSON: missing required fields');
  }
  return data;
}

function calculateNetAmount(grossAmount: string): { gross: bigint; fee: bigint; net: bigint } {
  const gross = BigInt(grossAmount);
  const fee = (gross * BigInt(TRANSFER_FEE_BPS)) / BigInt(10000);
  const net = gross - fee;
  return { gross, fee, net };
}

function formatTokenAmount(amount: bigint): string {
  return amount.toLocaleString();
}

function mapClaimError(error: any): string {
  const msg = error?.message || String(error);
  if (msg.includes('AlreadyClaimed')) {
    return 'You have already claimed tokens for this epoch.';
  }
  if (msg.includes('InvalidProof')) {
    return 'The merkle proof is invalid. Please check your proof file.';
  }
  if (msg.includes('user rejected')) {
    return 'Transaction was cancelled.';
  }
  if (msg.includes('insufficient funds')) {
    return 'Insufficient SOL for transaction fees.';
  }
  return `Transaction failed: ${msg.slice(0, 100)}`;
}

export default function ClaimCLS() {
  const [proofInput, setProofInput] = useState<string>('');
  const [claimData, setClaimData] = useState<ClaimProof | null>(null);
  const [walletAddress, setWalletAddress] = useState<string | null>(null);
  const [status, setStatus] = useState<string>('');
  const [claimPhase, setClaimPhase] = useState<ClaimPhase>('idle');
  const [balanceBefore, setBalanceBefore] = useState<string>('—');
  const [balanceAfter, setBalanceAfter] = useState<string>('—');
  const [txSignature, setTxSignature] = useState<string>('');
  const [currentStep, setCurrentStep] = useState<ClaimStep>('proof');

  // Auto-connect wallet on mount
  useEffect(() => {
    const connectWallet = async () => {
      if (!window.solana) {
        setStatus('Phantom wallet not detected. Please install it.');
        return;
      }
      if (window.solana.isConnected) {
        try {
          const resp = await window.solana.connect();
          setWalletAddress(resp.publicKey.toString());
        } catch (err) {
          console.log('Wallet connection skipped');
        }
      }
    };
    connectWallet();
  }, []);

  const handleProofUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (ev) => {
      try {
        const content = ev.target?.result as string;
        setProofInput(content);
        const parsed = parseProofJSON(content);
        setClaimData(parsed);
        setStatus('✅ Proof loaded successfully');
        setCurrentStep('wallet');
        setBalanceBefore('—');
        setBalanceAfter('—');
        setTxSignature('');
        setClaimPhase('idle');
      } catch (error: any) {
        setStatus(`❌ Error parsing proof: ${error.message}`);
        setClaimData(null);
        setCurrentStep('proof');
      }
    };
    reader.readAsText(file);
  };

  const handleProofPaste = () => {
    try {
      const parsed = parseProofJSON(proofInput);
      setClaimData(parsed);
      setStatus('✅ Proof loaded successfully');
      setCurrentStep('wallet');
      setBalanceBefore('—');
      setBalanceAfter('—');
      setTxSignature('');
      setClaimPhase('idle');
    } catch (error: any) {
      setStatus(`❌ Error parsing proof: ${error.message}`);
      setClaimData(null);
      setCurrentStep('proof');
    }
  };

  const connectWallet = async () => {
    if (!window.solana) {
      setStatus('❌ Install Phantom wallet to claim.');
      return;
    }
    try {
      setClaimPhase('connecting');
      const resp = await window.solana.connect();
      const connectedAddress = resp.publicKey.toString();
      setWalletAddress(connectedAddress);

      // Check if wallet matches proof
      if (claimData && claimData.claimer !== connectedAddress) {
        setStatus(`⚠️ Warning: Proof is for ${claimData.claimer.slice(0, 8)}... but you connected ${connectedAddress.slice(0, 8)}...`);
        setClaimPhase('error');
      } else {
        setStatus('✅ Wallet connected');
        setCurrentStep('review');
        setClaimPhase('idle');
      }
    } catch (error: any) {
      setStatus('❌ Wallet connection cancelled.');
      setClaimPhase('idle');
    }
  };

  const claim = async () => {
    if (!walletAddress || !claimData) {
      setStatus('❌ Connect wallet and load proof first.');
      return;
    }

    try {
      setClaimPhase('claiming');
      setCurrentStep('claim');
      setStatus('⏳ Checking balance…');

      const connection = new Connection(RPC_URL, 'confirmed');
      const claimerPubkey = new PublicKey(claimData.claimer);
      const mintPubkey = new PublicKey(claimData.mint);
      const streamerKey = deriveStreamerKey(claimData.channel);
      const epoch = BigInt(claimData.epoch);
      const claimIndex = claimData.index;
      const claimAmount = BigInt(claimData.amount);
      const claimId = claimData.id;
      const proofNodes: Uint8Array[] = claimData.proof.map((hex: string) =>
        Buffer.from(hex.replace('0x', ''), 'hex')
      );

      // Verify wallet matches
      if (claimerPubkey.toBase58() !== walletAddress) {
        setStatus(`❌ Wallet mismatch: Proof is for ${claimerPubkey.toBase58().slice(0, 8)}..., but you're using ${walletAddress.slice(0, 8)}...`);
        setClaimPhase('error');
        return;
      }

      // Derive PDAs
      const [protocolPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('protocol'), mintPubkey.toBuffer()],
        PROGRAM_ID
      );

      const [channelPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('channel_state'), mintPubkey.toBuffer(), streamerKey.toBuffer()],
        PROGRAM_ID
      );

      const treasuryAta = getAssociatedTokenAddressSync(mintPubkey, protocolPda, true, TOKEN_2022_PROGRAM_ID);
      const claimerAta = getAssociatedTokenAddressSync(mintPubkey, claimerPubkey, false, TOKEN_2022_PROGRAM_ID);

      // Fetch balance before
      let beforeBalance = BigInt(0);
      try {
        const acct = await getAccount(connection, claimerAta, 'confirmed', TOKEN_2022_PROGRAM_ID);
        beforeBalance = acct.amount;
        setBalanceBefore(beforeBalance.toString());
      } catch (_) {
        setBalanceBefore('0 (new ATA)');
      }

      setStatus('⏳ Constructing transaction…');

      // Construct instruction
      const serializedArgs = serializeClaimWithRing({
        epoch,
        index: claimIndex,
        amount: claimAmount,
        proof: proofNodes,
        id: claimId,
        streamer_key: streamerKey.toBytes(),
      });

      const DISC_CLAIM_WITH_RING = await discriminator('claim_with_ring');
      const instructionData = Buffer.concat([DISC_CLAIM_WITH_RING, serializedArgs]);

      const claimIx = new TransactionInstruction({
        programId: PROGRAM_ID,
        keys: [
          { pubkey: claimerPubkey, isSigner: true, isWritable: true },
          { pubkey: protocolPda, isSigner: false, isWritable: true },
          { pubkey: channelPda, isSigner: false, isWritable: true },
          { pubkey: mintPubkey, isSigner: false, isWritable: false },
          { pubkey: treasuryAta, isSigner: false, isWritable: true },
          { pubkey: claimerAta, isSigner: false, isWritable: true },
          { pubkey: TOKEN_2022_PROGRAM_ID, isSigner: false, isWritable: false },
          { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        ],
        data: instructionData,
      });

      const tx = new Transaction().add(claimIx);
      setStatus('⏳ Waiting for signature…');

      // Sign with Phantom
      if (!window.solana) throw new Error('Wallet disconnected');
      const signed = await window.solana.signTransaction(tx);
      const sig = await connection.sendRawTransaction(signed.serialize());

      setStatus('⏳ Confirming transaction…');
      await connection.confirmTransaction(sig, 'confirmed');

      // Fetch balance after
      try {
        const acctAfter = await getAccount(connection, claimerAta, 'confirmed', TOKEN_2022_PROGRAM_ID);
        const afterBalance = acctAfter.amount;
        setBalanceAfter(afterBalance.toString());
      } catch (e) {
        setBalanceAfter('Error');
      }

      setTxSignature(sig);
      setStatus('✅ Claim successful!');
      setClaimPhase('claimed');
    } catch (error: any) {
      setStatus(`❌ ${mapClaimError(error)}`);
      setClaimPhase('error');
    }
  };

  // Calculate if claim button should be enabled
  const canClaim = claimData && walletAddress && claimData.claimer === walletAddress && claimPhase !== 'claiming' && claimPhase !== 'claimed';

  // Stepper component
  const Stepper = () => {
    const steps = [
      { id: 'proof', label: '1. Load Proof', active: currentStep === 'proof', completed: !!claimData },
      { id: 'wallet', label: '2. Connect Wallet', active: currentStep === 'wallet', completed: !!walletAddress },
      { id: 'review', label: '3. Review', active: currentStep === 'review', completed: currentStep === 'claim' || claimPhase === 'claimed' },
      { id: 'claim', label: '4. Claim', active: currentStep === 'claim' || claimPhase === 'claimed', completed: claimPhase === 'claimed' },
    ];

    return (
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '2rem', padding: '1rem', backgroundColor: '#f5f5f5', borderRadius: '8px' }}>
        {steps.map((step, idx) => (
          <div key={step.id} style={{ display: 'flex', alignItems: 'center', flex: 1 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
              <div style={{
                width: '32px',
                height: '32px',
                borderRadius: '50%',
                backgroundColor: step.completed ? '#22c55e' : step.active ? '#3b82f6' : '#d1d5db',
                color: 'white',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                fontWeight: 'bold',
                fontSize: '0.9rem',
              }}>
                {step.completed ? '✓' : idx + 1}
              </div>
              <span style={{
                fontSize: '0.85rem',
                fontWeight: step.active ? 'bold' : 'normal',
                color: step.active ? '#1f2937' : '#6b7280',
              }}>
                {step.label}
              </span>
            </div>
            {idx < steps.length - 1 && (
              <div style={{ flex: 1, height: '2px', backgroundColor: '#d1d5db', margin: '0 1rem' }} />
            )}
          </div>
        ))}
      </div>
    );
  };

  return (
    <main className="wrap">
      <style>{`
        @keyframes spin {
          to { transform: rotate(360deg); }
        }
        .spinner {
          display: inline-block;
          width: 16px;
          height: 16px;
          border: 2px solid #ccc;
          border-top-color: #3b82f6;
          border-radius: 50%;
          animation: spin 0.8s linear infinite;
        }
      `}</style>

      <header className="top">
        <h1 className="brand">CLS Claim</h1>
        <div className="spacer" />
      </header>

      <Stepper />

      <section className="card stack">
        <h2>1. Load Proof</h2>
        <p className="muted">Upload or paste your claim proof JSON file.</p>

        <div style={{ marginBottom: '1rem' }}>
          <div style={{ marginBottom: '0.5rem' }}>
            <strong>Upload File:</strong>
          </div>
          <input
            type="file"
            accept=".json"
            onChange={handleProofUpload}
            style={{
              padding: '0.5rem',
              borderRadius: '4px',
              border: '1px solid #ccc',
              width: '100%',
              fontFamily: 'monospace',
            }}
          />
        </div>

        <div style={{ marginBottom: '0.5rem' }}>
          <strong>Or paste JSON:</strong>
        </div>
        <textarea
          value={proofInput}
          onChange={(e) => setProofInput(e.target.value)}
          placeholder='{"claimer": "...", "mint": "...", ...}'
          style={{
            width: '100%',
            minHeight: '150px',
            padding: '0.5rem',
            fontFamily: 'monospace',
            fontSize: '0.85rem',
            borderRadius: '4px',
            border: '1px solid #ccc',
            resize: 'vertical',
          }}
        />
        <button onClick={handleProofPaste} className="secondary" style={{ marginTop: '0.5rem' }}>
          Parse JSON
        </button>
      </section>

      {claimData && (
        <>
          <section className="card stack">
            <h2>2. Claim Details</h2>
            <div style={{ fontSize: '0.9rem', fontFamily: 'monospace', lineHeight: '1.8' }}>
              <div>
                <strong>Channel:</strong> {claimData.channel}
              </div>
              <div>
                <strong>Epoch:</strong> {claimData.epoch}
              </div>
              <div>
                <strong>Gross Amount:</strong> {formatTokenAmount(BigInt(claimData.amount))} tokens
              </div>
              <div>
                <strong>Transfer Fee (1%):</strong> {formatTokenAmount(calculateNetAmount(claimData.amount).fee)} tokens
              </div>
              <div style={{ backgroundColor: '#f0fdf4', padding: '0.5rem', marginTop: '0.5rem', borderRadius: '4px' }}>
                <strong>Net Amount:</strong> <span style={{ color: '#22c55e', fontSize: '1.1rem' }}>{formatTokenAmount(calculateNetAmount(claimData.amount).net)}</span> tokens
              </div>
              <div style={{ marginTop: '0.5rem' }}>
                <strong>Claim ID:</strong> {claimData.id.slice(0, 40)}…
              </div>
              <div>
                <strong>Proof Nodes:</strong> {claimData.proof.length}
              </div>
            </div>
          </section>

          <section className="card stack">
            <h2>3. Wallet</h2>
            <p className="muted">Connect Phantom to sign the claim.</p>
            {!walletAddress ? (
              <button onClick={connectWallet} className="primary" disabled={claimPhase === 'connecting'}>
                {claimPhase === 'connecting' ? 'Connecting…' : 'Connect Wallet'}
              </button>
            ) : (
              <div
                style={{
                  padding: '1rem',
                  backgroundColor: '#f5f5f5',
                  borderRadius: '4px',
                  wordBreak: 'break-all',
                  fontFamily: 'monospace',
                  fontSize: '0.85rem',
                }}
              >
                {walletAddress}
              </div>
            )}
          </section>

          {walletAddress && (
            <section className="card stack">
              <h2>4. Submit Claim</h2>
              {!canClaim && claimData && walletAddress && claimData.claimer !== walletAddress && (
                <div style={{ padding: '1rem', backgroundColor: '#fef2f2', borderRadius: '4px', marginBottom: '1rem', color: '#991b1b' }}>
                  ⚠️ <strong>Wallet mismatch:</strong> This proof is for a different wallet address.
                </div>
              )}
              <button
                onClick={claim}
                className="primary"
                disabled={!canClaim}
                style={{
                  padding: '1rem',
                  fontSize: '1rem',
                  fontWeight: 'bold',
                  opacity: canClaim ? 1 : 0.5,
                  cursor: canClaim ? 'pointer' : 'not-allowed',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  gap: '0.5rem',
                }}
              >
                {claimPhase === 'claiming' && <div className="spinner" />}
                {claimPhase === 'claiming' ? 'Claiming…' : claimPhase === 'claimed' ? '✅ Claimed' : 'Submit Claim'}
              </button>

              {(balanceBefore !== '—' || balanceAfter !== '—') && (
                <div style={{ marginTop: '1rem', fontSize: '0.9rem' }}>
                  <div>
                    <strong>Balance Before:</strong> {balanceBefore}
                  </div>
                  <div>
                    <strong>Balance After:</strong> {balanceAfter}
                  </div>
                  {balanceBefore !== '—' && balanceAfter !== '—' && balanceAfter !== 'Error' && (
                    <div>
                      <strong>Received:</strong> {(BigInt(balanceAfter) - BigInt(balanceBefore.split(' ')[0])).toString()}{' '}
                      tokens
                    </div>
                  )}
                </div>
              )}

              {txSignature && (
                <div
                  style={{
                    marginTop: '1rem',
                    padding: '1.5rem',
                    backgroundColor: '#f0fdf4',
                    border: '2px solid #22c55e',
                    borderRadius: '8px',
                    fontSize: '0.9rem',
                  }}
                >
                  <div style={{ fontSize: '1.2rem', fontWeight: 'bold', color: '#22c55e', marginBottom: '1rem' }}>
                    ✅ Claim Successful!
                  </div>
                  <div style={{ marginBottom: '0.75rem', fontFamily: 'monospace', fontSize: '0.85rem' }}>
                    <strong>Transaction Signature:</strong>
                    <div style={{ wordBreak: 'break-all', marginTop: '0.25rem', color: '#666' }}>{txSignature}</div>
                  </div>
                  <a
                    href={`${import.meta.env.VITE_EXPLORER_BASE || 'https://explorer.solana.com'}/tx/${txSignature}`}
                    target="_blank"
                    rel="noreferrer"
                    style={{
                      display: 'inline-block',
                      marginTop: '0.5rem',
                      padding: '0.5rem 1rem',
                      backgroundColor: '#22c55e',
                      color: 'white',
                      borderRadius: '4px',
                      textDecoration: 'none',
                      fontWeight: 'bold',
                    }}
                  >
                    View on Explorer →
                  </a>
                </div>
              )}

              {status && (
                <div
                  style={{
                    marginTop: '1rem',
                    padding: '0.75rem',
                    backgroundColor: claimPhase === 'error' ? '#fee2e2' : claimPhase === 'claimed' ? '#f0fdf4' : claimPhase === 'claiming' ? '#eff6ff' : '#fef3c7',
                    borderRadius: '4px',
                    fontSize: '0.9rem',
                    color: claimPhase === 'error' ? '#991b1b' : claimPhase === 'claimed' ? '#166534' : claimPhase === 'claiming' ? '#1e40af' : '#92400e',
                    display: 'flex',
                    alignItems: 'center',
                    gap: '0.5rem',
                  }}
                >
                  {claimPhase === 'claiming' && <div className="spinner" />}
                  <span>{status}</span>
                </div>
              )}
            </section>
          )}
        </>
      )}

      <footer className="foot">
        <span className="muted">CLS Token Claim • Mainnet</span>
      </footer>
    </main>
  );
}
