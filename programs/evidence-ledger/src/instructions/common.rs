//! Shared mechanics for instruction handlers: bounds-checked argument
//! readers, account-role checks, and PDA creation via System CreateAccount.
//! `cpi_create_account` is a port of the live AO runtime's helper.

use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Seed, Signer},
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};

#[inline(always)]
pub fn read_u64(data: &[u8], off: usize) -> Result<u64, ProgramError> {
    let b = data
        .get(off..off + 8)
        .ok_or(ProgramError::InvalidInstructionData)?;
    Ok(u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
}

#[inline(always)]
pub fn read_array32(data: &[u8], off: usize) -> Result<[u8; 32], ProgramError> {
    let b = data
        .get(off..off + 32)
        .ok_or(ProgramError::InvalidInstructionData)?;
    let mut out = [0u8; 32];
    out.copy_from_slice(b);
    Ok(out)
}

#[inline(always)]
pub fn expect_signer(ai: &AccountInfo) -> ProgramResult {
    if !ai.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    Ok(())
}

#[inline(always)]
pub fn expect_system_program(ai: &AccountInfo) -> ProgramResult {
    if !pubkey::pubkey_eq(ai.key(), &crate::SYSTEM_ID) {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

#[inline(always)]
pub fn is_default_pubkey(k: &Pubkey) -> bool {
    k.iter().all(|b| *b == 0)
}

/// System program CreateAccount, signed by `signers` (PDA seeds) for `to`.
/// Data layout: [0..4] index 0u32, [4..12] lamports, [12..20] space, [20..52] owner.
#[inline(never)]
pub fn cpi_create_account(
    from: &AccountInfo,
    to: &AccountInfo,
    lamports: u64,
    space: u64,
    owner: &Pubkey,
    signers: &[Signer],
) -> ProgramResult {
    let mut data = [0u8; 52];
    data[4..12].copy_from_slice(&lamports.to_le_bytes());
    data[12..20].copy_from_slice(&space.to_le_bytes());
    data[20..52].copy_from_slice(owner);
    let metas = [
        AccountMeta::writable_signer(from.key()),
        AccountMeta::writable_signer(to.key()),
    ];
    let ix = Instruction {
        program_id: &crate::SYSTEM_ID,
        accounts: &metas,
        data: &data,
    };
    pinocchio::cpi::slice_invoke_signed(&ix, &[from, to], signers)
}

/// Create a rent-exempt, program-owned account at a PDA whose seeds (bump
/// already appended) are `seeds`. Refuses to touch an account that already
/// holds data or lamports, so an init can never silently reinitialize.
#[inline(never)]
pub fn create_pda(
    payer: &AccountInfo,
    target: &AccountInfo,
    seeds: &[Seed],
    space: usize,
) -> ProgramResult {
    if target.data_len() != 0 || target.lamports() != 0 {
        return Err(crate::error::LedgerError::AlreadyInitialized.into());
    }
    let lamports = Rent::get()?.minimum_balance(space);
    let signer = Signer::from(seeds);
    cpi_create_account(payer, target, lamports, space as u64, &crate::ID, &[signer])
}
