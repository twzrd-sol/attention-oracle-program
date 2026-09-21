//! Anchor a signed Receipt-log head (RFC 9162, scheme 2). The head's Ed25519
//! signature is checked by the runtime's ed25519 precompile in the SAME
//! transaction; this handler proves the verified (pubkey, message) pair is
//! exactly (ledger.trusted_signer, the STH preimage rebuilt here). The program
//! never signs anything, so the chain cannot become a second signing path.

use pinocchio::{
    account_info::AccountInfo,
    instruction::Seed,
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    sysvars::{clock::Clock, instructions::Instructions, Sysvar},
    ProgramResult,
};

use super::common::{create_pda, expect_signer, expect_system_program, read_array32, read_u64};
use crate::error::LedgerError;
use crate::keccak::keccak256;
use crate::state::{
    cast_account, cast_account_mut, Ledger, RootEntry, DISC_LEDGER, DISC_ROOT_ENTRY, ROOT_SEED,
    ROOT_ENTRY_VERSION, SCHEME_RFC9162,
};

/// Byte-exact with `twzrd_agent_intel.log_sth.STH_DOMAIN`.
pub const STH_DOMAIN: &[u8] = b"TWZRD:RECEIPT_LOG_STH_V1";
/// Ed25519SigVerify111111111111111111111111111
pub const ED25519_PROGRAM_ID: Pubkey = [
    0x03, 0x7d, 0x46, 0xd6, 0x7c, 0x93, 0xfb, 0xbe, 0x12, 0xf9, 0x42, 0x8f, 0x83, 0x8d, 0x40, 0xff,
    0x05, 0x70, 0x74, 0x49, 0x27, 0xf4, 0x8a, 0x64, 0xfc, 0xca, 0x70, 0x44, 0x80, 0x00, 0x00, 0x00,];
const MAX_LOG_ID: usize = 64;
/// domain(24) + len(2) + log_id(<=64) + tree_size(8) + ts(8) + root(32)
const MAX_PREIMAGE: usize = 24 + 2 + MAX_LOG_ID + 8 + 8 + 32;

/// Accounts: [authority (signer), payer (signer, writable), ledger (writable),
///            root PDA (writable), system program, instructions sysvar]
/// Data:     [tree_size: u64][signed_timestamp_unix: u64][root: 32]
///           [log_id_len: u8][log_id: UTF-8, 1..=64]
/// The ed25519 precompile instruction MUST be the immediately preceding
/// instruction in the transaction, carrying exactly one signature whose
/// pubkey is the ledger's trusted signer and whose message is the STH preimage.
pub fn anchor_head(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let [authority, payer, ledger_ai, root_ai, system, ix_sysvar] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    expect_signer(authority)?;
    expect_signer(payer)?;
    expect_system_program(system)?;

    let tree_size = read_u64(data, 0)?;
    let signed_ts = read_u64(data, 8)?;
    let root = read_array32(data, 16)?;
    let llen = *data.get(48).ok_or(ProgramError::InvalidInstructionData)? as usize;
    if tree_size == 0 || llen == 0 || llen > MAX_LOG_ID {
        return Err(ProgramError::InvalidInstructionData);
    }
    let log_id = data.get(49..49 + llen).ok_or(ProgramError::InvalidInstructionData)?;

    let trusted = {
        let l = cast_account::<Ledger>(ledger_ai, &DISC_LEDGER)?;
        if l.scheme != SCHEME_RFC9162 {
            return Err(LedgerError::WrongScheme.into());
        }
        if !pubkey::pubkey_eq(authority.key(), &l.authority) {
            return Err(LedgerError::Unauthorized.into());
        }
        if keccak256(&[log_id]) != l.log_id_hash {
            return Err(ProgramError::InvalidInstructionData);
        }
        if !l.accepts_seq(tree_size) {
            return Err(LedgerError::SequenceMismatch.into());
        }
        l.trusted_signer
    };

    // Rebuild the exact bytes the log key signed.
    let mut pre = [0u8; MAX_PREIMAGE];
    let mut n = 0;
    for part in [STH_DOMAIN, &(llen as u16).to_le_bytes()[..], log_id, &tree_size.to_le_bytes()[..], &signed_ts.to_le_bytes()[..], &root[..]] {
        pre[n..n + part.len()].copy_from_slice(part);
        n += part.len();
    }
    check_ed25519_precompile(ix_sysvar, &trusted, &pre[..n])?;

    let seq_le = tree_size.to_le_bytes();
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
    r.aux_hash = [0u8; 32];
    r.leaf_count = seq_le;
    r.signed_timestamp_unix = signed_ts.to_le_bytes();
    r.published_slot = clock.slot.to_le_bytes();
    r.published_unix = (clock.unix_timestamp as u64).to_le_bytes();
    r.publisher = *authority.key();
    cast_account_mut::<Ledger>(ledger_ai)?.record_seq(tree_size);
    Ok(())
}

/// The preceding instruction must be one ed25519 precompile call whose single
/// (pubkey, message) pair is exactly (`trusted`, `message`). Layout per
/// solana-ed25519-program: [num_sigs u8][pad u8][7 x u16 LE offsets][data...];
/// an instruction index of u16::MAX means "this instruction".
#[inline(never)]
fn check_ed25519_precompile(ix_sysvar: &AccountInfo, trusted: &Pubkey, message: &[u8]) -> ProgramResult {
    let ixs = Instructions::try_from(ix_sysvar)?;
    let cur = ixs.load_current_index();
    if cur == 0 {
        return Err(LedgerError::MissingSignatureCheck.into());
    }
    let prev = ixs.load_instruction_at((cur - 1) as usize)?;
    if !pubkey::pubkey_eq(prev.get_program_id(), &ED25519_PROGRAM_ID) {
        return Err(LedgerError::MissingSignatureCheck.into());
    }
    let d = prev.get_instruction_data();
    if d.len() < 16 || d[0] != 1 {
        return Err(LedgerError::MissingSignatureCheck.into());
    }
    let u16at = |o: usize| u16::from_le_bytes([d[o], d[o + 1]]);
    let (sig_ix, pk_off, pk_ix, msg_off, msg_len, msg_ix) =
        (u16at(4), u16at(6) as usize, u16at(8), u16at(10) as usize, u16at(12) as usize, u16at(14));
    let this = cur - 1;
    for ix in [sig_ix, pk_ix, msg_ix] {
        if ix != u16::MAX && ix != this {
            return Err(LedgerError::MissingSignatureCheck.into());
        }
    }
    let pk = d.get(pk_off..pk_off + 32).ok_or(LedgerError::MissingSignatureCheck)?;
    let msg = d.get(msg_off..msg_off + msg_len).ok_or(LedgerError::MissingSignatureCheck)?;
    if pk != trusted || msg != message {
        return Err(LedgerError::SignatureBindingMismatch.into());
    }
    Ok(())
}
