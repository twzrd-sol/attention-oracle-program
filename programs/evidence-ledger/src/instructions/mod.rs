//! Instruction dispatch on 8-byte Anchor-style discriminators
//! (`sha256("global:<name>")[..8]`). Unknown discriminators are rejected.

use pinocchio::{account_info::AccountInfo, program_error::ProgramError, ProgramResult};

pub mod anchor;
pub mod common;
pub mod ledger;
pub mod roots;

pub fn dispatch(disc: &[u8], accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    match disc {
        // init_ledger
        [0x5b, 0xc2, 0x20, 0x53, 0x5c, 0xce, 0x36, 0x43] => ledger::init_ledger(accounts, data),
        // set_authority
        [0x85, 0xfa, 0x25, 0x15, 0x6e, 0xa3, 0x1a, 0x79] => ledger::set_authority(accounts, data),
        // publish_root
        [0x32, 0xbd, 0x23, 0xd4, 0xb4, 0x64, 0x57, 0x19] => roots::publish_root(accounts, data),
        // verify_inclusion
        [0x38, 0xda, 0x8c, 0xf2, 0x5b, 0x05, 0x6a, 0xdc] => roots::verify_inclusion(accounts, data),
        // anchor_head
        [0x6d, 0xab, 0x20, 0x3d, 0xc5, 0x98, 0xeb, 0x71] => anchor::anchor_head(accounts, data),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
