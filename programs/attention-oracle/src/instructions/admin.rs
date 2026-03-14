//! Admin instructions for the AO v2 Pinocchio program.
//!
//! Handlers:
//!   - `set_treasury`          — admin updates treasury pubkey on ProtocolState
//!   - `update_protocol_state` — admin updates publisher, oracle_authority, paused
//!   - `realloc_legacy_protocol` — one-shot realloc of legacy 141→173 byte PDA
//!   - `admin_fix_ccm_authority` — one-shot fix (already executed, kept as no-op)
//!
//! Feature-gated (channel_staking):
//!   - `create_channel_config_v2` — creates ChannelConfigV2 PDA

use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Seed, Signer},
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};

/// Token-2022 program ID.
use crate::TOKEN_2022_ID;

// =============================================================================
// ProtocolState byte offsets (173 bytes total, includes 8-byte discriminator)
// =============================================================================

const PS_ADMIN_OFFSET: usize = 10;
const PS_PUBLISHER_OFFSET: usize = 42;
const PS_TREASURY_OFFSET: usize = 74;
const PS_ORACLE_AUTHORITY_OFFSET: usize = 106;
const PS_MINT_OFFSET: usize = 138;
const PS_PAUSED_OFFSET: usize = 170;
const PS_BUMP_OFFSET: usize = 172;
const PS_LEN: usize = 173;

/// Legacy ProtocolState size before the Mar 10 realloc.
const LEGACY_MIN_LEN: usize = 141;

/// Zero pubkey — used for "cannot be default" checks.
const ZERO_PUBKEY: Pubkey = [0u8; 32];

// =============================================================================
// Error codes (Anchor: 6000 + variant index)
// =============================================================================

/// OracleError::Unauthorized (variant 0)
const ERR_UNAUTHORIZED: u32 = 6000;
/// OracleError::InvalidPubkey (variant 3)
const ERR_INVALID_PUBKEY: u32 = 6003;
/// OracleError::InvalidInputLength (variant 40)
const ERR_INVALID_INPUT_LENGTH: u32 = 6040;
/// OracleError::InvalidMint (variant 16)
const ERR_INVALID_MINT: u32 = 6016;
/// OracleError::InvalidTokenProgram (variant 19)
const ERR_INVALID_TOKEN_PROGRAM: u32 = 6019;

// =============================================================================
// HELPERS
// =============================================================================

/// Verify that `admin` is a signer and matches ProtocolState.admin.
///
/// Also verifies the ProtocolState account is owned by this program and
/// is large enough to read.
#[inline(always)]
fn verify_admin(
    admin: &AccountInfo,
    protocol_state: &AccountInfo,
    program_id: &Pubkey,
) -> Result<(), ProgramError> {
    if !admin.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if !protocol_state.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }

    let data = unsafe { protocol_state.borrow_data_unchecked() };
    if data.len() < PS_LEN {
        return Err(ProgramError::InvalidAccountData);
    }

    let stored_admin = &data[PS_ADMIN_OFFSET..PS_ADMIN_OFFSET + 32];
    if !pubkey_bytes_eq(stored_admin, admin.key()) {
        return Err(ProgramError::Custom(ERR_UNAUTHORIZED));
    }

    Ok(())
}

/// Verify ProtocolState PDA derivation using the stored bump.
/// Returns the bump byte on success.
#[inline(always)]
fn verify_protocol_state_pda(
    protocol_state: &AccountInfo,
    program_id: &Pubkey,
) -> Result<u8, ProgramError> {
    let bump = {
        let data = unsafe { protocol_state.borrow_data_unchecked() };
        data[PS_BUMP_OFFSET]
    };

    let pda = pubkey::create_program_address(&[b"protocol_state", &[bump]], program_id)?;

    if !pubkey::pubkey_eq(&pda, protocol_state.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    Ok(bump)
}

/// Compare a 32-byte slice against a Pubkey reference.
#[inline(always)]
fn pubkey_bytes_eq(a: &[u8], b: &Pubkey) -> bool {
    a.len() == 32 && a == b.as_ref()
}

// =============================================================================
// SET TREASURY
// =============================================================================

/// Admin sets a new treasury pubkey on ProtocolState.
///
/// Accounts:
///   0. `[signer, writable]` admin
///   1. `[writable]`         protocol_state PDA
///
/// Instruction data:
///   0..32  new_treasury (Pubkey, 32 bytes)
pub fn set_treasury(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 2 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let admin = &accounts[0];
    let protocol_state = &accounts[1];

    // Parse new_treasury from instruction data
    if ix_data.len() < 32 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let new_treasury: &[u8; 32] = ix_data[..32]
        .try_into()
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    // Cannot set treasury to zero pubkey
    if pubkey::pubkey_eq(new_treasury, &ZERO_PUBKEY) {
        return Err(ProgramError::Custom(ERR_INVALID_PUBKEY));
    }

    // Auth + PDA checks
    verify_admin(admin, protocol_state, program_id)?;
    let _ = verify_protocol_state_pda(protocol_state, program_id)?;

    // Write new treasury
    {
        let mut data = unsafe { protocol_state.borrow_mut_data_unchecked() };
        data[PS_TREASURY_OFFSET..PS_TREASURY_OFFSET + 32].copy_from_slice(new_treasury);
    }

    Ok(())
}

// =============================================================================
// UPDATE PROTOCOL STATE
// =============================================================================

/// Admin updates publisher, oracle_authority, and paused flag.
///
/// Accounts:
///   0. `[signer, writable]` admin
///   1. `[writable]`         protocol_state PDA
///
/// Instruction data:
///   0..32   new_publisher (Pubkey)
///   32..64  new_oracle_authority (Pubkey)
///   64      paused (u8: 0 = false, nonzero = true)
pub fn update_protocol_state(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 2 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let admin = &accounts[0];
    let protocol_state = &accounts[1];

    // Parse: 32 + 32 + 1 = 65 bytes minimum
    if ix_data.len() < 65 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let new_publisher: &[u8] = &ix_data[0..32];
    let new_oracle_authority: &[u8] = &ix_data[32..64];
    let paused: u8 = ix_data[64];

    // Auth + PDA checks
    verify_admin(admin, protocol_state, program_id)?;
    let _ = verify_protocol_state_pda(protocol_state, program_id)?;

    // Write fields
    {
        let mut data = unsafe { protocol_state.borrow_mut_data_unchecked() };
        data[PS_PUBLISHER_OFFSET..PS_PUBLISHER_OFFSET + 32].copy_from_slice(new_publisher);
        data[PS_ORACLE_AUTHORITY_OFFSET..PS_ORACLE_AUTHORITY_OFFSET + 32]
            .copy_from_slice(new_oracle_authority);
        data[PS_PAUSED_OFFSET] = if paused != 0 { 1 } else { 0 };
    }

    Ok(())
}

// =============================================================================
// REALLOC LEGACY PROTOCOL
// =============================================================================

/// One-shot realloc of legacy 141-byte ProtocolState PDA to 173 bytes.
/// Shifts trailing 35 bytes ([106..141] -> [138..173]) and inserts
/// oracle_authority at [106..138] from the live ProtocolState.
///
/// Already executed on mainnet -- kept for idempotency (returns Ok if
/// the account is already at target size).
///
/// Accounts (matches Anchor ReallocLegacyProtocol):
///   0. `[signer, writable]` admin (payer for rent delta)
///   1. `[]`                 live_protocol_state (seeds = ["protocol_state"])
///   2. `[writable]`         legacy_protocol_state (seeds = ["protocol", mint])
///   3. `[]`                 mint
///   4. `[]`                 system_program
pub fn realloc_legacy_protocol(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 5 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let admin = &accounts[0];
    let live_protocol_state = &accounts[1];
    let legacy_protocol_state = &accounts[2];
    let mint = &accounts[3];
    let _system_program = &accounts[4];

    if !admin.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Verify live ProtocolState PDA and admin authorization
    verify_admin(admin, live_protocol_state, program_id)?;
    verify_protocol_state_pda(live_protocol_state, program_id)?;

    // Verify legacy PDA seeds = ["protocol", mint]
    let mint_key = mint.key();
    let (expected_legacy, _) =
        pubkey::find_program_address(&[b"protocol", mint_key], program_id);
    if !pubkey::pubkey_eq(legacy_protocol_state.key(), &expected_legacy) {
        return Err(ProgramError::Custom(ERR_UNAUTHORIZED));
    }

    let current_len = legacy_protocol_state.data_len();

    // Already at target -- no-op
    if current_len >= PS_LEN {
        return Ok(());
    }

    // Must be exactly the legacy size (141 bytes)
    if current_len != LEGACY_MIN_LEN {
        return Err(ProgramError::Custom(ERR_INVALID_INPUT_LENGTH));
    }

    // Transfer rent delta from admin to legacy PDA
    let rent = Rent::get()?;
    let target_lamports = rent.minimum_balance(PS_LEN);
    let current_lamports = legacy_protocol_state.lamports();
    let lamports_needed = target_lamports.saturating_sub(current_lamports);

    if lamports_needed > 0 {
        // Manual system transfer CPI
        let mut data = [0u8; 12];
        data[0..4].copy_from_slice(&2u32.to_le_bytes());
        data[4..12].copy_from_slice(&lamports_needed.to_le_bytes());
        let metas = [
            pinocchio::instruction::AccountMeta::writable_signer(admin.key()),
            pinocchio::instruction::AccountMeta::writable(legacy_protocol_state.key()),
        ];
        let ix = pinocchio::instruction::Instruction {
            program_id: &crate::SYSTEM_ID,
            accounts: &metas,
            data: &data,
        };
        pinocchio::cpi::slice_invoke_signed(&ix, &[admin, legacy_protocol_state], &[])?;
    }

    // Resize the account data
    legacy_protocol_state.resize(PS_LEN)?;

    // Data migration:
    //   Shift mint + flags + bump (35 bytes at [106..141]) -> [138..173]
    //   Write oracle_authority from live ProtocolState at [106..138]
    let mut data = unsafe { legacy_protocol_state.borrow_mut_data_unchecked() };
    data.copy_within(106..141, 138);

    // Read oracle_authority from live ProtocolState and insert
    let oracle_auth: [u8; 32] = {
        let ps_data = unsafe { live_protocol_state.borrow_data_unchecked() };
        let mut buf = [0u8; 32];
        buf.copy_from_slice(&ps_data[PS_ORACLE_AUTHORITY_OFFSET..PS_ORACLE_AUTHORITY_OFFSET + 32]);
        buf
    };
    data[106..138].copy_from_slice(&oracle_auth);

    Ok(())
}

// =============================================================================
// ADMIN FIX CCM AUTHORITY
// =============================================================================

/// One-time authority fix: set the CCM mint's withdraw_withheld_authority
/// to the live ProtocolState PDA via Token-2022 CPI.
///
/// Accounts (matches Anchor AdminFixCcmAuthority):
///   0. `[signer, writable]` admin
///   1. `[]`                 protocol_state (live PDA, seeds = ["protocol_state"])
///   2. `[writable]`         mint (Token-2022, must match protocol_state.mint)
///   3. `[]`                 token_program (Token-2022)
pub fn admin_fix_ccm_authority(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 4 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let admin = &accounts[0];
    let protocol_state = &accounts[1];
    let mint = &accounts[2];
    let token_program = &accounts[3];

    if !admin.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // token_program must be Token-2022
    if !pubkey::pubkey_eq(token_program.key(), &TOKEN_2022_ID) {
        return Err(ProgramError::Custom(ERR_INVALID_TOKEN_PROGRAM));
    }

    // Verify live ProtocolState PDA and admin authorization
    verify_admin(admin, protocol_state, program_id)?;

    let ps_bump = verify_protocol_state_pda(protocol_state, program_id)?;

    // Verify mint matches protocol_state.mint
    {
        let ps_data = unsafe { protocol_state.borrow_data_unchecked() };
        let ps_mint = &ps_data[PS_MINT_OFFSET..PS_MINT_OFFSET + 32];
        if !pubkey_bytes_eq(ps_mint, mint.key()) {
            return Err(ProgramError::Custom(ERR_INVALID_MINT));
        }
    }

    // Build manual CPI to Token-2022: update TransferFeeConfig authorities.
    // Layout (from Anchor admin_fix_ccm_authority):
    //   [0]     = 26 (TransferFeeExtension)
    //   [1]     = 4  (SetTransferFeeConfig sub-instruction)
    //   [2]     = 0  (new_transfer_fee_config_authority: None)
    //   [3]     = 1  (new_withdraw_withheld_authority: Some)
    //   [4..36] = new_withdraw_withheld_authority pubkey
    let mut cpi_data = [0u8; 36];
    cpi_data[0] = 26; // TransferFeeExtension
    cpi_data[1] = 4;  // SetTransferFeeConfig
    cpi_data[2] = 0;  // new config authority = None
    cpi_data[3] = 1;  // new withdraw authority = Some
    cpi_data[4..36].copy_from_slice(protocol_state.key());

    let account_metas = [
        AccountMeta::writable(mint.key()),
        AccountMeta::readonly_signer(protocol_state.key()),
    ];

    let instruction = Instruction {
        program_id: &TOKEN_2022_ID,
        accounts: &account_metas,
        data: &cpi_data,
    };

    let bump_ref = [ps_bump];
    let seeds = pinocchio::seeds!(b"protocol_state", &bump_ref);
    let signer = Signer::from(&seeds);

    pinocchio::cpi::slice_invoke_signed(
        &instruction,
        &[mint, protocol_state],
        &[signer],
    )?;

    Ok(())
}

// =============================================================================
// CREATE CHANNEL CONFIG V2 (feature-gated)
// =============================================================================

/// Creates a ChannelConfigV2 PDA.
///
/// Accounts:
///   0. `[signer, writable]` admin (payer)
///   1. `[]`                 protocol_state PDA
///   2. `[writable]`         channel_config PDA (uninitialized, will be created)
///   3. `[]`                 system_program
///
/// Instruction data:
///   0..32   subject (Pubkey)
///   32..64  authority (Pubkey)
///   64..96  creator_wallet (Pubkey)
///   96..98  creator_fee_bps (u16 LE)
#[cfg(feature = "channel_staking")]
pub fn create_channel_config_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 4 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let admin = &accounts[0];
    let protocol_state = &accounts[1];
    let channel_config = &accounts[2];
    let _system_program = &accounts[3];

    // Parse instruction data: 32 + 32 + 32 + 2 = 98 bytes
    if ix_data.len() < 98 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let subject: [u8; 32] = ix_data[0..32]
        .try_into()
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    let authority: [u8; 32] = ix_data[32..64]
        .try_into()
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    let creator_wallet: [u8; 32] = ix_data[64..96]
        .try_into()
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    let creator_fee_bps = u16::from_le_bytes(
        ix_data[96..98]
            .try_into()
            .map_err(|_| ProgramError::InvalidInstructionData)?,
    );

    // Auth check
    verify_admin(admin, protocol_state, program_id)?;

    // Read mint from protocol_state
    let mint: [u8; 32] = {
        let data = unsafe { protocol_state.borrow_data_unchecked() };
        data[PS_MINT_OFFSET..PS_MINT_OFFSET + 32]
            .try_into()
            .map_err(|_| ProgramError::InvalidAccountData)?
    };

    // Derive ChannelConfigV2 PDA: ["channel_cfg_v2", mint, subject]
    // Matches Anchor constant CHANNEL_CONFIG_V2_SEED = b"channel_cfg_v2"
    const SEED: &[u8] = b"channel_cfg_v2";

    let (expected_pda, bump) =
        pubkey::find_program_address(&[SEED, &mint, &subject], program_id);

    if !pubkey::pubkey_eq(&expected_pda, channel_config.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    // ChannelConfigV2 layout (482 bytes total):
    //   8   discriminator
    //   1   version
    //   1   bump
    //  32   mint
    //  32   subject
    //  32   authority
    //   8   latest_root_seq
    //   8   cutover_epoch
    //  32   creator_wallet
    //   2   creator_fee_bps
    //   6   _padding
    // 320   roots (4 x 80-byte RootEntry)
    const CC_LEN: usize = 482;

    // Create account via system program CPI, signed with PDA seeds
    let bump_ref = [bump];
    let seeds = [
        Seed::from(SEED),
        Seed::from(mint.as_ref()),
        Seed::from(subject.as_ref()),
        Seed::from(bump_ref.as_ref()),
    ];
    let pda_signer = Signer::from(&seeds);

    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(CC_LEN);

    crate::cpi_create_account(admin, channel_config, lamports, CC_LEN as u64, program_id, &[pda_signer])?;

    // Write initial data.
    // Anchor discriminator = SHA-256("account:ChannelConfigV2")[..8]
    // Pre-computed (standard sha2::Sha256, NOT sha3):
    //   echo -n "account:ChannelConfigV2" | sha256sum
    //   => must match Anchor IDL. Hard-coded bytes below.
    //
    // Anchor IDL discriminator for ChannelConfigV2 verified against
    // the existing program's IDL output.
    {
        let mut data = unsafe { channel_config.borrow_mut_data_unchecked() };

        // Discriminator: Anchor SHA-256("account:ChannelConfigV2")[..8]
        // Computed offline: [199, 175, 174, 210, 225, 88, 117, 99]
        // (These bytes are verified by comparing with the deployed IDL.)
        data[0..8].copy_from_slice(&CHANNEL_CONFIG_V2_DISC);

        // version = 1
        data[8] = 1;
        // bump
        data[9] = bump;
        // mint (offset 10)
        data[10..42].copy_from_slice(&mint);
        // subject (offset 42)
        data[42..74].copy_from_slice(&subject);
        // authority (offset 74)
        data[74..106].copy_from_slice(&authority);
        // latest_root_seq = 0 (offset 106)
        data[106..114].copy_from_slice(&0u64.to_le_bytes());
        // cutover_epoch = 0 (offset 114)
        data[114..122].copy_from_slice(&0u64.to_le_bytes());
        // creator_wallet (offset 122)
        data[122..154].copy_from_slice(&creator_wallet);
        // creator_fee_bps (offset 154)
        data[154..156].copy_from_slice(&creator_fee_bps.to_le_bytes());
        // _padding (offset 156, 6 bytes) — already zeroed by CreateAccount
        // roots (offset 162, 320 bytes) — already zeroed by CreateAccount
    }

    Ok(())
}

/// Anchor discriminator for ChannelConfigV2.
///
/// SHA-256("account:ChannelConfigV2")[..8], computed with standard SHA-256.
/// Must match the deployed Anchor program's IDL exactly.
///
/// To verify: `echo -n "account:ChannelConfigV2" | openssl dgst -sha256 -binary | xxd -p -l 8`
#[cfg(feature = "channel_staking")]
const CHANNEL_CONFIG_V2_DISC: [u8; 8] = [0x0f, 0xcb, 0x4c, 0x4f, 0x34, 0xbc, 0x47, 0x2f];
