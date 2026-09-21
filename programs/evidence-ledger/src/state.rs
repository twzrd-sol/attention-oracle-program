//! Account layouts. Every struct is `#[repr(C)]` with `[u8; N]` fields only
//! (alignment 1) and an 8-byte Anchor-style discriminator at offset 0, so
//! generic Anchor decoders can read them. Discriminators are hardcoded and
//! pinned by `tests::discriminators_match_anchor_convention`.

use pinocchio::{account_info::AccountInfo, program_error::ProgramError};

pub const LEDGER_SEED: &[u8] = b"ledger";
pub const ROOT_SEED: &[u8] = b"root";
pub const LEDGER_VERSION: u8 = 1;
pub const ROOT_ENTRY_VERSION: u8 = 1;

/// sha256("account:Ledger")[..8]
pub const DISC_LEDGER: [u8; 8] = [0x2b, 0x29, 0x15, 0xd5, 0xb4, 0xb0, 0x5f, 0x20];
/// sha256("account:RootEntry")[..8]
pub const DISC_ROOT_ENTRY: [u8; 8] = [0xe9, 0x53, 0xac, 0x00, 0xd6, 0x75, 0x69, 0x98];

/// One ledger per log / leaf domain. PDA: `["ledger", log_id_hash]` where
/// `log_id_hash = keccak256(log_id)` (e.g. `intel.twzrd.xyz/v6`, or a batch
/// domain such as `TWZRD:AO_REPUTATION_RECEIPT_V6`).
///
/// ```text
/// Offset  Size  Field
/// 0       8     discriminator
/// 8       1     version
/// 9       1     bump
/// 10      1     scheme          (1 = sorted-pair keccak batch, 2 = RFC 9162 keccak log)
/// 11      32    authority       (only key allowed to spend on publishing)
/// 43      32    log_id_hash
/// 75      32    trusted_signer  (Ed25519 key whose off-chain signature scheme-2 heads must carry; immutable)
/// 107     8     last_seq        (seq / tree_size of the newest entry)
/// 115     8     count           (entries published; 0 = last_seq unset)
/// 123     5     reserved
/// ```
/// `trusted_signer` is fixed at init on purpose: the log spec says a key change
/// means a new log_id, never a re-signed head, so there is no rotation path.
#[repr(C)]
pub struct Ledger {
    pub discriminator: [u8; 8],
    pub version: u8,
    pub bump: u8,
    pub scheme: u8,
    pub authority: [u8; 32],
    pub log_id_hash: [u8; 32],
    pub trusted_signer: [u8; 32],
    pub last_seq: [u8; 8],
    pub count: [u8; 8],
    pub reserved: [u8; 5],
}

pub const SCHEME_SORTED_PAIR: u8 = 1;
pub const SCHEME_RFC9162: u8 = 2;

impl Ledger {
    pub const LEN: usize = 128;
    #[inline]
    pub fn get_last_seq(&self) -> u64 {
        u64::from_le_bytes(self.last_seq)
    }
    #[inline]
    pub fn get_count(&self) -> u64 {
        u64::from_le_bytes(self.count)
    }
    /// Append-only cursor: the first entry may use any seq; later ones must be
    /// strictly greater (log heads jump by tree size, batches by one).
    #[inline]
    pub fn accepts_seq(&self, seq: u64) -> bool {
        self.get_count() == 0 || seq > self.get_last_seq()
    }
    #[inline]
    pub fn record_seq(&mut self, seq: u64) {
        self.last_seq = seq.to_le_bytes();
        self.count = (self.get_count() + 1).to_le_bytes();
    }
}

/// One immutable account per published root / anchored head.
/// PDA: `["root", ledger, seq_le]`. Never overwritten: a correction is a NEW
/// entry at a later seq and the old account stays readable (down-only).
///
/// ```text
/// Offset  Size  Field
/// 0       8     discriminator
/// 8       1     version
/// 9       1     bump
/// 10      8     seq                   (batch seq, or tree_size for a log head)
/// 18      32    root
/// 50      32    aux_hash              (scheme 1: off-chain manifest keccak; scheme 2: zero)
/// 82      8     leaf_count            (scheme 2: == tree_size)
/// 90      8     signed_timestamp_unix (scheme 2: the STH's own signed timestamp; scheme 1: 0)
/// 98      8     published_slot
/// 106     8     published_unix
/// 114     32    publisher             (authority key that signed this publish)
/// 146     6     reserved
/// ```
#[repr(C)]
pub struct RootEntry {
    pub discriminator: [u8; 8],
    pub version: u8,
    pub bump: u8,
    pub seq: [u8; 8],
    pub root: [u8; 32],
    pub aux_hash: [u8; 32],
    pub leaf_count: [u8; 8],
    pub signed_timestamp_unix: [u8; 8],
    pub published_slot: [u8; 8],
    pub published_unix: [u8; 8],
    pub publisher: [u8; 32],
    pub reserved: [u8; 6],
}

impl RootEntry {
    pub const LEN: usize = 152;
    #[inline]
    pub fn get_seq(&self) -> u64 {
        u64::from_le_bytes(self.seq)
    }
}

/// Cast account data to an immutable `#[repr(C)]` struct. Validates length,
/// program ownership and discriminator.
///
/// # Safety
/// All state structs are `#[repr(C)]` with `[u8; N]` fields only (alignment 1).
#[inline]
pub fn cast_account<'a, T>(
    account: &'a AccountInfo,
    expected_disc: &[u8; 8],
) -> Result<&'a T, ProgramError> {
    if account.owner() != &crate::ID {
        return Err(ProgramError::IncorrectProgramId);
    }
    let data = unsafe { account.borrow_data_unchecked() };
    if data.len() < core::mem::size_of::<T>() || data[..8] != *expected_disc {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(unsafe { &*(data.as_ptr() as *const T) })
}

/// Mutable variant. Does NOT check the discriminator (caller may be initializing).
#[inline]
pub fn cast_account_mut<T>(account: &AccountInfo) -> Result<&mut T, ProgramError> {
    let data = unsafe { account.borrow_mut_data_unchecked() };
    if data.len() < core::mem::size_of::<T>() {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(unsafe { &mut *(data.as_mut_ptr() as *mut T) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn anchor_disc(name: &str) -> [u8; 8] {
        Sha256::digest(format!("account:{name}").as_bytes())[..8].try_into().unwrap()
    }

    #[test]
    fn layouts_are_exact() {
        assert_eq!(core::mem::size_of::<Ledger>(), Ledger::LEN);
        assert_eq!(core::mem::size_of::<RootEntry>(), RootEntry::LEN);
        assert_eq!(core::mem::align_of::<Ledger>(), 1);
        assert_eq!(core::mem::align_of::<RootEntry>(), 1);
    }

    #[test]
    fn discriminators_match_anchor_convention() {
        assert_eq!(DISC_LEDGER, anchor_disc("Ledger"));
        assert_eq!(DISC_ROOT_ENTRY, anchor_disc("RootEntry"));
    }
}
