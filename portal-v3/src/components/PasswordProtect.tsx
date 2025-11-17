import React, { useState, useEffect } from 'react';

const PASSWORD = 'ISHOWSPEED2025';

interface PasswordProtectProps {
  children: React.ReactNode;
}

export const PasswordProtect: React.FC<PasswordProtectProps> = ({ children }) => {
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');

  useEffect(() => {
    // Check if already authenticated in session storage
    const stored = sessionStorage.getItem('portal-auth');
    if (stored === 'true') {
      setIsAuthenticated(true);
    }
  }, []);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (password === PASSWORD) {
      setIsAuthenticated(true);
      sessionStorage.setItem('portal-auth', 'true');
      setError('');
    } else {
      setError('Invalid password');
      setPassword('');
    }
  };

  if (!isAuthenticated) {
    return (
      <div style={styles.container}>
        <div style={styles.dialog}>
          <h1 style={styles.title}>Attention Oracle Portal</h1>
          <p style={styles.subtitle}>Early Access Portal</p>

          <form onSubmit={handleSubmit} style={styles.form}>
            <label style={styles.label}>Enter Password</label>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="Password"
              style={styles.input}
              autoFocus
            />
            {error && <p style={styles.error}>{error}</p>}
            <button type="submit" style={styles.button}>
              Unlock
            </button>
          </form>

          <p style={styles.hint}>
            This is an early access portal. Please contact support for access.
          </p>
        </div>
      </div>
    );
  }

  return <>{children}</>;
};

const styles = {
  container: {
    minHeight: '100vh',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: '#f9fafb',
    padding: '1.5rem',
  } as React.CSSProperties,

  dialog: {
    width: '100%',
    maxWidth: '400px',
    backgroundColor: '#ffffff',
    border: '1px solid #e5e7eb',
    borderRadius: '8px',
    padding: '2rem',
    boxShadow: '0 4px 12px rgba(0,0,0,0.1)',
  } as React.CSSProperties,

  title: {
    fontSize: '1.75rem',
    fontWeight: '700',
    margin: '0 0 0.5rem 0',
    color: '#1f2937',
    textAlign: 'center',
  } as React.CSSProperties,

  subtitle: {
    fontSize: '0.95rem',
    color: '#6b7280',
    margin: '0 0 1.5rem 0',
    textAlign: 'center',
  } as React.CSSProperties,

  form: {
    display: 'flex',
    flexDirection: 'column' as const,
    gap: '1rem',
  } as React.CSSProperties,

  label: {
    fontSize: '0.9rem',
    fontWeight: '500',
    color: '#374151',
  } as React.CSSProperties,

  input: {
    padding: '0.75rem',
    border: '1px solid #d1d5db',
    borderRadius: '6px',
    fontSize: '1rem',
    fontFamily: 'monospace',
    boxSizing: 'border-box' as const,
    width: '100%',
  } as React.CSSProperties,

  button: {
    padding: '0.75rem 1.5rem',
    backgroundColor: '#3b82f6',
    color: 'white',
    border: 'none',
    borderRadius: '6px',
    fontSize: '1rem',
    fontWeight: '600',
    cursor: 'pointer',
    transition: 'all 0.2s',
  } as React.CSSProperties,

  error: {
    color: '#dc2626',
    fontSize: '0.85rem',
    margin: '0',
    textAlign: 'center',
  } as React.CSSProperties,

  hint: {
    fontSize: '0.8rem',
    color: '#9ca3af',
    margin: '1rem 0 0 0',
    textAlign: 'center',
  } as React.CSSProperties,
};

export default PasswordProtect;
