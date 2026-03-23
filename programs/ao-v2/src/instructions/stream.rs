//! Stream merkle root + claim instruction handlers (Pinocchio).
//!
//! This path mints vLOFI directly from the ProtocolState PDA authority.
//! It is intentionally isolated from CCM/global transfer-based claims.
//!
//! Instruction set:
//!   - publish_stream_root
//!   - claim_stream
//!   - claim_stream_sponsored

use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Signer},
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    ProgramResult,
};

use crate::error::OracleError;
use crate::keccak::keccak256;
use crate::state::{DISC_CLAIM_STATE_STREAM, DISC_PROTOCOL_STATE, DISC_STREAM_ROOT_CONFIG};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Ring-buffer depth for recent merkle roots.
const CUMULATIVE_ROOT_HISTORY: usize = 4;

/// Domain tag for stream leaf hashing.
const STREAM_V1_DOMAIN: &[u8] = b"TWZRD:STREAM_V1";

/// Internal version stamps written into on-chain data.
const STREAM_ROOT_VERSION: u8 = 1;
const CLAIM_STATE_STREAM_VERSION: u8 = 1;

/// Maximum merkle proof length (tree depth).
const MAX_PROOF_LEN: usize = 32;

// ---------------------------------------------------------------------------
// PDA seeds
// ---------------------------------------------------------------------------

const PROTOCOL_STATE_SEED: &[u8] = b"protocol_state";
const STREAM_ROOT_SEED: &[u8] = b"stream_root";
const CLAIM_STATE_STREAM_SEED: &[u8] = b"claim_stream";

// ---------------------------------------------------------------------------
// Byte offsets
// ---------------------------------------------------------------------------

// ProtocolState (173 bytes total):
const PS_LEN: usize = 173;
const PS_ADMIN: usize = 10;
const PS_PUBLISHER: usize = 42;
const PS_PAUSED: usize = 170;
const PS_BUMP: usize = 172;

// StreamRootConfig (370 bytes):
const SRC_LEN: usize = 370;
const SRC_VERSION: usize = 8;
const SRC_BUMP: usize = 9;
const SRC_MINT: usize = 10;
const SRC_LATEST_ROOT_SEQ: usize = 42;
const SRC_ROOTS_START: usize = 50;
const ROOT_ENTRY_SIZE: usize = 80;

// ClaimStateStream (90 bytes):
const CSS_LEN: usize = 90;
const CSS_VERSION: usize = 8;
const CSS_BUMP: usize = 9;
const CSS_MINT: usize = 10;
const CSS_WALLET: usize = 42;
const CSS_CLAIMED_TOTAL: usize = 74;
const CSS_LAST_CLAIM_SEQ: usize = 82;

// SPL Token account layout offsets.
const TOKEN_ACCOUNT_MIN_LEN: usize = 64;
const TOKEN_ACCOUNT_MINT: usize = 0;
const TOKEN_ACCOUNT_OWNER: usize = 32;

// ---------------------------------------------------------------------------
// Event discriminators
// ---------------------------------------------------------------------------

/// SHA-256("event:StreamRootPublished")[..8]
const DISC_STREAM_ROOT_PUBLISHED: [u8; 8] = [0x79, 0xe1, 0xa1, 0x9b, 0x6c, 0x79, 0xc3, 0xd4];
/// SHA-256("event:StreamRewardsClaimed")[..8]
const DISC_STREAM_REWARDS_CLAIMED: [u8; 8] = [0xe7, 0x28, 0x0c, 0x04, 0x6b, 0xd0, 0x7e, 0x91];

// ---------------------------------------------------------------------------
// Inline helpers
// ---------------------------------------------------------------------------

#[inline(always)]
fn read_u64(data: &[u8], offset: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&data[offset..offset + 8]);
    u64::from_le_bytes(b)
}

#[inline(always)]
fn write_u64(data: &mut [u8], offset: usize, val: u64) {
    data[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
}

#[inline(always)]
fn read_pubkey(data: &[u8], offset: usize) -> Pubkey {
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&data[offset..offset + 32]);
    pk
}

#[inline(always)]
fn read_hash(data: &[u8], offset: usize) -> [u8; 32] {
    let mut h = [0u8; 32];
    h.copy_from_slice(&data[offset..offset + 32]);
    h
}

#[inline(always)]
fn is_default_pubkey(pk: &Pubkey) -> bool {
    *pk == [0u8; 32]
}

#[inline(always)]
fn token_program_is_supported(tp: &Pubkey) -> bool {
    pubkey::pubkey_eq(tp, &crate::SPL_TOKEN_ID) || pubkey::pubkey_eq(tp, &crate::TOKEN_2022_ID)
}

#[inline(always)]
fn validate_token_program(token_program: &AccountInfo) -> Result<(), ProgramError> {
    if token_program_is_supported(token_program.key()) {
        Ok(())
    } else {
        Err(OracleError::InvalidTokenProgram.into())
    }
}

#[inline(always)]
fn token_account_mint(account: &AccountInfo) -> Result<Pubkey, ProgramError> {
    let data = unsafe { account.borrow_data_unchecked() };
    if data.len() < TOKEN_ACCOUNT_MIN_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(read_pubkey(&data, TOKEN_ACCOUNT_MINT))
}

#[inline(always)]
fn token_account_owner(account: &AccountInfo) -> Result<Pubkey, ProgramError> {
    let data = unsafe { account.borrow_data_unchecked() };
    if data.len() < TOKEN_ACCOUNT_MIN_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(read_pubkey(&data, TOKEN_ACCOUNT_OWNER))
}

// ---------------------------------------------------------------------------
// Keccak / Merkle helpers
// ---------------------------------------------------------------------------

#[inline(never)]
fn verify_proof(proof: &[[u8; 32]], mut hash: [u8; 32], root: [u8; 32]) -> bool {
    if proof.len() > MAX_PROOF_LEN {
        return false;
    }
    for sibling in proof.iter() {
        let (a, b) = if hash <= *sibling {
            (hash, *sibling)
        } else {
            (*sibling, hash)
        };
        hash = keccak256(&[&a, &b]);
    }
    hash == root
}

#[inline(never)]
fn compute_stream_leaf(
    mint: &Pubkey,
    root_seq: u64,
    wallet: &Pubkey,
    cumulative_total: u64,
) -> [u8; 32] {
    keccak256(&[
        STREAM_V1_DOMAIN,
        mint,
        &root_seq.to_le_bytes(),
        wallet,
        &cumulative_total.to_le_bytes(),
    ])
}

// ---------------------------------------------------------------------------
// CPI helper: SPL/Token-2022 MintTo
// ---------------------------------------------------------------------------

#[inline(never)]
fn mint_to_cpi(
    token_program: &AccountInfo,
    mint: &AccountInfo,
    to: &AccountInfo,
    authority: &AccountInfo,
    amount: u64,
    signer_seeds: &[pinocchio::instruction::Seed],
) -> ProgramResult {
    validate_token_program(token_program)?;

    let mut ix_data = [0u8; 9];
    ix_data[0] = 7; // MintTo
    ix_data[1..9].copy_from_slice(&amount.to_le_bytes());

    let metas = [
        AccountMeta::writable(mint.key()),
        AccountMeta::writable(to.key()),
        AccountMeta::readonly_signer(authority.key()),
    ];
    let ix = Instruction {
        program_id: token_program.key(),
        accounts: &metas,
        data: &ix_data,
    };

    let signer = Signer::from(signer_seeds);
    pinocchio::cpi::slice_invoke_signed(&ix, &[mint, to, authority, token_program], &[signer])
}

// ---------------------------------------------------------------------------
// Event helpers
// ---------------------------------------------------------------------------

#[inline(never)]
fn emit_event(buf: &[u8]) {
    pinocchio::log::sol_log_data(&[buf]);
}

#[inline(never)]
fn emit_stream_root_published(
    mint: &Pubkey,
    root_seq: u64,
    root: &[u8; 32],
    dataset_hash: &[u8; 32],
    publisher: &Pubkey,
    slot: u64,
) {
    let mut buf = [0u8; 152];
    buf[0..8].copy_from_slice(&DISC_STREAM_ROOT_PUBLISHED);
    buf[8..40].copy_from_slice(mint);
    buf[40..48].copy_from_slice(&root_seq.to_le_bytes());
    buf[48..80].copy_from_slice(root);
    buf[80..112].copy_from_slice(dataset_hash);
    buf[112..144].copy_from_slice(publisher);
    buf[144..152].copy_from_slice(&slot.to_le_bytes());
    emit_event(&buf);
}

#[inline(never)]
fn emit_stream_rewards_claimed(
    claimer: &Pubkey,
    amount: u64,
    cumulative_total: u64,
    root_seq: u64,
) {
    let mut buf = [0u8; 64];
    buf[0..8].copy_from_slice(&DISC_STREAM_REWARDS_CLAIMED);
    buf[8..40].copy_from_slice(claimer);
    buf[40..48].copy_from_slice(&amount.to_le_bytes());
    buf[48..56].copy_from_slice(&cumulative_total.to_le_bytes());
    buf[56..64].copy_from_slice(&root_seq.to_le_bytes());
    emit_event(&buf);
}

// ===========================================================================
// PUBLISH STREAM ROOT
// ===========================================================================
//
// Accounts:
//   [0] payer              -- mut, signer (admin or publisher)
//   [1] protocol_state     -- immut
//   [2] stream_root_config -- mut (init_if_needed)
//   [3] vlofi_mint         -- immut
//   [4] system_program     -- immut
//
// Instruction data (after 8-byte disc):
//   [0..8]   root_seq       (u64 LE)
//   [8..40]  root           ([u8; 32])
//   [40..72] dataset_hash   ([u8; 32])
// ===========================================================================

pub fn publish_stream_root(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if ix_data.len() < 72 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let root_seq = u64::from_le_bytes(ix_data[0..8].try_into().unwrap());
    let root = read_hash(ix_data, 8);
    let dataset_hash = read_hash(ix_data, 40);

    let [payer, protocol_state_ai, stream_root_ai, mint_ai, system_program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if !payer.is_writable() || !stream_root_ai.is_writable() {
        return Err(ProgramError::InvalidAccountData);
    }
    if !pubkey::pubkey_eq(system_program.key(), &crate::SYSTEM_ID) {
        return Err(ProgramError::IncorrectProgramId);
    }

    // -- validate protocol_state --
    if protocol_state_ai.owner() != &crate::ID {
        return Err(ProgramError::IllegalOwner);
    }
    let ps_data = unsafe { protocol_state_ai.borrow_data_unchecked() };
    if ps_data.len() < PS_LEN || ps_data[0..8] != DISC_PROTOCOL_STATE {
        return Err(ProgramError::InvalidAccountData);
    }

    let (expected_ps, _) = pubkey::find_program_address(&[PROTOCOL_STATE_SEED], &crate::ID);
    if !pubkey::pubkey_eq(protocol_state_ai.key(), &expected_ps) {
        return Err(ProgramError::InvalidSeeds);
    }

    let admin = read_pubkey(&ps_data, PS_ADMIN);
    let publisher = read_pubkey(&ps_data, PS_PUBLISHER);
    let signer_key = payer.key();
    let is_admin = pubkey::pubkey_eq(signer_key, &admin);
    let is_publisher = !is_default_pubkey(&publisher) && pubkey::pubkey_eq(signer_key, &publisher);
    if !is_admin && !is_publisher {
        return Err(OracleError::Unauthorized.into());
    }

    let paused = ps_data[PS_PAUSED] != 0;
    if paused && !is_admin {
        return Err(OracleError::ProtocolPaused.into());
    }
    drop(ps_data);

    // -- validate stream_root_config PDA --
    let mint_key = mint_ai.key();
    let (expected_src, src_bump) =
        pubkey::find_program_address(&[STREAM_ROOT_SEED, mint_key], &crate::ID);
    if !pubkey::pubkey_eq(stream_root_ai.key(), &expected_src) {
        return Err(ProgramError::InvalidSeeds);
    }

    // -- init_if_needed stream root config --
    if stream_root_ai.data_len() == 0 {
        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(SRC_LEN);
        let bump_byte = [src_bump];

        let seeds = [
            pinocchio::instruction::Seed::from(STREAM_ROOT_SEED),
            pinocchio::instruction::Seed::from(mint_key.as_ref()),
            pinocchio::instruction::Seed::from(bump_byte.as_ref()),
        ];
        let signer = Signer::from(&seeds);
        crate::cpi_create_account(
            payer,
            stream_root_ai,
            lamports,
            SRC_LEN as u64,
            &crate::ID,
            &[signer],
        )?;
    }

    let mut src_data = unsafe { stream_root_ai.borrow_mut_data_unchecked() };
    if src_data.len() < SRC_LEN {
        return Err(ProgramError::InvalidAccountData);
    }

    if src_data[SRC_VERSION] == 0 {
        src_data[0..8].copy_from_slice(&DISC_STREAM_ROOT_CONFIG);
        src_data[SRC_VERSION] = STREAM_ROOT_VERSION;
        src_data[SRC_BUMP] = src_bump;
        src_data[SRC_MINT..SRC_MINT + 32].copy_from_slice(mint_key);
        write_u64(&mut src_data, SRC_LATEST_ROOT_SEQ, 0);
    } else {
        if src_data[0..8] != DISC_STREAM_ROOT_CONFIG {
            return Err(ProgramError::InvalidAccountData);
        }
        if src_data[SRC_VERSION] != STREAM_ROOT_VERSION {
            return Err(OracleError::InvalidChannelState.into());
        }
        let src_mint = read_pubkey(&src_data, SRC_MINT);
        if !pubkey::pubkey_eq(&src_mint, mint_key) {
            return Err(OracleError::InvalidMint.into());
        }
    }

    let current_seq = read_u64(&src_data, SRC_LATEST_ROOT_SEQ);
    if root_seq != current_seq + 1 {
        return Err(OracleError::InvalidRootSeq.into());
    }

    let idx = (root_seq as usize) % CUMULATIVE_ROOT_HISTORY;
    let entry_offset = SRC_ROOTS_START + idx * ROOT_ENTRY_SIZE;
    let slot = Clock::get()?.slot;

    write_u64(&mut src_data, entry_offset, root_seq);
    src_data[entry_offset + 8..entry_offset + 40].copy_from_slice(&root);
    src_data[entry_offset + 40..entry_offset + 72].copy_from_slice(&dataset_hash);
    write_u64(&mut src_data, entry_offset + 72, slot);

    write_u64(&mut src_data, SRC_LATEST_ROOT_SEQ, root_seq);
    drop(src_data);

    emit_stream_root_published(mint_key, root_seq, &root, &dataset_hash, signer_key, slot);
    Ok(())
}

// ===========================================================================
// CLAIM STREAM (self-signed)
// ===========================================================================
//
// Accounts:
//   [0] claimer            -- mut, signer
//   [1] protocol_state     -- immut (PDA signer for mint_to CPI)
//   [2] stream_root_config -- immut
//   [3] claim_state_stream -- mut (init_if_needed)
//   [4] vlofi_mint         -- mut
//   [5] claimer_vlofi_ata  -- mut
//   [6] token_program      -- immut
//   [7] system_program     -- immut
//
// Instruction data (after 8-byte disc):
//   [0..8]   root_seq          (u64 LE)
//   [8..16]  cumulative_total  (u64 LE)
//   [16..20] proof_len         (u32 LE)
//   [20..]   proof             (proof_len x 32 bytes)
// ===========================================================================

pub fn claim_stream(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if ix_data.len() < 20 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let root_seq = u64::from_le_bytes(ix_data[0..8].try_into().unwrap());
    let cumulative_total = u64::from_le_bytes(ix_data[8..16].try_into().unwrap());
    let proof_len = u32::from_le_bytes(ix_data[16..20].try_into().unwrap()) as usize;

    if proof_len > MAX_PROOF_LEN {
        return Err(OracleError::InvalidProofLength.into());
    }
    if ix_data.len() < 20 + proof_len * 32 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let (proof_nodes, proof_count) = parse_proof(&ix_data[20..], proof_len);

    if accounts.len() < 8 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let claimer = &accounts[0];

    if !claimer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    claim_stream_inner(
        claimer.key(),
        claimer,
        &accounts[1], // protocol_state
        &accounts[2], // stream_root_config
        &accounts[3], // claim_state_stream
        &accounts[4], // vlofi_mint
        &accounts[5], // claimer_vlofi_ata
        &accounts[6], // token_program
        &accounts[7], // system_program
        root_seq,
        cumulative_total,
        &proof_nodes[..proof_count],
    )
}

// ===========================================================================
// CLAIM STREAM SPONSORED (gasless relay)
// ===========================================================================
//
// Accounts:
//   [0] payer              -- mut, signer (relayer)
//   [1] claimer            -- immut, NOT signer
//   [2] protocol_state     -- immut
//   [3] stream_root_config -- immut
//   [4] claim_state_stream -- mut (init_if_needed)
//   [5] vlofi_mint         -- mut
//   [6] claimer_vlofi_ata  -- mut
//   [7] token_program      -- immut
//   [8] system_program     -- immut
// ===========================================================================

pub fn claim_stream_sponsored(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if ix_data.len() < 20 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let root_seq = u64::from_le_bytes(ix_data[0..8].try_into().unwrap());
    let cumulative_total = u64::from_le_bytes(ix_data[8..16].try_into().unwrap());
    let proof_len = u32::from_le_bytes(ix_data[16..20].try_into().unwrap()) as usize;

    if proof_len > MAX_PROOF_LEN {
        return Err(OracleError::InvalidProofLength.into());
    }
    if ix_data.len() < 20 + proof_len * 32 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let (proof_nodes, proof_count) = parse_proof(&ix_data[20..], proof_len);

    if accounts.len() < 9 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let payer = &accounts[0];
    let claimer = &accounts[1];

    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    claim_stream_inner(
        claimer.key(),
        payer,
        &accounts[2], // protocol_state
        &accounts[3], // stream_root_config
        &accounts[4], // claim_state_stream
        &accounts[5], // vlofi_mint
        &accounts[6], // claimer_vlofi_ata
        &accounts[7], // token_program
        &accounts[8], // system_program
        root_seq,
        cumulative_total,
        &proof_nodes[..proof_count],
    )
}

// ---------------------------------------------------------------------------
// Shared claim logic
// ---------------------------------------------------------------------------

const MAX_PROOF_NODES: usize = 32;

#[inline(never)]
fn parse_proof(data: &[u8], count: usize) -> ([[u8; 32]; MAX_PROOF_NODES], usize) {
    let clamped = if count > MAX_PROOF_NODES {
        MAX_PROOF_NODES
    } else {
        count
    };
    let mut nodes = [[0u8; 32]; MAX_PROOF_NODES];
    for i in 0..clamped {
        let off = i * 32;
        if off + 32 <= data.len() {
            nodes[i].copy_from_slice(&data[off..off + 32]);
        }
    }
    (nodes, clamped)
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn claim_stream_inner<'a>(
    wallet_key: &Pubkey,
    payer: &'a AccountInfo,
    protocol_state_ai: &'a AccountInfo,
    stream_root_ai: &'a AccountInfo,
    claim_state_ai: &'a AccountInfo,
    mint_ai: &'a AccountInfo,
    claimer_ata: &'a AccountInfo,
    token_program: &'a AccountInfo,
    system_program: &'a AccountInfo,
    root_seq: u64,
    cumulative_total: u64,
    proof: &[[u8; 32]],
) -> ProgramResult {
    if !payer.is_writable() || !claim_state_ai.is_writable() || !claimer_ata.is_writable() {
        return Err(ProgramError::InvalidAccountData);
    }
    if !pubkey::pubkey_eq(system_program.key(), &crate::SYSTEM_ID) {
        return Err(ProgramError::IncorrectProgramId);
    }
    validate_token_program(token_program)?;
    if mint_ai.owner() != token_program.key() || claimer_ata.owner() != token_program.key() {
        return Err(OracleError::InvalidTokenProgram.into());
    }

    // -- validate protocol_state --
    if protocol_state_ai.owner() != &crate::ID {
        return Err(ProgramError::IllegalOwner);
    }
    let ps_data = unsafe { protocol_state_ai.borrow_data_unchecked() };
    if ps_data.len() < PS_LEN || ps_data[0..8] != DISC_PROTOCOL_STATE {
        return Err(ProgramError::InvalidAccountData);
    }

    let (expected_ps, _) = pubkey::find_program_address(&[PROTOCOL_STATE_SEED], &crate::ID);
    if !pubkey::pubkey_eq(protocol_state_ai.key(), &expected_ps) {
        return Err(ProgramError::InvalidSeeds);
    }

    let paused = ps_data[PS_PAUSED] != 0;
    if paused {
        return Err(OracleError::ProtocolPaused.into());
    }
    let ps_bump = ps_data[PS_BUMP];
    drop(ps_data);

    // -- validate stream_root_config --
    if stream_root_ai.owner() != &crate::ID {
        return Err(ProgramError::IllegalOwner);
    }
    let src_data = unsafe { stream_root_ai.borrow_data_unchecked() };
    if src_data.len() < SRC_LEN || src_data[0..8] != DISC_STREAM_ROOT_CONFIG {
        return Err(ProgramError::InvalidAccountData);
    }
    if src_data[SRC_VERSION] != STREAM_ROOT_VERSION {
        return Err(OracleError::InvalidChannelState.into());
    }
    let src_mint = read_pubkey(&src_data, SRC_MINT);
    if !pubkey::pubkey_eq(&src_mint, mint_ai.key()) {
        return Err(OracleError::InvalidMint.into());
    }
    let (expected_src, _) =
        pubkey::find_program_address(&[STREAM_ROOT_SEED, mint_ai.key()], &crate::ID);
    if !pubkey::pubkey_eq(stream_root_ai.key(), &expected_src) {
        return Err(ProgramError::InvalidSeeds);
    }

    // -- root lookup --
    let idx = (root_seq as usize) % CUMULATIVE_ROOT_HISTORY;
    let entry_offset = SRC_ROOTS_START + idx * ROOT_ENTRY_SIZE;
    let entry_seq = read_u64(&src_data, entry_offset);
    if entry_seq != root_seq {
        return Err(OracleError::RootTooOldOrMissing.into());
    }
    let entry_root = read_hash(&src_data, entry_offset + 8);
    drop(src_data);

    // -- validate destination account belongs to claimer + mint --
    let ata_mint = token_account_mint(claimer_ata)?;
    if !pubkey::pubkey_eq(&ata_mint, mint_ai.key()) {
        return Err(OracleError::InvalidMint.into());
    }
    let ata_owner = token_account_owner(claimer_ata)?;
    if !pubkey::pubkey_eq(&ata_owner, wallet_key) {
        return Err(OracleError::InvalidClaimState.into());
    }

    // -- verify proof --
    if proof.len() > MAX_PROOF_LEN {
        return Err(OracleError::InvalidProofLength.into());
    }
    let leaf = compute_stream_leaf(mint_ai.key(), root_seq, wallet_key, cumulative_total);
    if !verify_proof(proof, leaf, entry_root) {
        return Err(OracleError::InvalidProof.into());
    }

    // -- init_if_needed claim state --
    let (expected_cs, cs_bump) = pubkey::find_program_address(
        &[CLAIM_STATE_STREAM_SEED, mint_ai.key(), wallet_key],
        &crate::ID,
    );
    if !pubkey::pubkey_eq(claim_state_ai.key(), &expected_cs) {
        return Err(ProgramError::InvalidSeeds);
    }

    let needs_create = claim_state_ai.data_len() == 0;
    if needs_create {
        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(CSS_LEN);
        let bump_byte = [cs_bump];

        let seeds = [
            pinocchio::instruction::Seed::from(CLAIM_STATE_STREAM_SEED),
            pinocchio::instruction::Seed::from(mint_ai.key().as_ref()),
            pinocchio::instruction::Seed::from(wallet_key.as_ref()),
            pinocchio::instruction::Seed::from(bump_byte.as_ref()),
        ];
        let signer = Signer::from(&seeds);
        crate::cpi_create_account(
            payer,
            claim_state_ai,
            lamports,
            CSS_LEN as u64,
            &crate::ID,
            &[signer],
        )?;
    } else if claim_state_ai.owner() != &crate::ID {
        return Err(ProgramError::IllegalOwner);
    }

    let mut cs_data = unsafe { claim_state_ai.borrow_mut_data_unchecked() };
    if cs_data.len() < CSS_LEN {
        return Err(ProgramError::InvalidAccountData);
    }

    if cs_data[CSS_VERSION] == 0 {
        cs_data[0..8].copy_from_slice(&DISC_CLAIM_STATE_STREAM);
        cs_data[CSS_VERSION] = CLAIM_STATE_STREAM_VERSION;
        cs_data[CSS_BUMP] = cs_bump;
        cs_data[CSS_MINT..CSS_MINT + 32].copy_from_slice(mint_ai.key());
        cs_data[CSS_WALLET..CSS_WALLET + 32].copy_from_slice(wallet_key);
        write_u64(&mut cs_data, CSS_CLAIMED_TOTAL, 0);
        write_u64(&mut cs_data, CSS_LAST_CLAIM_SEQ, 0);
    } else {
        if cs_data[0..8] != DISC_CLAIM_STATE_STREAM {
            return Err(ProgramError::InvalidAccountData);
        }
        let cs_mint = read_pubkey(&cs_data, CSS_MINT);
        if !pubkey::pubkey_eq(&cs_mint, mint_ai.key()) {
            return Err(OracleError::InvalidClaimState.into());
        }
        let cs_wallet = read_pubkey(&cs_data, CSS_WALLET);
        if !pubkey::pubkey_eq(&cs_wallet, wallet_key) {
            return Err(OracleError::InvalidClaimState.into());
        }
    }

    let claimed_total = read_u64(&cs_data, CSS_CLAIMED_TOTAL);
    if cumulative_total <= claimed_total {
        return Ok(());
    }

    let delta = cumulative_total
        .checked_sub(claimed_total)
        .ok_or::<ProgramError>(OracleError::MathOverflow.into())?;

    drop(cs_data);

    let bump_byte = [ps_bump];
    let seeds = [
        pinocchio::instruction::Seed::from(PROTOCOL_STATE_SEED),
        pinocchio::instruction::Seed::from(bump_byte.as_ref()),
    ];
    mint_to_cpi(
        token_program,
        mint_ai,
        claimer_ata,
        protocol_state_ai,
        delta,
        &seeds,
    )?;

    let mut cs_data = unsafe { claim_state_ai.borrow_mut_data_unchecked() };
    write_u64(&mut cs_data, CSS_CLAIMED_TOTAL, cumulative_total);
    write_u64(&mut cs_data, CSS_LAST_CLAIM_SEQ, root_seq);
    drop(cs_data);

    emit_stream_rewards_claimed(wallet_key, delta, cumulative_total, root_seq);
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_empty_is_root() {
        let leaf = [7u8; 32];
        assert!(verify_proof(&[], leaf, leaf));
    }

    #[test]
    fn proof_rejects_wrong_root() {
        assert!(!verify_proof(&[[2u8; 32]], [1u8; 32], [99u8; 32]));
    }

    #[test]
    fn stream_leaf_deterministic() {
        let mint = [0x11; 32];
        let wallet = [0x22; 32];
        let leaf1 = compute_stream_leaf(&mint, 7, &wallet, 123_456);
        let leaf2 = compute_stream_leaf(&mint, 7, &wallet, 123_456);
        assert_eq!(leaf1, leaf2);
    }

    #[test]
    fn stream_leaf_changes_with_total() {
        let mint = [0x11; 32];
        let wallet = [0x22; 32];
        let leaf1 = compute_stream_leaf(&mint, 7, &wallet, 123_456);
        let leaf2 = compute_stream_leaf(&mint, 7, &wallet, 123_457);
        assert_ne!(leaf1, leaf2);
    }

    #[test]
    fn parse_proof_works() {
        let mut data = vec![0u8; 64];
        data[0..32].copy_from_slice(&[1u8; 32]);
        data[32..64].copy_from_slice(&[2u8; 32]);
        let (proof_nodes, proof_count) = parse_proof(&data, 2);
        assert_eq!(proof_count, 2);
        assert_eq!(proof_nodes[0], [1u8; 32]);
        assert_eq!(proof_nodes[1], [2u8; 32]);
    }

    #[test]
    fn account_layout_sizes() {
        assert_eq!(SRC_LEN, 370);
        assert_eq!(CSS_LEN, 90);
        assert_eq!(ROOT_ENTRY_SIZE, 80);
    }
}
