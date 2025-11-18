/**
 * Attention Oracle TypeScript SDK
 *
 * Provides type-safe interfaces for interacting with the Attention Oracle program.
 *
 * @packageDocumentation
 */

export * from './client';
export * from './instructions';
export * from './accounts';
export * from './types';
export * from './utils';

// Re-export common types from dependencies
export { PublicKey, Connection, Keypair } from '@solana/web3.js';
export { Program, AnchorProvider } from '@coral-xyz/anchor';
