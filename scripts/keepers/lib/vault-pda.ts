/**
 * Complete PDA derivation for both Oracle and Vault programs.
 *
 * Given a channel_config pubkey and the CCM mint, every account
 * required by compound / harvest / deploy can be derived deterministically.
 */

import { PublicKey } from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";

// =========================================================================
// Program IDs
// =========================================================================

export const ORACLE_PROGRAM_ID = new PublicKey(
  "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop",
);
export const VAULT_PROGRAM_ID = new PublicKey(
  "5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ",
);
export const METADATA_PROGRAM_ID = new PublicKey(
  "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s",
);

// Mainnet CCM token mint (Token-2022)
export const CCM_MINT = new PublicKey(
  "Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM",
);

// =========================================================================
// Seeds (matching constants.rs in both programs)
// =========================================================================

const PROTOCOL_SEED = Buffer.from("protocol");
const CHANNEL_STAKE_POOL_SEED = Buffer.from("channel_pool");
const CHANNEL_USER_STAKE_SEED = Buffer.from("channel_user");
const STAKE_NFT_MINT_SEED = Buffer.from("stake_nft");
const STAKE_VAULT_SEED = Buffer.from("stake_vault");

const VAULT_SEED = Buffer.from("vault");
const VAULT_CCM_BUFFER_SEED = Buffer.from("vault_ccm");
const VLOFI_MINT_SEED = Buffer.from("vlofi");
const VAULT_ORACLE_POSITION_SEED = Buffer.from("vault_oracle");

// =========================================================================
// Oracle-side PDAs
// =========================================================================

export function deriveProtocolState(ccmMint: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, ccmMint.toBuffer()],
    ORACLE_PROGRAM_ID,
  )[0];
}

export function deriveFeeConfig(ccmMint: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, ccmMint.toBuffer(), Buffer.from("fee_config")],
    ORACLE_PROGRAM_ID,
  )[0];
}

export function deriveOracleStakePool(
  channelConfig: PublicKey,
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [CHANNEL_STAKE_POOL_SEED, channelConfig.toBuffer()],
    ORACLE_PROGRAM_ID,
  )[0];
}

export function deriveOracleStakeVault(stakePool: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [STAKE_VAULT_SEED, stakePool.toBuffer()],
    ORACLE_PROGRAM_ID,
  )[0];
}

export function deriveOracleUserStake(
  channelConfig: PublicKey,
  user: PublicKey,
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [CHANNEL_USER_STAKE_SEED, channelConfig.toBuffer(), user.toBuffer()],
    ORACLE_PROGRAM_ID,
  )[0];
}

export function deriveOracleNftMint(
  stakePool: PublicKey,
  user: PublicKey,
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [STAKE_NFT_MINT_SEED, stakePool.toBuffer(), user.toBuffer()],
    ORACLE_PROGRAM_ID,
  )[0];
}

// =========================================================================
// Vault-side PDAs
// =========================================================================

export function deriveVault(channelConfig: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [VAULT_SEED, channelConfig.toBuffer()],
    VAULT_PROGRAM_ID,
  )[0];
}

export function deriveVlofiMint(vault: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [VLOFI_MINT_SEED, vault.toBuffer()],
    VAULT_PROGRAM_ID,
  )[0];
}

export function deriveCcmBuffer(vault: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [VAULT_CCM_BUFFER_SEED, vault.toBuffer()],
    VAULT_PROGRAM_ID,
  )[0];
}

export function deriveOraclePosition(vault: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [VAULT_ORACLE_POSITION_SEED, vault.toBuffer()],
    VAULT_PROGRAM_ID,
  )[0];
}

export function deriveMetadata(mint: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("metadata"),
      METADATA_PROGRAM_ID.toBuffer(),
      mint.toBuffer(),
    ],
    METADATA_PROGRAM_ID,
  )[0];
}

// =========================================================================
// Compound account bundle
// =========================================================================

export interface CompoundAccounts {
  vault: PublicKey;
  vaultOraclePosition: PublicKey;
  vaultCcmBuffer: PublicKey;
  ccmMint: PublicKey;
  oracleProgram: PublicKey;
  oracleProtocol: PublicKey;
  oracleChannelConfig: PublicKey;
  oracleStakePool: PublicKey;
  oracleVault: PublicKey;
  oracleUserStake: PublicKey;
  oracleNftMint: PublicKey;
  vaultNftAta: PublicKey;
}

/**
 * Derive all accounts needed for vault.compound() from
 * a channel_config pubkey and the CCM mint.
 */
export function deriveCompoundAccounts(
  channelConfig: PublicKey,
  ccmMint: PublicKey,
): CompoundAccounts {
  const vault = deriveVault(channelConfig);
  const oracleStakePool = deriveOracleStakePool(channelConfig);
  const oracleNftMint = deriveOracleNftMint(oracleStakePool, vault);
  const vaultNftAta = getAssociatedTokenAddressSync(
    oracleNftMint,
    vault,
    true, // allowOwnerOffCurve (vault is a PDA)
    TOKEN_2022_PROGRAM_ID,
  );

  return {
    vault,
    vaultOraclePosition: deriveOraclePosition(vault),
    vaultCcmBuffer: deriveCcmBuffer(vault),
    ccmMint,
    oracleProgram: ORACLE_PROGRAM_ID,
    oracleProtocol: deriveProtocolState(ccmMint),
    oracleChannelConfig: channelConfig,
    oracleStakePool,
    oracleVault: deriveOracleStakeVault(oracleStakePool),
    oracleUserStake: deriveOracleUserStake(channelConfig, vault),
    oracleNftMint,
    vaultNftAta,
  };
}
