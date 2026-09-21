//! Ledger lifecycle: create one ledger per leaf domain, rotate its authority.

use pinocchio::{account_info::AccountInfo, instruction::Seed, pubkey, ProgramResult};

use super::common::{create_pda, expect_signer, expect_system_program, is_default_pubkey, read_array32};
use crate::error::LedgerError;
use crate::keccak::keccak256;
use crate::state::{
    cast_account, cast_account_mut, Ledger, DISC_LEDGER, LEDGER_SEED, LEDGER_VERSION,
    SCHEME_RFC9162, SCHEME_SORTED_PAIR,
};

pub const MAX_DOMAIN_LEN: usize = 64;

/// Accounts: [payer (signer, writable), ledger PDA (writable), system program]
/// Data:     [authority: 32][trusted_signer: 32][scheme: u8][log_id_len: u8][log_id: UTF-8, 1..=64]
///
/// The ledger is keyed by keccak256(log_id) so the chain, not the caller,
/// binds it to the log / domain string. `trusted_signer` is the Ed25519 key
/// scheme-2 heads must be signed by (required non-zero for scheme 2) and is
/// immutable for the life of the ledger.
pub fn init_ledger(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let [payer, ledger_ai, system] = accounts else {
        return Err(pinocchio::program_error::ProgramError::NotEnoughAccountKeys);
    };
    expect_signer(payer)?;
    expect_system_program(system)?;

    let authority = read_array32(data, 0)?;
    if is_default_pubkey(&authority) {
        return Err(LedgerError::InvalidPubkey.into());
    }
    let trusted_signer = read_array32(data, 32)?;
    let scheme = *data
        .get(64)
        .ok_or(pinocchio::program_error::ProgramError::InvalidInstructionData)?;
    if scheme != SCHEME_SORTED_PAIR && scheme != SCHEME_RFC9162 {
        return Err(pinocchio::program_error::ProgramError::InvalidInstructionData);
    }
    if scheme == SCHEME_RFC9162 && is_default_pubkey(&trusted_signer) {
        return Err(LedgerError::InvalidPubkey.into());
    }
    let dlen = *data
        .get(65)
        .ok_or(pinocchio::program_error::ProgramError::InvalidInstructionData)?
        as usize;
    if dlen == 0 || dlen > MAX_DOMAIN_LEN {
        return Err(pinocchio::program_error::ProgramError::InvalidInstructionData);
    }
    let domain = data
        .get(66..66 + dlen)
        .ok_or(pinocchio::program_error::ProgramError::InvalidInstructionData)?;
    let domain_hash = keccak256(&[domain]);

    let (expected, bump) = pubkey::find_program_address(&[LEDGER_SEED, &domain_hash], &crate::ID);
    if !pubkey::pubkey_eq(ledger_ai.key(), &expected) {
        return Err(pinocchio::program_error::ProgramError::InvalidSeeds);
    }
    let bump_bytes = [bump];
    let seeds = [
        Seed::from(LEDGER_SEED),
        Seed::from(domain_hash.as_ref()),
        Seed::from(bump_bytes.as_ref()),
    ];
    create_pda(payer, ledger_ai, &seeds, Ledger::LEN)?;

    let l = cast_account_mut::<Ledger>(ledger_ai)?;
    l.discriminator = DISC_LEDGER;
    l.version = LEDGER_VERSION;
    l.bump = bump;
    l.scheme = scheme;
    l.authority = authority;
    l.log_id_hash = domain_hash;
    l.trusted_signer = trusted_signer;
    l.last_seq = [0u8; 8];
    l.count = [0u8; 8];
    Ok(())
}

/// Accounts: [current authority (signer), ledger (writable)]
/// Data:     [new_authority: 32]
pub fn set_authority(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let [authority, ledger_ai] = accounts else {
        return Err(pinocchio::program_error::ProgramError::NotEnoughAccountKeys);
    };
    expect_signer(authority)?;
    let new_authority = read_array32(data, 0)?;
    if is_default_pubkey(&new_authority) {
        return Err(LedgerError::InvalidPubkey.into());
    }
    {
        let l = cast_account::<Ledger>(ledger_ai, &DISC_LEDGER)?;
        if !pubkey::pubkey_eq(authority.key(), &l.authority) {
            return Err(LedgerError::Unauthorized.into());
        }
    }
    let l = cast_account_mut::<Ledger>(ledger_ai)?;
    l.authority = new_authority;
    Ok(())
}
