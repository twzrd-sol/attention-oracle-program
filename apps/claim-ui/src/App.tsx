import React, { useState } from 'react';
import { ProofUpload, WalletConnect, ClaimReview, ClaimExecution } from './components';
import { useMerkleProof } from './hooks';

type ClaimStep = 'proof' | 'wallet' | 'review' | 'execution' | 'complete';

interface StepConfig {
  id: ClaimStep;
  label: string;
  component: React.ComponentType<any>;
}

const steps: StepConfig[] = [
  { id: 'proof', label: 'Load Proof', component: ProofUpload },
  { id: 'wallet', label: 'Connect Wallet', component: WalletConnect },
  { id: 'review', label: 'Review Claim', component: ClaimReview },
  { id: 'execution', label: 'Execute', component: ClaimExecution },
];

export const App: React.FC = () => {
  const [currentStep, setCurrentStep] = useState<ClaimStep>('proof');
  const { proof } = useMerkleProof();

  const currentStepIndex = steps.findIndex(s => s.id === currentStep);
  const isLastStep = currentStepIndex === steps.length - 1;

  const handleProofLoaded = () => {
    setCurrentStep('wallet');
  };

  const handleWalletConnected = () => {
    // Auto-advance to review
    setCurrentStep('review');
  };

  const handleReviewProceed = () => {
    setCurrentStep('execution');
  };

  const handleExecutionSuccess = (signature: string) => {
    setCurrentStep('complete');
  };

  const handleReset = () => {
    setCurrentStep('proof');
  };

  const renderStep = () => {
    const stepConfig = steps.find(s => s.id === currentStep);
    if (!stepConfig) return null;

    const Component = stepConfig.component;

    switch (currentStep) {
      case 'proof':
        return <Component onProofLoaded={handleProofLoaded} />;
      case 'wallet':
        return (
          <Component
            onConnected={handleWalletConnected}
            proofClaimerAddress={proof?.claimer}
          />
        );
      case 'review':
        return <Component onProceed={handleReviewProceed} walletAddress={proof?.claimer} />;
      case 'execution':
        return (
          <Component
            onSuccess={handleExecutionSuccess}
            onError={() => {
              // Keep user on execution step on error, they can retry
            }}
          />
        );
      default:
        return null;
    }
  };

  const completionPercentage = currentStep === 'complete'
    ? 100
    : ((currentStepIndex + 1) / steps.length) * 100;

  return (
    <div style={styles.appContainer}>
      {/* Header */}
      <div style={styles.header}>
        <div style={styles.headerContent}>
          <h1 style={styles.appTitle}>Attention Oracle Claim Portal</h1>
          <p style={styles.appSubtitle}>Claim your creator tokens from Twitch channel rewards</p>
        </div>
      </div>

      {/* Main Content */}
      <div style={styles.mainContainer}>
        {/* Stepper - Only show if not complete */}
        {currentStep !== 'complete' && (
          <div style={styles.stepperContainer}>
            <div style={styles.progressBar}>
              <div
                style={{
                  ...styles.progressFill,
                  width: `${completionPercentage}%`,
                }}
              />
            </div>
            <div style={styles.stepsGrid}>
              {steps.map((step, index) => {
                const isActive = currentStep === step.id;
                const isCompleted = currentStepIndex > index;

                return (
                  <div
                    key={step.id}
                    style={{
                      ...styles.stepItem,
                      ...(isActive && styles.stepItemActive),
                      ...(isCompleted && styles.stepItemCompleted),
                    }}
                  >
                    <div
                      style={{
                        ...styles.stepNumber,
                        ...(isActive && styles.stepNumberActive),
                        ...(isCompleted && styles.stepNumberCompleted),
                      }}
                    >
                      {isCompleted ? '✓' : index + 1}
                    </div>
                    <div
                      style={{
                        ...styles.stepLabel,
                        ...(isActive && styles.stepLabelActive),
                        ...(isCompleted && styles.stepLabelCompleted),
                      }}
                    >
                      {step.label}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {/* Step Content */}
        <div style={styles.stepContent}>{renderStep()}</div>

        {/* Completion Screen */}
        {currentStep === 'complete' && (
          <div style={styles.completionContainer}>
            <div style={styles.completionCard}>
              <div style={styles.completionIcon}>🎉</div>
              <h2 style={styles.completionTitle}>Claim Complete!</h2>
              <p style={styles.completionText}>
                Your claim has been successfully processed and your tokens have been transferred to your
                wallet. You can now use your tokens on any Solana DEX or hold for future appreciation.
              </p>

              <div style={styles.completionActions}>
                <button
                  onClick={handleReset}
                  style={{ ...styles.button, ...styles.buttonPrimary }}
                >
                  Claim Again
                </button>
                <a
                  href="https://phantom.app"
                  target="_blank"
                  rel="noopener noreferrer"
                  style={{ ...styles.button, ...styles.buttonSecondary, textDecoration: 'none' }}
                >
                  View in Wallet
                </a>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Footer */}
      <div style={styles.footer}>
        <p style={styles.footerText}>
          Questions? Check out our{' '}
          <a href="#" style={styles.footerLink}>
            documentation
          </a>{' '}
          or reach out on{' '}
          <a href="#" style={styles.footerLink}>
            Discord
          </a>
        </p>
      </div>
    </div>
  );
};

const styles = {
  appContainer: {
    minHeight: '100vh',
    backgroundColor: '#f9fafb',
    display: 'flex',
    flexDirection: 'column' as const,
  },

  header: {
    backgroundColor: '#ffffff',
    borderBottom: '1px solid #e5e7eb',
    padding: '2rem 0',
    boxShadow: '0 1px 3px rgba(0,0,0,0.05)',
  } as React.CSSProperties,

  headerContent: {
    maxWidth: '1200px',
    margin: '0 auto',
    padding: '0 1.5rem',
  } as React.CSSProperties,

  appTitle: {
    fontSize: '2rem',
    fontWeight: '700',
    margin: '0 0 0.5rem 0',
    color: '#1f2937',
  } as React.CSSProperties,

  appSubtitle: {
    fontSize: '1rem',
    color: '#6b7280',
    margin: '0',
  } as React.CSSProperties,

  mainContainer: {
    flex: 1,
    maxWidth: '1200px',
    margin: '0 auto',
    width: '100%',
    padding: '2rem 1.5rem',
  } as React.CSSProperties,

  stepperContainer: {
    marginBottom: '2rem',
  } as React.CSSProperties,

  progressBar: {
    width: '100%',
    height: '4px',
    backgroundColor: '#e5e7eb',
    borderRadius: '2px',
    overflow: 'hidden',
    marginBottom: '1.5rem',
  } as React.CSSProperties,

  progressFill: {
    height: '100%',
    backgroundColor: '#3b82f6',
    transition: 'width 0.3s ease',
  } as React.CSSProperties,

  stepsGrid: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fit, minmax(120px, 1fr))',
    gap: '1rem',
  } as React.CSSProperties,

  stepItem: {
    display: 'flex',
    flexDirection: 'column' as const,
    alignItems: 'center',
    gap: '0.5rem',
    opacity: 0.5,
    transition: 'all 0.2s',
  } as React.CSSProperties,

  stepItemActive: {
    opacity: 1,
  } as React.CSSProperties,

  stepItemCompleted: {
    opacity: 1,
  } as React.CSSProperties,

  stepNumber: {
    width: '36px',
    height: '36px',
    borderRadius: '50%',
    backgroundColor: '#e5e7eb',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    fontWeight: '600',
    color: '#6b7280',
    fontSize: '0.95rem',
  } as React.CSSProperties,

  stepNumberActive: {
    backgroundColor: '#3b82f6',
    color: 'white',
    boxShadow: '0 0 0 3px rgba(59, 130, 246, 0.1)',
  } as React.CSSProperties,

  stepNumberCompleted: {
    backgroundColor: '#22c55e',
    color: 'white',
  } as React.CSSProperties,

  stepLabel: {
    fontSize: '0.85rem',
    fontWeight: '500',
    color: '#6b7280',
    textAlign: 'center',
  } as React.CSSProperties,

  stepLabelActive: {
    color: '#1f2937',
    fontWeight: '600',
  } as React.CSSProperties,

  stepLabelCompleted: {
    color: '#166534',
  } as React.CSSProperties,

  stepContent: {
    animation: 'fadeIn 0.3s ease',
  } as React.CSSProperties,

  completionContainer: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    minHeight: '400px',
  } as React.CSSProperties,

  completionCard: {
    backgroundColor: '#ffffff',
    border: '2px solid #22c55e',
    borderRadius: '12px',
    padding: '3rem 2rem',
    textAlign: 'center' as const,
    maxWidth: '500px',
    boxShadow: '0 10px 25px rgba(0,0,0,0.1)',
  } as React.CSSProperties,

  completionIcon: {
    fontSize: '4rem',
    marginBottom: '1rem',
  } as React.CSSProperties,

  completionTitle: {
    fontSize: '1.75rem',
    fontWeight: '700',
    color: '#166534',
    margin: '0 0 1rem 0',
  } as React.CSSProperties,

  completionText: {
    fontSize: '1rem',
    color: '#4b5563',
    lineHeight: '1.6',
    margin: '0 0 2rem 0',
  } as React.CSSProperties,

  completionActions: {
    display: 'flex',
    gap: '1rem',
    justifyContent: 'center',
  } as React.CSSProperties,

  button: {
    padding: '0.75rem 1.5rem',
    borderRadius: '6px',
    border: 'none',
    fontSize: '0.95rem',
    fontWeight: '500',
    cursor: 'pointer',
    transition: 'all 0.2s',
    display: 'inline-block',
  } as React.CSSProperties,

  buttonPrimary: {
    backgroundColor: '#3b82f6',
    color: 'white',
  } as React.CSSProperties,

  buttonSecondary: {
    backgroundColor: '#f3f4f6',
    color: '#374151',
    border: '1px solid #d1d5db',
  } as React.CSSProperties,

  footer: {
    backgroundColor: '#ffffff',
    borderTop: '1px solid #e5e7eb',
    padding: '2rem 1.5rem',
    textAlign: 'center' as const,
    marginTop: 'auto',
  } as React.CSSProperties,

  footerText: {
    fontSize: '0.9rem',
    color: '#6b7280',
    margin: '0',
  } as React.CSSProperties,

  footerLink: {
    color: '#3b82f6',
    textDecoration: 'none',
    fontWeight: '500',
  } as React.CSSProperties,
};

export default App;
