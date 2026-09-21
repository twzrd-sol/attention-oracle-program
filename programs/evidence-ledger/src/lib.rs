//! TWZRD Evidence Ledger (Pinocchio)
//!
//! Append-only on-chain registry of merkle roots over signed evidence leaves
//! (AO receipt V6 leaves, decision-outcome attestation V1 leaves). A published
//! root is a commitment that TWZRD signed these leaves before a given slot. It
//! is NOT a vouch, an eligibility flag, or an unlock: this program has no
//! claim, mint, transfer, or scoring instruction by construction (#2195 locks).

#![cfg_attr(not(test), no_std)]

use pinocchio::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

/// Program ID: BzBAYJxUtJp6mUkJPjEYjd8vdb2FUGnAfB5X9LqrQ72W
pub const ID: Pubkey = [
    0xa3, 0x3a, 0x77, 0xe8, 0xb5, 0x5e, 0xf0, 0xaa, 0xfe, 0xbb, 0x48, 0x6d, 0x26, 0x2e, 0x16, 0x42,
    0x9f, 0xa3, 0xb3, 0x43, 0x21, 0xdf, 0x63, 0x86, 0x3e, 0xf1, 0x90, 0x16, 0x26, 0x01, 0x2f, 0x97,
];

/// System program ID (all zeros).
pub const SYSTEM_ID: Pubkey = [0u8; 32];

pub mod error;
pub mod instructions;
pub mod keccak;
pub mod merkle;
// Host/test only. A release SBF build must stay byte-identical to mainnet
// commit 43ff827; this encoder is not an instruction.
#[cfg(any(test, feature = "localtest"))]
pub mod leaf_stripe;
pub mod state;

#[cfg_attr(target_os = "solana", link_section = ".security.txt")]
#[allow(dead_code)]
#[no_mangle]
pub static security_txt: [u8; 367] = *b"\
=======BEGIN SECURITY.TXT V1=======\0\
name\0TWZRD Evidence Ledger\0\
project_url\0https://twzrd.xyz\0\
contacts\0email:security@twzrd.xyz\0\
policy\0https://github.com/twzrd-sol/wzrd-final/blob/main/SECURITY.md\0\
preferred_languages\0en\0\
source_code\0https://github.com/twzrd-sol/wzrd-final/tree/main/programs/evidence-ledger\0\
source_revision\0\0\
auditors\0\0\
=======END SECURITY.TXT V1=======\0";

pinocchio::program_entrypoint!(process_instruction);
pinocchio::default_allocator!();
pinocchio::nostd_panic_handler!();

/// Instruction data = 8-byte discriminator (sha256("global:<name>")[..8],
/// Anchor convention so generic decoders work) ++ borsh-free fixed-width args.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if program_id != &crate::ID {
        return Err(ProgramError::IncorrectProgramId);
    }
    let disc: &[u8] = instruction_data
        .get(..8)
        .ok_or(ProgramError::InvalidInstructionData)?;
    let ix_data: &[u8] = &instruction_data[8..];
    instructions::dispatch(disc, accounts, ix_data)
}
