import React, { useRef } from 'react';
import { useMerkleProof } from '@hooks';

interface ProofUploadProps {
  onProofLoaded?: () => void;
}

export const ProofUpload: React.FC<ProofUploadProps> = ({ onProofLoaded }) => {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const { proof, loading, error, loadProofFromFile, loadProofFromJSON, clearProof } = useMerkleProof();
  const [jsonInput, setJsonInput] = React.useState('');

  const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    await loadProofFromFile(file);
    if (!error) {
      onProofLoaded?.();
    }
  };

  const handlePasteJSON = () => {
    if (!jsonInput.trim()) {
      return;
    }
    loadProofFromJSON(jsonInput);
    if (!error) {
      onProofLoaded?.();
    }
  };

  const handleClear = () => {
    clearProof();
    setJsonInput('');
    if (fileInputRef.current) {
      fileInputRef.current.value = '';
    }
  };

  return (
    <div style={styles.container}>
      <div style={styles.card}>
        <h2 style={styles.title}>1. Load Proof</h2>
        <p style={styles.subtitle}>Upload or paste your claim proof JSON file.</p>

        {!proof ? (
          <>
            {/* File Upload */}
            <div style={styles.section}>
              <label style={styles.label}>Upload File</label>
              <input
                ref={fileInputRef}
                type="file"
                accept=".json"
                onChange={handleFileChange}
                disabled={loading}
                style={styles.fileInput}
              />
            </div>

            {/* Divider */}
            <div style={styles.divider}>or</div>

            {/* JSON Paste */}
            <div style={styles.section}>
              <label style={styles.label}>Paste JSON</label>
              <textarea
                value={jsonInput}
                onChange={(e) => setJsonInput(e.target.value)}
                placeholder='{"claimer": "...", "mint": "...", "channel": "...", ...}'
                disabled={loading}
                style={styles.textarea}
              />
              <button
                onClick={handlePasteJSON}
                disabled={loading || !jsonInput.trim()}
                style={{
                  ...styles.button,
                  ...(loading || !jsonInput.trim() ? styles.buttonDisabled : styles.buttonSecondary),
                }}
              >
                {loading ? 'Parsing...' : 'Parse JSON'}
              </button>
            </div>

            {/* Error Message */}
            {error && (
              <div style={styles.errorBox}>
                <strong>❌ Error:</strong> {error}
              </div>
            )}
          </>
        ) : (
          <>
            {/* Proof Loaded */}
            <div style={styles.successBox}>
              <div style={styles.successTitle}>✅ Proof Loaded</div>
              <div style={styles.proofSummary}>
                <div><strong>Channel:</strong> {proof.channel}</div>
                <div><strong>Epoch:</strong> {proof.epoch}</div>
                <div><strong>Claimer:</strong> {proof.claimer.slice(0, 8)}...{proof.claimer.slice(-6)}</div>
                <div><strong>Amount:</strong> {proof.amount} tokens</div>
                <div><strong>Proof Depth:</strong> {proof.proof.length} nodes</div>
              </div>
              <button
                onClick={handleClear}
                style={{ ...styles.button, ...styles.buttonSecondary, marginTop: '1rem' }}
              >
                Load Different Proof
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
};

const styles = {
  container: {
    padding: '1.5rem',
  } as React.CSSProperties,

  card: {
    backgroundColor: '#ffffff',
    border: '1px solid #e5e7eb',
    borderRadius: '8px',
    padding: '1.5rem',
    boxShadow: '0 1px 3px rgba(0,0,0,0.1)',
  } as React.CSSProperties,

  title: {
    fontSize: '1.5rem',
    fontWeight: '600',
    margin: '0 0 0.5rem 0',
    color: '#1f2937',
  } as React.CSSProperties,

  subtitle: {
    fontSize: '0.9rem',
    color: '#6b7280',
    margin: '0 0 1.5rem 0',
  } as React.CSSProperties,

  section: {
    marginBottom: '1.5rem',
  } as React.CSSProperties,

  label: {
    display: 'block',
    fontSize: '0.9rem',
    fontWeight: '500',
    color: '#374151',
    marginBottom: '0.5rem',
  } as React.CSSProperties,

  fileInput: {
    display: 'block',
    width: '100%',
    padding: '0.75rem',
    border: '1px solid #d1d5db',
    borderRadius: '6px',
    fontSize: '0.9rem',
    fontFamily: 'monospace',
  } as React.CSSProperties,

  textarea: {
    display: 'block',
    width: '100%',
    minHeight: '150px',
    padding: '0.75rem',
    border: '1px solid #d1d5db',
    borderRadius: '6px',
    fontSize: '0.85rem',
    fontFamily: 'monospace',
    fontWeight: '400',
    resize: 'vertical',
    boxSizing: 'border-box',
  } as React.CSSProperties,

  divider: {
    textAlign: 'center',
    color: '#9ca3af',
    margin: '1.5rem 0',
    fontSize: '0.9rem',
    fontWeight: '500',
  } as React.CSSProperties,

  button: {
    padding: '0.75rem 1.5rem',
    borderRadius: '6px',
    border: 'none',
    fontSize: '0.95rem',
    fontWeight: '500',
    cursor: 'pointer',
    transition: 'all 0.2s',
  } as React.CSSProperties,

  buttonPrimary: {
    backgroundColor: '#3b82f6',
    color: 'white',
  } as React.CSSProperties,

  buttonSecondary: {
    backgroundColor: '#f3f4f6',
    color: '#374151',
    border: '1px solid #d1d5db',
    marginTop: '0.75rem',
  } as React.CSSProperties,

  buttonDisabled: {
    opacity: 0.5,
    cursor: 'not-allowed',
  } as React.CSSProperties,

  errorBox: {
    padding: '1rem',
    backgroundColor: '#fee2e2',
    border: '1px solid #fca5a5',
    borderRadius: '6px',
    color: '#991b1b',
    fontSize: '0.9rem',
    marginTop: '1rem',
  } as React.CSSProperties,

  successBox: {
    padding: '1rem',
    backgroundColor: '#f0fdf4',
    border: '2px solid #22c55e',
    borderRadius: '6px',
    color: '#166534',
  } as React.CSSProperties,

  successTitle: {
    fontSize: '1rem',
    fontWeight: '600',
    marginBottom: '1rem',
  } as React.CSSProperties,

  proofSummary: {
    fontSize: '0.9rem',
    fontFamily: 'monospace',
    lineHeight: '1.8',
    color: '#166534',
  } as React.CSSProperties,
};

export default ProofUpload;
