//! Governance instructions: harvest_fees, withdraw_fees_from_mint, route_treasury.
//!
//! All three operate on the legacy ProtocolState PDA (seeds = [b"protocol", mint])
//! which is the CCM mint's withdraw_withheld_authority. Token-2022 transfer fee
//! CPIs are built manually since pinocchio-token does not ship extension helpers.

use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Signer},
    program_error::ProgramError,
    pubkey::{create_program_address, find_program_address, pubkey_eq, Pubkey},
    ProgramResult,
};

// ============================================================================
// Constants
// ============================================================================

/// Token-2022 program ID.
use crate::TOKEN_2022_ID;

/// PDA seed for legacy ProtocolState.
const PROTOCOL_SEED: &[u8] = b"protocol";

/// Byte offset of the treasury pubkey in the legacy ProtocolState PDA (post-realloc).
const LEGACY_TREASURY_OFFSET: usize = 74;

/// Byte offset of the bump in the legacy ProtocolState PDA (post-realloc).
const LEGACY_BUMP_OFFSET: usize = 172;

/// Minimum account data length for the legacy ProtocolState PDA (post-realloc).
const LEGACY_MIN_LEN: usize = 173;

/// ProtocolState byte offsets (live PDA, seeds = ["protocol_state"]).
const PS_ADMIN_OFFSET: usize = 10;
const PS_ORACLE_AUTHORITY_OFFSET: usize = 106;
const PS_MINT_OFFSET: usize = 138;
const PS_PAUSED_OFFSET: usize = 170;
const PS_BUMP_OFFSET: usize = 172;

/// Token-2022 TokenInstruction::TransferFeeExtension discriminator.
const TRANSFER_FEE_EXTENSION: u8 = 26;

/// TransferFeeInstruction::WithdrawWithheldTokensFromMint sub-instruction.
const WITHDRAW_FROM_MINT_SUB_IX: u8 = 2;

/// TransferFeeInstruction::WithdrawWithheldTokensFromAccounts sub-instruction.
const WITHDRAW_FROM_ACCOUNTS_SUB_IX: u8 = 3;

/// Maximum remaining accounts for harvest_fees batch.
const MAX_HARVEST_SOURCES: usize = 30;

// ============================================================================
// Error helpers
// ============================================================================

/// Map to Anchor-compatible error codes (6000 + index).
/// These must match the error enum in error.rs.
const ERR_UNAUTHORIZED: u32 = 6000;
const ERR_PROTOCOL_PAUSED: u32 = 6002;
const ERR_INVALID_INPUT_LENGTH: u32 = 6040;
const ERR_INVALID_MINT: u32 = 6016;
const ERR_INVALID_TOKEN_PROGRAM: u32 = 6019;
const ERR_INSUFFICIENT_TREASURY_BALANCE: u32 = 6020;

// ============================================================================
// Helpers
// ============================================================================

/// Read a 32-byte pubkey from account data at the given offset.
#[inline(always)]
fn read_pubkey(data: &[u8], offset: usize) -> Pubkey {
    let mut key = [0u8; 32];
    key.copy_from_slice(&data[offset..offset + 32]);
    key
}

/// Read a u64 from account data at the given offset (little-endian).
#[inline(always)]
fn read_u64_le(data: &[u8], offset: usize) -> u64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&data[offset..offset + 8]);
    u64::from_le_bytes(buf)
}

/// Read the CCM mint decimals from the mint account.
/// SPL Mint layout: COption<Pubkey>(36) + supply(8) + decimals(1) => offset 44.
#[inline(always)]
fn read_mint_decimals(mint_data: &[u8]) -> u8 {
    mint_data[44]
}

/// Read the token account amount from a Token-2022 token account.
/// Token account layout: offset 64 = amount (u64 LE).
#[inline(always)]
fn read_token_amount(data: &[u8]) -> u64 {
    read_u64_le(data, 64)
}

/// Read the token account mint from a Token-2022 token account.
/// Token account layout: offset 0 = mint (Pubkey).
#[inline(always)]
fn read_token_mint(data: &[u8]) -> Pubkey {
    read_pubkey(data, 0)
}

/// Read the token account owner from a Token-2022 token account.
/// Token account layout: offset 32 = owner (Pubkey).
#[inline(always)]
fn read_token_owner(data: &[u8]) -> Pubkey {
    read_pubkey(data, 32)
}

// ============================================================================
// 1. harvest_fees
// ============================================================================
//
// Harvest withheld Token-2022 transfer fees from remaining_accounts to the
// treasury ATA. Uses WithdrawWithheldTokensFromAccounts (requires the PDA
// as withdraw_withheld_authority to sign).
//
// Accounts:
//   0. [SIGNER, WRITE] authority (permissionless payer)
//   1. []               protocol_state (legacy PDA, UncheckedAccount)
//   2. [WRITE]          mint (Token-2022)
//   3. [WRITE]          treasury (Token-2022 ATA)
//   4. []               token_program (Token-2022)
//   5..N [WRITE]        remaining source token accounts

pub fn harvest_fees(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _ix_data: &[u8],
) -> ProgramResult {
    // Need at least 5 fixed accounts + 1 remaining
    if accounts.len() < 6 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let authority = &accounts[0];
    let protocol_state = &accounts[1];
    let mint = &accounts[2];
    let treasury = &accounts[3];
    let token_program = &accounts[4];
    let remaining = &accounts[5..];

    // authority must be signer
    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // token_program must be Token-2022
    if !pubkey_eq(token_program.key(), &TOKEN_2022_ID) {
        return Err(ProgramError::Custom(ERR_INVALID_TOKEN_PROGRAM));
    }

    // Parse legacy PDA data
    let protocol_data = unsafe { protocol_state.borrow_data_unchecked() };
    if protocol_data.len() < LEGACY_MIN_LEN {
        return Err(ProgramError::Custom(ERR_INVALID_INPUT_LENGTH));
    }

    let treasury_pubkey = read_pubkey(&protocol_data, LEGACY_TREASURY_OFFSET);
    let bump = protocol_data[LEGACY_BUMP_OFFSET];
    drop(protocol_data);

    // Verify PDA address
    let mint_key = mint.key();
    let (expected_pda, _) = find_program_address(
        &[PROTOCOL_SEED, mint_key],
        program_id,
    );
    if !pubkey_eq(protocol_state.key(), &expected_pda) {
        return Err(ProgramError::Custom(ERR_UNAUTHORIZED));
    }

    // Verify treasury ATA owner matches protocol treasury
    {
        let treasury_data = unsafe { treasury.borrow_data_unchecked() };
        if treasury_data.len() < 72 {
            return Err(ProgramError::Custom(ERR_INVALID_TOKEN_PROGRAM));
        }
        let treasury_mint = read_token_mint(&treasury_data);
        if !pubkey_eq(&treasury_mint, mint_key) {
            return Err(ProgramError::Custom(ERR_INVALID_MINT));
        }
        let treasury_owner = read_token_owner(&treasury_data);
        if !pubkey_eq(&treasury_owner, &treasury_pubkey) {
            return Err(ProgramError::Custom(ERR_UNAUTHORIZED));
        }
    }

    // Validate remaining accounts
    if remaining.is_empty() {
        return Err(ProgramError::Custom(ERR_INVALID_INPUT_LENGTH));
    }
    if remaining.len() > MAX_HARVEST_SOURCES {
        return Err(ProgramError::Custom(ERR_INVALID_INPUT_LENGTH));
    }

    // Verify each source account is a Token-2022 account for this mint
    for source in remaining.iter() {
        if !pubkey_eq(source.owner(), &TOKEN_2022_ID) {
            return Err(ProgramError::Custom(ERR_INVALID_TOKEN_PROGRAM));
        }
        let data = unsafe { source.borrow_data_unchecked() };
        if data.len() < 32 {
            return Err(ProgramError::Custom(ERR_INVALID_TOKEN_PROGRAM));
        }
        let account_mint = read_pubkey(&data, 0);
        if !pubkey_eq(&account_mint, mint_key) {
            return Err(ProgramError::Custom(ERR_INVALID_MINT));
        }
    }

    // Build CPI: WithdrawWithheldTokensFromAccounts
    // Instruction data: [26, 3] (TransferFeeExtension, WithdrawWithheldTokensFromAccounts)
    // Accounts: [mint(W), destination(W), authority(S), ...sources(W)]
    let cpi_ix_data: [u8; 2] = [TRANSFER_FEE_EXTENSION, WITHDRAW_FROM_ACCOUNTS_SUB_IX];

    // Build account metas dynamically (3 fixed + N sources).
    // Use MaybeUninit properly -- only read initialized slots.
    let n_accounts = 3 + remaining.len();

    let mut account_metas_buf: [core::mem::MaybeUninit<AccountMeta>; 33] = unsafe {
        core::mem::MaybeUninit::uninit().assume_init()
    };
    account_metas_buf[0].write(AccountMeta::writable(mint.key()));
    account_metas_buf[1].write(AccountMeta::writable(treasury.key()));
    account_metas_buf[2].write(AccountMeta::readonly_signer(protocol_state.key()));
    for (i, source) in remaining.iter().enumerate() {
        account_metas_buf[3 + i].write(AccountMeta::writable(source.key()));
    }

    // SAFETY: We initialized exactly n_accounts elements above.
    let account_metas = unsafe {
        core::slice::from_raw_parts(
            account_metas_buf.as_ptr() as *const AccountMeta,
            n_accounts,
        )
    };

    let instruction = Instruction {
        program_id: &TOKEN_2022_ID,
        accounts: account_metas,
        data: &cpi_ix_data,
    };

    // Build account infos slice for CPI.
    // Use MaybeUninit for the reference array since we cannot have a const
    // for &AccountInfo (requires lifetime).
    let mut account_refs_buf: [core::mem::MaybeUninit<&AccountInfo>; 33] = unsafe {
        core::mem::MaybeUninit::uninit().assume_init()
    };
    account_refs_buf[0].write(mint);
    account_refs_buf[1].write(treasury);
    account_refs_buf[2].write(protocol_state);
    for (i, source) in remaining.iter().enumerate() {
        account_refs_buf[3 + i].write(source);
    }

    // SAFETY: We initialized exactly n_accounts elements above.
    let account_refs = unsafe {
        core::slice::from_raw_parts(
            account_refs_buf.as_ptr() as *const &AccountInfo,
            n_accounts,
        )
    };

    // PDA signer seeds
    let bump_ref = [bump];
    let seeds = pinocchio::seeds!(PROTOCOL_SEED, mint_key, &bump_ref);
    let signer = Signer::from(&seeds);

    pinocchio::cpi::slice_invoke_signed(
        &instruction,
        account_refs,
        &[signer],
    )?;


    Ok(())
}

// ============================================================================
// 2. withdraw_fees_from_mint
// ============================================================================
//
// Withdraw accumulated mint-level fees to treasury ATA.
// Uses WithdrawWithheldTokensFromMint. Permissionless.
//
// Accounts:
//   0. [SIGNER, WRITE] authority (permissionless payer)
//   1. []               protocol_state (legacy PDA, UncheckedAccount)
//   2. [WRITE]          mint (Token-2022)
//   3. [WRITE]          treasury_ata (Token-2022 ATA)
//   4. []               token_program (Token-2022)

pub fn withdraw_fees_from_mint(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 5 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let authority = &accounts[0];
    let protocol_state = &accounts[1];
    let mint = &accounts[2];
    let treasury_ata = &accounts[3];
    let token_program = &accounts[4];

    // authority must be signer
    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // token_program must be Token-2022
    if !pubkey_eq(token_program.key(), &TOKEN_2022_ID) {
        return Err(ProgramError::Custom(ERR_INVALID_TOKEN_PROGRAM));
    }

    // Parse legacy PDA data
    let protocol_data = unsafe { protocol_state.borrow_data_unchecked() };
    if protocol_data.len() < LEGACY_MIN_LEN {
        return Err(ProgramError::Custom(ERR_INVALID_INPUT_LENGTH));
    }

    let treasury_pubkey = read_pubkey(&protocol_data, LEGACY_TREASURY_OFFSET);
    let bump = protocol_data[LEGACY_BUMP_OFFSET];
    drop(protocol_data);

    // Verify PDA address
    let mint_key = mint.key();
    let (expected_pda, _) = find_program_address(
        &[PROTOCOL_SEED, mint_key],
        program_id,
    );
    if !pubkey_eq(protocol_state.key(), &expected_pda) {
        return Err(ProgramError::Custom(ERR_UNAUTHORIZED));
    }

    // Verify treasury ATA
    {
        let ata_data = unsafe { treasury_ata.borrow_data_unchecked() };
        if ata_data.len() < 72 {
            return Err(ProgramError::Custom(ERR_INVALID_TOKEN_PROGRAM));
        }
        let ata_mint = read_token_mint(&ata_data);
        if !pubkey_eq(&ata_mint, mint_key) {
            return Err(ProgramError::Custom(ERR_INVALID_MINT));
        }
        let ata_owner = read_token_owner(&ata_data);
        if !pubkey_eq(&ata_owner, &treasury_pubkey) {
            return Err(ProgramError::Custom(ERR_UNAUTHORIZED));
        }
    }

    // Build CPI: WithdrawWithheldTokensFromMint
    // Instruction data: [26, 2]
    // Accounts: [mint(W), destination(W), authority(S)]
    let ix_data: [u8; 2] = [TRANSFER_FEE_EXTENSION, WITHDRAW_FROM_MINT_SUB_IX];

    let account_metas = [
        AccountMeta::writable(mint.key()),
        AccountMeta::writable(treasury_ata.key()),
        AccountMeta::readonly_signer(protocol_state.key()),
    ];

    let instruction = Instruction {
        program_id: &TOKEN_2022_ID,
        accounts: &account_metas,
        data: &ix_data,
    };

    let bump_ref = [bump];
    let seeds = pinocchio::seeds!(PROTOCOL_SEED, mint_key, &bump_ref);
    let signer = Signer::from(&seeds);

    pinocchio::cpi::slice_invoke_signed(
        &instruction,
        &[mint, treasury_ata, protocol_state],
        &[signer],
    )?;


    Ok(())
}

// ============================================================================
// 3. route_treasury
// ============================================================================
//
// Transfer CCM from treasury ATA to destination (vault buffer) ATA.
// Requires oracle_authority OR admin signer.
// Reads the live ProtocolState PDA (seeds = ["protocol_state"], 173 bytes).
//
// Accounts:
//   0. [SIGNER, WRITE] admin (admin or oracle_authority)
//   1. []               protocol_state (live PDA, seeds = ["protocol_state"])
//   2. []               mint (Token-2022)
//   3. [WRITE]          treasury_ata
//   4. [WRITE]          destination_ata
//   5. []               token_program (Token-2022)
//
// Instruction data:
//   [0..8]  amount: u64 (LE)
//   [8..16] min_reserve: u64 (LE)

pub fn route_treasury(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 6 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    if ix_data.len() < 16 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let admin = &accounts[0];
    let protocol_state = &accounts[1];
    let mint = &accounts[2];
    let treasury_ata = &accounts[3];
    let destination_ata = &accounts[4];
    let token_program = &accounts[5];

    // Parse instruction data
    let amount = read_u64_le(ix_data, 0);
    let min_reserve = read_u64_le(ix_data, 8);

    // admin must be signer
    if !admin.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // token_program must be Token-2022
    if !pubkey_eq(token_program.key(), &TOKEN_2022_ID) {
        return Err(ProgramError::Custom(ERR_INVALID_TOKEN_PROGRAM));
    }

    // Verify live ProtocolState PDA
    let ps_data = unsafe { protocol_state.borrow_data_unchecked() };
    if ps_data.len() < LEGACY_MIN_LEN {
        return Err(ProgramError::Custom(ERR_INVALID_INPUT_LENGTH));
    }

    let ps_admin = read_pubkey(&ps_data, PS_ADMIN_OFFSET);
    let ps_oracle = read_pubkey(&ps_data, PS_ORACLE_AUTHORITY_OFFSET);
    let ps_mint = read_pubkey(&ps_data, PS_MINT_OFFSET);
    let ps_paused = ps_data[PS_PAUSED_OFFSET];
    let ps_bump = ps_data[PS_BUMP_OFFSET];
    drop(ps_data);

    // Verify PDA: legacy seeds = ["protocol", mint, bump]
    let bump_ref = &[ps_bump];
    let mint_key_ref = mint.key();
    let expected_pda = create_program_address(
        &[PROTOCOL_SEED, mint_key_ref, bump_ref],
        program_id,
    )?;
    if !pubkey_eq(protocol_state.key(), &expected_pda) {
        return Err(ProgramError::Custom(ERR_UNAUTHORIZED));
    }

    // admin must be admin OR oracle_authority
    if !pubkey_eq(admin.key(), &ps_admin) && !pubkey_eq(admin.key(), &ps_oracle) {
        return Err(ProgramError::Custom(ERR_UNAUTHORIZED));
    }

    // Verify mint matches
    if !pubkey_eq(mint.key(), &ps_mint) {
        return Err(ProgramError::Custom(ERR_INVALID_MINT));
    }

    // Protocol must not be paused
    if ps_paused != 0 {
        return Err(ProgramError::Custom(ERR_PROTOCOL_PAUSED));
    }

    // Validate amounts
    if amount == 0 {
        return Err(ProgramError::Custom(ERR_INVALID_INPUT_LENGTH));
    }
    if min_reserve == 0 {
        return Err(ProgramError::Custom(ERR_INVALID_INPUT_LENGTH));
    }

    // Verify treasury_ata
    let mint_key = mint.key();
    {
        let ata_data = unsafe { treasury_ata.borrow_data_unchecked() };
        if ata_data.len() < 72 {
            return Err(ProgramError::Custom(ERR_INVALID_TOKEN_PROGRAM));
        }
        let ata_mint = read_token_mint(&ata_data);
        if !pubkey_eq(&ata_mint, mint_key) {
            return Err(ProgramError::Custom(ERR_INVALID_MINT));
        }
        // treasury_ata must be owned by the protocol_state PDA
        let ata_owner = read_token_owner(&ata_data);
        if !pubkey_eq(&ata_owner, protocol_state.key()) {
            return Err(ProgramError::Custom(ERR_UNAUTHORIZED));
        }

        // Check balance
        let treasury_balance = read_token_amount(&ata_data);
        let balance_after = treasury_balance
            .checked_sub(amount)
            .ok_or(ProgramError::Custom(ERR_INSUFFICIENT_TREASURY_BALANCE))?;
        if balance_after < min_reserve {
            return Err(ProgramError::Custom(ERR_INSUFFICIENT_TREASURY_BALANCE));
        }
    }

    // Verify destination_ata mint
    {
        let dest_data = unsafe { destination_ata.borrow_data_unchecked() };
        if dest_data.len() < 72 {
            return Err(ProgramError::Custom(ERR_INVALID_TOKEN_PROGRAM));
        }
        let dest_mint = read_token_mint(&dest_data);
        if !pubkey_eq(&dest_mint, mint_key) {
            return Err(ProgramError::Custom(ERR_INVALID_MINT));
        }
    }

    // Read mint decimals
    let decimals = {
        let mint_data = unsafe { mint.borrow_data_unchecked() };
        read_mint_decimals(&mint_data)
    };

    // Build CPI: TransferChecked (Token-2022)
    // Instruction data: [12, amount(8), decimals(1)]
    let mut cpi_data = [0u8; 10];
    cpi_data[0] = 12; // TransferChecked discriminator
    cpi_data[1..9].copy_from_slice(&amount.to_le_bytes());
    cpi_data[9] = decimals;

    let account_metas = [
        AccountMeta::writable(treasury_ata.key()),
        AccountMeta::readonly(mint.key()),
        AccountMeta::writable(destination_ata.key()),
        AccountMeta::readonly_signer(protocol_state.key()),
    ];

    let instruction = Instruction {
        program_id: &TOKEN_2022_ID,
        accounts: &account_metas,
        data: &cpi_data,
    };

    let ps_bump_ref = [ps_bump];
    let seeds = pinocchio::seeds!(PROTOCOL_SEED, mint_key_ref, &ps_bump_ref);
    let signer = Signer::from(&seeds);

    pinocchio::cpi::slice_invoke_signed(
        &instruction,
        &[treasury_ata, mint, destination_ata, protocol_state],
        &[signer],
    )?;


    Ok(())
}
