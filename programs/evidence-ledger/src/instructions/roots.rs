//! Publish a root (append-only) and verify a leaf against a published root.

use pinocchio::{
    account_info::AccountInfo,
    instruction::Seed,
    program_error::ProgramError,
    pubkey,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

use super::common::{create_pda, expect_signer, expect_system_program, read_array32, read_u64};
use crate::error::LedgerError;
use crate::merkle::{verify_inclusion_rfc9162, verify_proof};
use crate::state::{
    cast_account, cast_account_mut, Ledger, RootEntry, DISC_LEDGER, DISC_ROOT_ENTRY, ROOT_SEED,
    ROOT_ENTRY_VERSION, SCHEME_RFC9162, SCHEME_SORTED_PAIR,
};

/// Accounts: [authority (signer), payer (signer, writable), ledger (writable),
///            root PDA (writable), system program]
/// Data:     [seq: u64][root: 32][manifest_hash: 32][leaf_count: u64]
///
/// Scheme 1 (batch roots) only: a scheme-2 log ledger accepts heads solely
/// through anchor_head, where the off-chain signature is verified, so the
/// authority can never anchor an unsigned head. Append-only by construction:
/// `seq` must be strictly greater than the last one, the root account is a
/// fresh PDA ["root", ledger, seq_le] that create_pda refuses to reuse, and
/// nothing here can modify an earlier RootEntry.
pub fn publish_root(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let [authority, payer, ledger_ai, root_ai, system] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    expect_signer(authority)?;
    expect_signer(payer)?;
    expect_system_program(system)?;

    let seq = read_u64(data, 0)?;
    let root = read_array32(data, 8)?;
    let manifest_hash = read_array32(data, 40)?;
    let leaf_count = read_u64(data, 72)?;
    if leaf_count == 0 {
        return Err(ProgramError::InvalidInstructionData);
    }

    {
        let l = cast_account::<Ledger>(ledger_ai, &DISC_LEDGER)?;
        if l.scheme != SCHEME_SORTED_PAIR {
            return Err(LedgerError::WrongScheme.into());
        }
        if !pubkey::pubkey_eq(authority.key(), &l.authority) {
            return Err(LedgerError::Unauthorized.into());
        }
        if !l.accepts_seq(seq) {
            return Err(LedgerError::SequenceMismatch.into());
        }
    }

    let seq_le = seq.to_le_bytes();
    let (expected, bump) =
        pubkey::find_program_address(&[ROOT_SEED, ledger_ai.key(), &seq_le], &crate::ID);
    if !pubkey::pubkey_eq(root_ai.key(), &expected) {
        return Err(ProgramError::InvalidSeeds);
    }
    let bump_bytes = [bump];
    let seeds = [
        Seed::from(ROOT_SEED),
        Seed::from(ledger_ai.key().as_ref()),
        Seed::from(seq_le.as_ref()),
        Seed::from(bump_bytes.as_ref()),
    ];
    create_pda(payer, root_ai, &seeds, RootEntry::LEN)?;

    let clock = Clock::get()?;
    let r = cast_account_mut::<RootEntry>(root_ai)?;
    r.discriminator = DISC_ROOT_ENTRY;
    r.version = ROOT_ENTRY_VERSION;
    r.bump = bump;
    r.seq = seq_le;
    r.root = root;
    r.aux_hash = manifest_hash;
    r.leaf_count = leaf_count.to_le_bytes();
    r.signed_timestamp_unix = [0u8; 8];
    r.published_slot = clock.slot.to_le_bytes();
    r.published_unix = (clock.unix_timestamp as u64).to_le_bytes();
    r.publisher = *authority.key();

    let l = cast_account_mut::<Ledger>(ledger_ai)?;
    l.record_seq(seq);
    Ok(())
}

/// Accounts: [ledger, root PDA]   (no signers, no writes)
/// Data, scheme 1: [seq: u64][leaf: 32][proof: 32 * n]
/// Data, scheme 2: [seq: u64][leaf: 32][index: u64][path: 32 * n]   (seq == tree_size)
///
/// Succeeds iff `leaf` is in the root published at `seq` for this ledger.
/// Callable via CPI by any program that wants to gate on anchored evidence.
pub fn verify_inclusion(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let [ledger_ai, root_ai] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let seq = read_u64(data, 0)?;
    let leaf = read_array32(data, 8)?;

    let scheme = cast_account::<Ledger>(ledger_ai, &DISC_LEDGER)?.scheme;
    let r = cast_account::<RootEntry>(root_ai, &DISC_ROOT_ENTRY)?;
    let (expected, _) =
        pubkey::find_program_address(&[ROOT_SEED, ledger_ai.key(), &seq.to_le_bytes()], &crate::ID);
    if !pubkey::pubkey_eq(root_ai.key(), &expected) || r.get_seq() != seq {
        return Err(ProgramError::InvalidSeeds);
    }
    let ok = match scheme {
        SCHEME_SORTED_PAIR => {
            let proof = data.get(40..).ok_or(ProgramError::InvalidInstructionData)?;
            verify_proof(proof, &leaf, &r.root)
        }
        SCHEME_RFC9162 => {
            let index = read_u64(data, 40)?;
            let path = data.get(48..).ok_or(ProgramError::InvalidInstructionData)?;
            verify_inclusion_rfc9162(&leaf, index, seq, path, &r.root)
        }
        _ => false,
    };
    if !ok {
        return Err(LedgerError::InvalidProof.into());
    }
    Ok(())
}
