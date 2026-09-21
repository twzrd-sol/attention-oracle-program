//! Error codes. Anchor convention: custom code = 6000 + variant index.
//! Append only; never reorder (clients match on the number).

use pinocchio::program_error::ProgramError;

pub const ANCHOR_ERROR_OFFSET: u32 = 6000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum LedgerError {
    /// 6000 — signer is not the ledger authority
    Unauthorized = 0,
    /// 6001 — account already initialized
    AlreadyInitialized = 1,
    /// 6002 — seq must equal ledger.next_seq (append-only, no gaps, no rewrites)
    SequenceMismatch = 2,
    /// 6003 — merkle proof does not reach the published root
    InvalidProof = 3,
    /// 6004 — a required pubkey is the default (all-zero) key
    InvalidPubkey = 4,
    /// 6005 — instruction not valid for this ledger's scheme
    WrongScheme = 5,
    /// 6006 — no ed25519 precompile instruction immediately precedes this one
    MissingSignatureCheck = 6,
    /// 6007 — the precompile verified a different (pubkey, message) than this head
    SignatureBindingMismatch = 7,
}

impl From<LedgerError> for ProgramError {
    fn from(e: LedgerError) -> Self {
        ProgramError::Custom(ANCHOR_ERROR_OFFSET + e as u32)
    }
}
