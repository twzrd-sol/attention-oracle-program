import React from 'react';
import { WalletMultiButton } from '@solana/wallet-adapter-react-ui';
import ClaimCLS from './components/ClaimCLS';
import { PasswordProtect } from './components/PasswordProtect';
import { getClusterName, isMainnet } from './lib/solana';

export const App: React.FC = () => {
  return (
    <PasswordProtect>
      <div style={styles.appContainer}>
        {/* Header */}
        <header style={styles.header}>
        <div style={styles.headerContent}>
          <div style={styles.headerBrand}>
            <h1 style={styles.brandName}>TWZRD</h1>
            <p style={styles.brandTagline}>Attention Oracle Portal</p>
          </div>
          <WalletMultiButton />
        </div>
        {!isMainnet() && (
          <div style={styles.networkBanner}>
                    ⚠️ {getClusterName()} Network
          </div>
        )}
      </header>

      {/* Main Content */}
      <main style={styles.main}>
        <ClaimCLS />
      </main>

      {/* Footer */}
      <footer style={styles.footer}>
        <div style={styles.footerContent}>
          <p style={styles.footerText}>
            Learn more about{' '}
            <a href="https://github.com/twzrd-sol/attention-oracle-program" target="_blank" rel="noopener noreferrer" style={styles.footerLink}>
              Attention Oracle
            </a>
            {' '}| Join the{' '}
            <a href="https://discord.gg/twzrd" target="_blank" rel="noopener noreferrer" style={styles.footerLink}>
              Discord Community
            </a>
          </p>
          <p style={styles.footerMeta}>
            Running on{' '}
            <strong>{getClusterName()}</strong>
          </p>
        </div>
      </footer>
    </div>
    </PasswordProtect>
  );
};

const styles = {
  appContainer: {
    minHeight: '100vh',
    display: 'flex',
    flexDirection: 'column' as const,
    backgroundColor: '#f9fafb',
  },

  header: {
    backgroundColor: '#ffffff',
    borderBottom: '1px solid #e5e7eb',
    boxShadow: '0 1px 3px rgba(0,0,0,0.05)',
  } as React.CSSProperties,

  headerContent: {
    maxWidth: '1200px',
    margin: '0 auto',
    padding: '1rem 1.5rem',
    width: '100%',
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
  } as React.CSSProperties,

  headerBrand: {
    display: 'flex',
    flexDirection: 'column' as const,
    gap: '0.25rem',
  } as React.CSSProperties,

  brandName: {
    fontSize: '1.5rem',
    fontWeight: '800',
    margin: '0',
    color: '#1f2937',
    letterSpacing: '-0.5px',
  } as React.CSSProperties,

  brandTagline: {
    fontSize: '0.8rem',
    color: '#6b7280',
    margin: '0',
    fontWeight: '500',
  } as React.CSSProperties,

  networkBanner: {
    backgroundColor: '#fef3c7',
    color: '#92400e',
    padding: '0.5rem 1.5rem',
    textAlign: 'center' as const,
    fontSize: '0.85rem',
    fontWeight: '500',
    borderTop: '1px solid #fcd34d',
  } as React.CSSProperties,

  main: {
    flex: 1,
    maxWidth: '1200px',
    margin: '0 auto',
    width: '100%',
    padding: '2rem 1.5rem',
  } as React.CSSProperties,

  footer: {
    backgroundColor: '#ffffff',
    borderTop: '1px solid #e5e7eb',
    padding: '2rem 1.5rem',
    marginTop: 'auto',
  } as React.CSSProperties,

  footerContent: {
    maxWidth: '1200px',
    margin: '0 auto',
    textAlign: 'center' as const,
  } as React.CSSProperties,

  footerText: {
    fontSize: '0.9rem',
    color: '#6b7280',
    margin: '0 0 0.5rem 0',
  } as React.CSSProperties,

  footerMeta: {
    fontSize: '0.85rem',
    color: '#9ca3af',
    margin: '0',
  } as React.CSSProperties,

  footerLink: {
    color: '#3b82f6',
    textDecoration: 'none',
    fontWeight: '500',
  } as React.CSSProperties,
};

export default App;
