import React, { ReactNode } from 'react';

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error?: Error;
}

class ErrorBoundary extends React.Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('ClaimCLS Error:', error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div style={styles.errorContainer}>
          <div style={styles.errorBox}>
            <h2 style={styles.errorTitle}>Something went wrong</h2>
            <p style={styles.errorMessage}>
              An unexpected error occurred. Please refresh the page and try again.
            </p>
            {this.state.error && (
              <details style={styles.errorDetails}>
                <summary style={styles.errorSummary}>Error details</summary>
                <pre style={styles.errorStack}>{this.state.error.toString()}</pre>
              </details>
            )}
            <button
              onClick={() => window.location.reload()}
              style={styles.errorButton}
            >
              Refresh Page
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

const styles = {
  errorContainer: {
    display: 'flex',
    justifyContent: 'center',
    alignItems: 'center',
    minHeight: '400px',
    padding: '2rem 1.5rem',
  } as React.CSSProperties,

  errorBox: {
    backgroundColor: '#fee2e2',
    border: '1px solid #fecaca',
    borderRadius: '8px',
    padding: '2rem',
    maxWidth: '500px',
    textAlign: 'center' as const,
  } as React.CSSProperties,

  errorTitle: {
    color: '#991b1b',
    fontSize: '1.5rem',
    fontWeight: '600',
    margin: '0 0 1rem 0',
  } as React.CSSProperties,

  errorMessage: {
    color: '#7f1d1d',
    fontSize: '0.95rem',
    margin: '0 0 1.5rem 0',
    lineHeight: '1.6',
  } as React.CSSProperties,

  errorDetails: {
    textAlign: 'left' as const,
    backgroundColor: '#fef2f2',
    padding: '1rem',
    borderRadius: '4px',
    margin: '0 0 1.5rem 0',
    cursor: 'pointer',
  } as React.CSSProperties,

  errorSummary: {
    color: '#991b1b',
    fontWeight: '500',
    fontSize: '0.9rem',
  } as React.CSSProperties,

  errorStack: {
    fontSize: '0.75rem',
    color: '#7f1d1d',
    margin: '0.75rem 0 0 0',
    padding: '0.75rem',
    backgroundColor: '#fecaca',
    borderRadius: '4px',
    overflow: 'auto',
    maxHeight: '200px',
  } as React.CSSProperties,

  errorButton: {
    backgroundColor: '#dc2626',
    color: '#ffffff',
    border: 'none',
    padding: '0.75rem 1.5rem',
    borderRadius: '6px',
    fontSize: '0.95rem',
    fontWeight: '500',
    cursor: 'pointer',
    transition: 'background-color 0.2s',
  } as React.CSSProperties,
};

export default ErrorBoundary;
