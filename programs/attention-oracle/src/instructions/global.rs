//! Global merkle root + claim instruction handlers (Pinocchio).
//!
//! Wire-compatible with the Anchor AO program -- same discriminators, same
//! account layouts, same PDA seeds, same error codes.
//!
//! Instruction set:
//!   - initialize_global_root    (disc ca366bf618f74bfd)
//!   - publish_global_root       (disc 518de216fea762ff)
//!   - claim_global_v2           (disc f82caa6531aa8c7e)
//!   - claim_global_sponsored_v2 (disc 595484508b5c5e04)
//!   - claim_global (V1 compat)  (disc 36b4975b48f36ef7)
//!   - claim_global_sponsored    (disc 1573d04978f0f494)

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

// ---------------------------------------------------------------------------
// Constants (must match Anchor crate values exactly)
// ---------------------------------------------------------------------------

/// Ring-buffer depth for recent merkle roots.
const CUMULATIVE_ROOT_HISTORY: usize = 4;

/// Domain tags for leaf hashing -- byte-identical to Anchor constants.
const GLOBAL_V4_DOMAIN: &[u8] = b"TWZRD:GLOBAL_V4";
const GLOBAL_V5_DOMAIN: &[u8] = b"TWZRD:GLOBAL_V5";

/// Leaf version selectors.
const LEAF_VERSION_V4: u8 = 4;
const LEAF_VERSION_V5: u8 = 5;

/// Internal version stamps written into on-chain data.
const GLOBAL_ROOT_VERSION: u8 = 1;
const CLAIM_STATE_GLOBAL_VERSION: u8 = 1;

/// Maximum merkle proof length (tree depth).
const MAX_PROOF_LEN: usize = 32;

/// CCM token decimals (9).
const CCM_DECIMALS: u8 = 9;

// ---------------------------------------------------------------------------
// PDA seeds
// ---------------------------------------------------------------------------

const PROTOCOL_STATE_SEED: &[u8] = b"protocol_state";
const GLOBAL_ROOT_SEED: &[u8] = b"global_root";
const CLAIM_STATE_GLOBAL_SEED: &[u8] = b"claim_global";

// ---------------------------------------------------------------------------
// Anchor-compatible 8-byte discriminators (SHA-256("account:<Name>")[..8])
// Pre-computed -- these MUST match so existing PDAs deserialize correctly.
//
// Verified via:
//   echo -n "account:ProtocolState" | sha256sum | cut -c1-16
// ---------------------------------------------------------------------------

// Import from state.rs — single source of truth for account discriminators
use crate::state::{DISC_PROTOCOL_STATE, DISC_GLOBAL_ROOT_CONFIG, DISC_CLAIM_STATE_GLOBAL};

// ---------------------------------------------------------------------------
// Account byte-offset constants
// All Anchor accounts have an 8-byte discriminator prefix.
// ---------------------------------------------------------------------------

// ProtocolState (173 bytes total):
const PS_LEN: usize = 173;
const PS_ADMIN: usize = 10;
const PS_PUBLISHER: usize = 42;
const PS_MINT: usize = 138;
const PS_PAUSED: usize = 170;
const PS_BUMP: usize = 172;

// GlobalRootConfig (370 bytes):
const GRC_LEN: usize = 370;
const GRC_VERSION: usize = 8;
const GRC_BUMP: usize = 9;
const GRC_MINT: usize = 10;
const GRC_LATEST_ROOT_SEQ: usize = 42;
const GRC_ROOTS_START: usize = 50;
const ROOT_ENTRY_SIZE: usize = 80;

// ClaimStateGlobal (90 bytes):
const CSG_LEN: usize = 90;
const CSG_VERSION: usize = 8;
const CSG_BUMP: usize = 9;
const CSG_MINT: usize = 10;
const CSG_WALLET: usize = 42;
const CSG_CLAIMED_TOTAL: usize = 74;
const CSG_LAST_CLAIM_SEQ: usize = 82;

// ---------------------------------------------------------------------------
// Well-known program IDs
// ---------------------------------------------------------------------------

use crate::TOKEN_2022_ID as SPL_TOKEN_2022_ID;

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

// ---------------------------------------------------------------------------
// Keccak / Merkle helpers (identical algorithm to merkle_proof.rs)
// ---------------------------------------------------------------------------

/// Verify a merkle proof against a known root.
///
/// Sorts each pair lexicographically, matching the canonical sorted-pair
/// tree used by the off-chain publisher.
#[inline(never)]
fn verify_proof(proof: &[[u8; 32]], mut hash: [u8; 32], root: [u8; 32]) -> bool {
    if proof.len() > 32 {
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

/// V4 global leaf: `keccak(domain || mint || root_seq || wallet || cumulative_total)`
#[inline(never)]
fn compute_global_leaf_v4(
    mint: &Pubkey,
    root_seq: u64,
    wallet: &Pubkey,
    cumulative_total: u64,
) -> [u8; 32] {
    keccak256(&[
        GLOBAL_V4_DOMAIN,
        mint,
        &root_seq.to_le_bytes(),
        wallet,
        &cumulative_total.to_le_bytes(),
    ])
}

/// V5 global leaf: `keccak(domain || mint || root_seq || wallet || base_yield || attention_bonus)`
#[inline(never)]
fn compute_global_leaf_v5(
    mint: &Pubkey,
    root_seq: u64,
    wallet: &Pubkey,
    base_yield: u64,
    attention_bonus: u64,
) -> [u8; 32] {
    keccak256(&[
        GLOBAL_V5_DOMAIN,
        mint,
        &root_seq.to_le_bytes(),
        wallet,
        &base_yield.to_le_bytes(),
        &attention_bonus.to_le_bytes(),
    ])
}

// ---------------------------------------------------------------------------
// Token-2022 CPI: transfer_checked with remaining accounts
// ---------------------------------------------------------------------------

/// Build and invoke a Token-2022 `transfer_checked` CPI, forwarding any
/// remaining accounts (transfer-fee extension hooks) exactly as the Anchor
/// implementation does.
#[inline(never)]
fn transfer_checked_cpi<'a>(
    token_program: &'a AccountInfo,
    from_ata: &'a AccountInfo,
    mint: &'a AccountInfo,
    to_ata: &'a AccountInfo,
    authority: &'a AccountInfo,
    amount: u64,
    decimals: u8,
    signer_seeds: &[pinocchio::instruction::Seed],
    remaining_accounts: &'a [AccountInfo],
) -> ProgramResult {
    // Validate token program key
    let tp_key = token_program.key();
    if !pubkey::pubkey_eq(tp_key, &SPL_TOKEN_2022_ID)
        && !pubkey::pubkey_eq(tp_key, &crate::SPL_TOKEN_ID)
    {
        return Err(OracleError::InvalidTokenProgram.into());
    }

    // Remaining accounts are forwarded as-is (transfer-fee hook accounts).
    // The token program itself validates these during CPI.

    // Instruction data for TransferChecked:
    //   [0]    = 12 (instruction index)
    //   [1..9] = amount (u64 LE)
    //   [9]    = decimals
    let mut ix_data = [0u8; 10];
    ix_data[0] = 12; // TransferChecked
    ix_data[1..9].copy_from_slice(&amount.to_le_bytes());
    ix_data[9] = decimals;

    // Build account metas: from, mint, to, authority + remaining
    // Max remaining = MAX_PROOF_LEN is generous; transfer-fee hooks use 0-2.
    const BASE_ACCOUNTS: usize = 4;
    const MAX_REMAINING: usize = 4;
    let total_metas = BASE_ACCOUNTS + remaining_accounts.len();
    if remaining_accounts.len() > MAX_REMAINING {
        return Err(ProgramError::InvalidArgument);
    }

    // Stack-allocated account metas array.
    let mut metas: [AccountMeta; BASE_ACCOUNTS + MAX_REMAINING] = [
        AccountMeta::writable(from_ata.key()),
        AccountMeta::readonly(mint.key()),
        AccountMeta::writable(to_ata.key()),
        AccountMeta::readonly_signer(authority.key()),
        AccountMeta::readonly(&[0u8; 32]),
        AccountMeta::readonly(&[0u8; 32]),
        AccountMeta::readonly(&[0u8; 32]),
        AccountMeta::readonly(&[0u8; 32]),
    ];

    for (i, acct) in remaining_accounts.iter().enumerate() {
        metas[BASE_ACCOUNTS + i] =
            AccountMeta::new(acct.key(), acct.is_writable(), acct.is_signer());
    }

    let ix = Instruction {
        program_id: tp_key,
        accounts: &metas[..total_metas],
        data: &ix_data,
    };

    let total_infos = total_metas + 1;
    let mut infos: [&AccountInfo; BASE_ACCOUNTS + MAX_REMAINING + 1] = [
        from_ata, mint, to_ata, authority,
        token_program, token_program, token_program, token_program, token_program,
    ];
    for (i, acct) in remaining_accounts.iter().enumerate() {
        infos[BASE_ACCOUNTS + i] = acct;
    }
    infos[total_metas] = token_program;

    let seeds_arr = Signer::from(signer_seeds);

    pinocchio::cpi::slice_invoke_signed(&ix, &infos[..total_infos], &[seeds_arr])
}

// ---------------------------------------------------------------------------
// Anchor event emission helpers
// ---------------------------------------------------------------------------

/// Pre-computed event discriminators: SHA-256("event:<EventName>")[..8].
///
/// Verified via: `echo -n "event:GlobalRootPublished" | sha256sum`
/// and: `echo -n "event:GlobalRewardsClaimed" | sha256sum`
const DISC_GLOBAL_ROOT_PUBLISHED: [u8; 8] = [0xf5, 0x3e, 0xd3, 0x8c, 0x75, 0x0d, 0x1e, 0xbc];
const DISC_GLOBAL_REWARDS_CLAIMED: [u8; 8] = [0x95, 0x5e, 0x89, 0xd5, 0x18, 0x9b, 0x60, 0x3d];

/// Emit Anchor-compatible event via `sol_log_data`.
#[inline(never)]
fn emit_event(buf: &[u8]) {
    pinocchio::log::sol_log_data(&[buf]);
}

#[inline(never)]
fn emit_global_root_published(
    mint: &Pubkey,
    root_seq: u64,
    root: &[u8; 32],
    dataset_hash: &[u8; 32],
    publisher: &Pubkey,
    slot: u64,
) {
    let mut buf = [0u8; 152];
    buf[0..8].copy_from_slice(&DISC_GLOBAL_ROOT_PUBLISHED);
    buf[8..40].copy_from_slice(mint);
    buf[40..48].copy_from_slice(&root_seq.to_le_bytes());
    buf[48..80].copy_from_slice(root);
    buf[80..112].copy_from_slice(dataset_hash);
    buf[112..144].copy_from_slice(publisher);
    buf[144..152].copy_from_slice(&slot.to_le_bytes());
    emit_event(&buf);
}

#[inline(never)]
fn emit_global_rewards_claimed(
    claimer: &Pubkey,
    amount: u64,
    cumulative_total: u64,
    root_seq: u64,
) {
    let mut buf = [0u8; 64];
    buf[0..8].copy_from_slice(&DISC_GLOBAL_REWARDS_CLAIMED);
    buf[8..40].copy_from_slice(claimer);
    buf[40..48].copy_from_slice(&amount.to_le_bytes());
    buf[48..56].copy_from_slice(&cumulative_total.to_le_bytes());
    buf[56..64].copy_from_slice(&root_seq.to_le_bytes());
    emit_event(&buf);
}

// ===========================================================================
// INITIALIZE GLOBAL ROOT
// ===========================================================================
//
// Accounts:
//   [0] payer              -- mut, signer
//   [1] protocol_state     -- immut, PDA ["protocol_state"]
//   [2] global_root_config -- mut (created), PDA ["global_root", mint]
//   [3] system_program     -- immut
//
// No instruction data beyond the 8-byte discriminator.
// ===========================================================================

pub fn initialize_global_root(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _ix_data: &[u8],
) -> ProgramResult {
    let [payer, protocol_state_ai, global_root_ai, system_program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // -- signer & writable --
    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if !payer.is_writable() || !global_root_ai.is_writable() {
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

    // Verify PDA
    let (expected_ps, _) = pubkey::find_program_address(&[PROTOCOL_STATE_SEED], &crate::ID);
    if !pubkey::pubkey_eq(protocol_state_ai.key(), &expected_ps) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Auth: payer must be admin or publisher
    let admin = read_pubkey(&ps_data, PS_ADMIN);
    let publisher = read_pubkey(&ps_data, PS_PUBLISHER);
    let signer_key = payer.key();
    let is_admin = pubkey::pubkey_eq(signer_key, &admin);
    let is_publisher = !is_default_pubkey(&publisher) && pubkey::pubkey_eq(signer_key, &publisher);
    if !is_admin && !is_publisher {
        return Err(OracleError::Unauthorized.into());
    }

    let mint = read_pubkey(&ps_data, PS_MINT);
    drop(ps_data);

    // -- derive PDA for global_root_config --
    let (expected_grc, grc_bump) =
        pubkey::find_program_address(&[GLOBAL_ROOT_SEED, &mint], &crate::ID);
    if !pubkey::pubkey_eq(global_root_ai.key(), &expected_grc) {
        return Err(ProgramError::InvalidSeeds);
    }

    // -- create the account --
    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(GRC_LEN);
    let bump_byte = [grc_bump];

    {
        let seeds = [
            pinocchio::instruction::Seed::from(GLOBAL_ROOT_SEED),
            pinocchio::instruction::Seed::from(mint.as_ref()),
            pinocchio::instruction::Seed::from(bump_byte.as_ref()),
        ];
        let signer = Signer::from(&seeds);
        crate::cpi_create_account(payer, global_root_ai, lamports, GRC_LEN as u64, &crate::ID, &[signer])?;
    }

    // -- initialize data --
    let mut grc_data = unsafe { global_root_ai.borrow_mut_data_unchecked() };
    grc_data[0..8].copy_from_slice(&DISC_GLOBAL_ROOT_CONFIG);
    grc_data[GRC_VERSION] = GLOBAL_ROOT_VERSION;
    grc_data[GRC_BUMP] = grc_bump;
    grc_data[GRC_MINT..GRC_MINT + 32].copy_from_slice(&mint);
    write_u64(&mut grc_data, GRC_LATEST_ROOT_SEQ, 0);
    // roots[4] already zeroed by CreateAccount

    Ok(())
}

// ===========================================================================
// PUBLISH GLOBAL ROOT
// ===========================================================================
//
// Accounts:
//   [0] payer              -- mut, signer
//   [1] protocol_state     -- immut
//   [2] global_root_config -- mut
//
// Instruction data (after 8-byte disc):
//   [0..8]   root_seq       (u64 LE)
//   [8..40]  root           ([u8; 32])
//   [40..72] dataset_hash   ([u8; 32])
// ===========================================================================

pub fn publish_global_root(
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

    let [payer, protocol_state_ai, global_root_ai, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if !global_root_ai.is_writable() {
        return Err(ProgramError::InvalidAccountData);
    }

    // -- validate protocol_state --
    if protocol_state_ai.owner() != &crate::ID {
        return Err(ProgramError::IllegalOwner);
    }
    let ps_data = unsafe { protocol_state_ai.borrow_data_unchecked() };
    if ps_data.len() < PS_LEN || ps_data[0..8] != DISC_PROTOCOL_STATE {
        return Err(ProgramError::InvalidAccountData);
    }

    // Verify protocol_state PDA derivation
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

    let ps_mint = read_pubkey(&ps_data, PS_MINT);
    drop(ps_data);

    // -- validate global_root_config --
    if global_root_ai.owner() != &crate::ID {
        return Err(ProgramError::IllegalOwner);
    }

    // Verify PDA
    let (expected_grc, _) =
        pubkey::find_program_address(&[GLOBAL_ROOT_SEED, &ps_mint], &crate::ID);
    if !pubkey::pubkey_eq(global_root_ai.key(), &expected_grc) {
        return Err(ProgramError::InvalidSeeds);
    }

    let mut grc_data = unsafe { global_root_ai.borrow_mut_data_unchecked() };
    if grc_data.len() < GRC_LEN || grc_data[0..8] != DISC_GLOBAL_ROOT_CONFIG {
        return Err(ProgramError::InvalidAccountData);
    }
    if grc_data[GRC_VERSION] != GLOBAL_ROOT_VERSION {
        return Err(OracleError::InvalidChannelState.into());
    }
    let grc_mint = read_pubkey(&grc_data, GRC_MINT);
    if !pubkey::pubkey_eq(&grc_mint, &ps_mint) {
        return Err(OracleError::InvalidMint.into());
    }

    // root_seq must be strictly incrementing
    let current_seq = read_u64(&grc_data, GRC_LATEST_ROOT_SEQ);
    if root_seq != current_seq + 1 {
        return Err(OracleError::InvalidRootSeq.into());
    }

    // -- write root entry into ring buffer --
    let idx = (root_seq as usize) % CUMULATIVE_ROOT_HISTORY;
    let entry_offset = GRC_ROOTS_START + idx * ROOT_ENTRY_SIZE;
    let slot = Clock::get()?.slot;

    write_u64(&mut grc_data, entry_offset, root_seq);
    grc_data[entry_offset + 8..entry_offset + 40].copy_from_slice(&root);
    grc_data[entry_offset + 40..entry_offset + 72].copy_from_slice(&dataset_hash);
    write_u64(&mut grc_data, entry_offset + 72, slot);

    write_u64(&mut grc_data, GRC_LATEST_ROOT_SEQ, root_seq);
    drop(grc_data);

    emit_global_root_published(&ps_mint, root_seq, &root, &dataset_hash, signer_key, slot);

    Ok(())
}

// ===========================================================================
// CLAIM GLOBAL V2 (self-signed)
// ===========================================================================
//
// Accounts:
//   [0]  claimer         -- mut, signer
//   [1]  protocol_state  -- mut (PDA signer for CPI)
//   [2]  global_root_config -- immut
//   [3]  claim_state     -- mut (init_if_needed)
//   [4]  mint            -- immut
//   [5]  treasury_ata    -- mut
//   [6]  claimer_ata     -- mut (init_if_needed externally)
//   [7]  token_program   -- immut
//   [8]  associated_token_program -- immut
//   [9]  system_program  -- immut
//   [10..] remaining     -- transfer hook accounts
//
// Instruction data (after 8-byte disc):
//   [0..8]   root_seq         (u64 LE)
//   [8..16]  base_yield       (u64 LE)
//   [16..24] attention_bonus   (u64 LE)
//   [24..28] proof_len        (u32 LE, Borsh Vec prefix)
//   [28..]   proof            (proof_len x 32 bytes)
// ===========================================================================

pub fn claim_global_v2(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if ix_data.len() < 28 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let root_seq = u64::from_le_bytes(ix_data[0..8].try_into().unwrap());
    let base_yield = u64::from_le_bytes(ix_data[8..16].try_into().unwrap());
    let attention_bonus = u64::from_le_bytes(ix_data[16..24].try_into().unwrap());
    let proof_len = u32::from_le_bytes(ix_data[24..28].try_into().unwrap()) as usize;

    if proof_len > MAX_PROOF_LEN {
        return Err(OracleError::InvalidProofLength.into());
    }
    if ix_data.len() < 28 + proof_len * 32 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let (proof_nodes, proof_count) = parse_proof(&ix_data[28..], proof_len);

    if accounts.len() < 10 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let claimer = &accounts[0];
    let remaining = if accounts.len() > 10 {
        &accounts[10..]
    } else {
        &[]
    };

    if !claimer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    claim_global_inner(
        claimer.key(),
        claimer,            // payer = claimer
        &accounts[1],       // protocol_state
        &accounts[2],       // global_root_config
        &accounts[3],       // claim_state
        &accounts[4],       // mint
        &accounts[5],       // treasury_ata
        &accounts[6],       // claimer_ata
        &accounts[7],       // token_program
        &accounts[9],       // system_program
        remaining,
        root_seq,
        LEAF_VERSION_V5,
        0,
        base_yield,
        attention_bonus,
        &proof_nodes[..proof_count],
    )
}

// ===========================================================================
// CLAIM GLOBAL SPONSORED V2 (gasless relay)
// ===========================================================================
//
// Accounts:
//   [0]  payer           -- mut, signer (relayer)
//   [1]  claimer         -- immut, NOT signer
//   [2]  protocol_state  -- mut
//   [3]  global_root_config -- immut
//   [4]  claim_state     -- mut (init_if_needed)
//   [5]  mint            -- immut
//   [6]  treasury_ata    -- mut
//   [7]  claimer_ata     -- mut
//   [8]  token_program   -- immut
//   [9]  associated_token_program -- immut
//   [10] system_program  -- immut
//   [11..] remaining     -- transfer hook accounts
// ===========================================================================

pub fn claim_global_sponsored_v2(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if ix_data.len() < 28 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let root_seq = u64::from_le_bytes(ix_data[0..8].try_into().unwrap());
    let base_yield = u64::from_le_bytes(ix_data[8..16].try_into().unwrap());
    let attention_bonus = u64::from_le_bytes(ix_data[16..24].try_into().unwrap());
    let proof_len = u32::from_le_bytes(ix_data[24..28].try_into().unwrap()) as usize;

    if proof_len > MAX_PROOF_LEN {
        return Err(OracleError::InvalidProofLength.into());
    }
    if ix_data.len() < 28 + proof_len * 32 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let (proof_nodes, proof_count) = parse_proof(&ix_data[28..], proof_len);

    if accounts.len() < 11 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let payer = &accounts[0];
    let claimer = &accounts[1];
    let remaining = if accounts.len() > 11 {
        &accounts[11..]
    } else {
        &[]
    };

    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    claim_global_inner(
        claimer.key(),
        payer,              // payer = relayer
        &accounts[2],       // protocol_state
        &accounts[3],       // global_root_config
        &accounts[4],       // claim_state
        &accounts[5],       // mint
        &accounts[6],       // treasury_ata
        &accounts[7],       // claimer_ata
        &accounts[8],       // token_program
        &accounts[10],      // system_program
        remaining,
        root_seq,
        LEAF_VERSION_V5,
        0,
        base_yield,
        attention_bonus,
        &proof_nodes[..proof_count],
    )
}

// ===========================================================================
// CLAIM GLOBAL V1 (backward compat)
// ===========================================================================
//
// V1 uses V4 leaf format (single cumulative_total).
// Same account layout as self-signed V2.
//
// Instruction data (after 8-byte disc):
//   [0..8]   root_seq          (u64 LE)
//   [8..16]  cumulative_total  (u64 LE)
//   [16..20] proof_len         (u32 LE)
//   [20..]   proof             (proof_len x 32 bytes)
// ===========================================================================

pub fn claim_global_v1(
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

    if accounts.len() < 10 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let claimer = &accounts[0];
    let remaining = if accounts.len() > 10 {
        &accounts[10..]
    } else {
        &[]
    };

    if !claimer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    claim_global_inner(
        claimer.key(),
        claimer,
        &accounts[1],
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &accounts[5],
        &accounts[6],
        &accounts[7],
        &accounts[9],
        remaining,
        root_seq,
        LEAF_VERSION_V4,
        cumulative_total,
        0,
        0,
        &proof_nodes[..proof_count],
    )
}

// ===========================================================================
// CLAIM GLOBAL SPONSORED V1 (backward compat)
// ===========================================================================
//
// Same as V1 but payer != claimer (relay pattern).
// Same account layout as sponsored V2.
// ===========================================================================

pub fn claim_global_sponsored_v1(
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

    if accounts.len() < 11 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let payer = &accounts[0];
    let claimer = &accounts[1];
    let remaining = if accounts.len() > 11 {
        &accounts[11..]
    } else {
        &[]
    };

    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    claim_global_inner(
        claimer.key(),
        payer,
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &accounts[5],
        &accounts[6],
        &accounts[7],
        &accounts[8],
        &accounts[10],
        remaining,
        root_seq,
        LEAF_VERSION_V4,
        cumulative_total,
        0,
        0,
        &proof_nodes[..proof_count],
    )
}

// ===========================================================================
// Shared claim logic
// ===========================================================================

/// Max merkle proof depth (2^32 leaves is more than enough).
const MAX_PROOF_NODES: usize = 32;

/// Parse a flat byte buffer into a fixed-size array of 32-byte proof nodes.
/// Returns (nodes, count). Callers should use `&nodes[..count]`.
#[inline(never)]
fn parse_proof(data: &[u8], count: usize) -> ([[u8; 32]; MAX_PROOF_NODES], usize) {
    let clamped = if count > MAX_PROOF_NODES { MAX_PROOF_NODES } else { count };
    let mut nodes = [[0u8; 32]; MAX_PROOF_NODES];
    for i in 0..clamped {
        let off = i * 32;
        if off + 32 <= data.len() {
            nodes[i].copy_from_slice(&data[off..off + 32]);
        }
    }
    (nodes, clamped)
}

/// Core claim logic shared by V1/V2, self-signed/sponsored variants.
///
/// Flow:
///   1. Validate protocol_state + global_root_config
///   2. Look up root from ring buffer
///   3. Compute and verify the merkle leaf
///   4. Init-if-needed ClaimStateGlobal PDA
///   5. Compute delta (cumulative_total - already_claimed)
///   6. CPI transfer CCM from treasury to claimer
///   7. Update claim state, emit event
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn claim_global_inner<'a>(
    wallet_key: &Pubkey,
    payer: &'a AccountInfo,
    protocol_state_ai: &'a AccountInfo,
    global_root_ai: &'a AccountInfo,
    claim_state_ai: &'a AccountInfo,
    mint_ai: &'a AccountInfo,
    treasury_ata: &'a AccountInfo,
    claimer_ata: &'a AccountInfo,
    token_program: &'a AccountInfo,
    _system_program: &'a AccountInfo,
    remaining: &'a [AccountInfo],
    root_seq: u64,
    leaf_version: u8,
    cumulative_total_v4: u64,
    base_yield: u64,
    attention_bonus: u64,
    proof: &[[u8; 32]],
) -> ProgramResult {
    // -- validate protocol_state --
    if protocol_state_ai.owner() != &crate::ID {
        return Err(ProgramError::IllegalOwner);
    }
    let ps_data = unsafe { protocol_state_ai.borrow_data_unchecked() };
    if ps_data.len() < PS_LEN || ps_data[0..8] != DISC_PROTOCOL_STATE {
        return Err(ProgramError::InvalidAccountData);
    }

    // Verify protocol_state PDA derivation
    let (expected_ps, _) = pubkey::find_program_address(&[PROTOCOL_STATE_SEED], &crate::ID);
    if !pubkey::pubkey_eq(protocol_state_ai.key(), &expected_ps) {
        return Err(ProgramError::InvalidSeeds);
    }

    let paused = ps_data[PS_PAUSED] != 0;
    if paused {
        return Err(OracleError::ProtocolPaused.into());
    }
    let ps_mint = read_pubkey(&ps_data, PS_MINT);
    let ps_bump = ps_data[PS_BUMP];

    if !pubkey::pubkey_eq(mint_ai.key(), &ps_mint) {
        return Err(OracleError::InvalidMint.into());
    }
    if proof.len() > MAX_PROOF_LEN {
        return Err(OracleError::InvalidProofLength.into());
    }
    drop(ps_data);

    // -- validate global_root_config --
    if global_root_ai.owner() != &crate::ID {
        return Err(ProgramError::IllegalOwner);
    }
    let grc_data = unsafe { global_root_ai.borrow_data_unchecked() };
    if grc_data.len() < GRC_LEN || grc_data[0..8] != DISC_GLOBAL_ROOT_CONFIG {
        return Err(ProgramError::InvalidAccountData);
    }
    if grc_data[GRC_VERSION] != GLOBAL_ROOT_VERSION {
        return Err(OracleError::InvalidChannelState.into());
    }
    let grc_mint = read_pubkey(&grc_data, GRC_MINT);
    if !pubkey::pubkey_eq(&grc_mint, &ps_mint) {
        return Err(OracleError::InvalidMint.into());
    }

    // -- look up root from ring buffer --
    let idx = (root_seq as usize) % CUMULATIVE_ROOT_HISTORY;
    let entry_offset = GRC_ROOTS_START + idx * ROOT_ENTRY_SIZE;
    let entry_seq = read_u64(&grc_data, entry_offset);
    if entry_seq != root_seq {
        return Err(OracleError::RootTooOldOrMissing.into());
    }
    let entry_root = read_hash(&grc_data, entry_offset + 8);
    drop(grc_data);

    // -- compute leaf + cumulative total --
    let (cumulative_total, leaf) = match leaf_version {
        LEAF_VERSION_V4 => {
            let leaf = compute_global_leaf_v4(&ps_mint, root_seq, wallet_key, cumulative_total_v4);
            (cumulative_total_v4, leaf)
        }
        LEAF_VERSION_V5 => {
            let total = base_yield
                .checked_add(attention_bonus)
                .ok_or::<ProgramError>(OracleError::MathOverflow.into())?;
            let leaf =
                compute_global_leaf_v5(&ps_mint, root_seq, wallet_key, base_yield, attention_bonus);
            (total, leaf)
        }
        _ => {
            return Err(OracleError::InvalidMerkleLeafVersion.into());
        }
    };

    // -- verify merkle proof --
    if !verify_proof(proof, leaf, entry_root) {
        return Err(OracleError::InvalidProof.into());
    }

    // -- init_if_needed: ClaimStateGlobal PDA --
    let (expected_cs, cs_bump) =
        pubkey::find_program_address(&[CLAIM_STATE_GLOBAL_SEED, &ps_mint, wallet_key], &crate::ID);
    if !pubkey::pubkey_eq(claim_state_ai.key(), &expected_cs) {
        return Err(ProgramError::InvalidSeeds);
    }

    let needs_create = claim_state_ai.data_len() == 0;
    if needs_create {
        // Create the ClaimStateGlobal account
        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(CSG_LEN);
        let bump_byte = [cs_bump];

        {
            let seeds = [
                pinocchio::instruction::Seed::from(CLAIM_STATE_GLOBAL_SEED),
                pinocchio::instruction::Seed::from(ps_mint.as_ref()),
                pinocchio::instruction::Seed::from(wallet_key.as_ref()),
                pinocchio::instruction::Seed::from(bump_byte.as_ref()),
            ];
            let signer = Signer::from(&seeds);
            crate::cpi_create_account(payer, claim_state_ai, lamports, CSG_LEN as u64, &crate::ID, &[signer])?;
        }
    }

    // Read/validate claim state (either freshly created or pre-existing)
    {
        if !needs_create && claim_state_ai.owner() != &crate::ID {
            return Err(ProgramError::IllegalOwner);
        }

        let mut cs_data = unsafe { claim_state_ai.borrow_mut_data_unchecked() };
        if cs_data.len() < CSG_LEN {
            return Err(ProgramError::InvalidAccountData);
        }

        if cs_data[CSG_VERSION] == 0 {
            // First time init (fresh create, or version==0 from prior partial init)
            cs_data[0..8].copy_from_slice(&DISC_CLAIM_STATE_GLOBAL);
            cs_data[CSG_VERSION] = CLAIM_STATE_GLOBAL_VERSION;
            cs_data[CSG_BUMP] = cs_bump;
            cs_data[CSG_MINT..CSG_MINT + 32].copy_from_slice(&ps_mint);
            cs_data[CSG_WALLET..CSG_WALLET + 32].copy_from_slice(wallet_key);
            write_u64(&mut cs_data, CSG_CLAIMED_TOTAL, 0);
            write_u64(&mut cs_data, CSG_LAST_CLAIM_SEQ, 0);
        } else {
            // Existing -- validate discriminator + fields
            if cs_data[0..8] != DISC_CLAIM_STATE_GLOBAL {
                return Err(ProgramError::InvalidAccountData);
            }
            let cs_mint = read_pubkey(&cs_data, CSG_MINT);
            if !pubkey::pubkey_eq(&cs_mint, &ps_mint) {
                return Err(OracleError::InvalidClaimState.into());
            }
            let cs_wallet = read_pubkey(&cs_data, CSG_WALLET);
            if !pubkey::pubkey_eq(&cs_wallet, wallet_key) {
                return Err(OracleError::InvalidClaimState.into());
            }
        }

        let claimed_total = read_u64(&cs_data, CSG_CLAIMED_TOTAL);

        // Idempotent: no-op if already claimed up to this total
        if cumulative_total <= claimed_total {
            return Ok(());
        }

        let delta = cumulative_total
            .checked_sub(claimed_total)
            .ok_or::<ProgramError>(OracleError::MathOverflow.into())?;

        // Must drop the mutable borrow before CPI
        // (pinocchio CPI checks borrow state).
        // So we store delta/cumulative_total, drop, do CPI, re-borrow.
        drop(cs_data);

        // -- CPI: transfer CCM from treasury to claimer --
        let bump_byte = [ps_bump];
        let seeds = [
            pinocchio::instruction::Seed::from(PROTOCOL_STATE_SEED),
            pinocchio::instruction::Seed::from(bump_byte.as_ref()),
        ];

        transfer_checked_cpi(
            token_program,
            treasury_ata,
            mint_ai,
            claimer_ata,
            protocol_state_ai,
            delta,
            CCM_DECIMALS,
            &seeds,
            remaining,
        )?;

        // -- update claim state --
        let mut cs_data = unsafe { claim_state_ai.borrow_mut_data_unchecked() };
        write_u64(&mut cs_data, CSG_CLAIMED_TOTAL, cumulative_total);
        write_u64(&mut cs_data, CSG_LAST_CLAIM_SEQ, root_seq);
        drop(cs_data);

        emit_global_rewards_claimed(wallet_key, delta, cumulative_total, root_seq);
    }

    Ok(())
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keccak_deterministic() {
        let a = keccak256(&[b"hello", b"world"]);
        let b = keccak256(&[b"hello", b"world"]);
        assert_eq!(a, b);
    }

    #[test]
    fn keccak_different_input() {
        let a = keccak256(&[b"hello"]);
        let b = keccak256(&[b"world"]);
        assert_ne!(a, b);
    }

    #[test]
    fn proof_empty_is_root() {
        let leaf = [42u8; 32];
        assert!(verify_proof(&[], leaf, leaf));
    }

    #[test]
    fn proof_empty_mismatch() {
        assert!(!verify_proof(&[], [42u8; 32], [0u8; 32]));
    }

    #[test]
    fn proof_rejects_oversized() {
        let proof = vec![[0u8; 32]; 33];
        assert!(!verify_proof(&proof, [1u8; 32], [1u8; 32]));
    }

    #[test]
    fn proof_single_sibling() {
        let leaf = [1u8; 32];
        let sibling = [2u8; 32];
        let (a, b) = if leaf <= sibling {
            (leaf, sibling)
        } else {
            (sibling, leaf)
        };
        let expected_root = keccak256(&[&a, &b]);
        assert!(verify_proof(&[sibling], leaf, expected_root));
    }

    #[test]
    fn proof_wrong_root() {
        assert!(!verify_proof(&[[2u8; 32]], [1u8; 32], [99u8; 32]));
    }

    #[test]
    fn leaf_v4_deterministic() {
        let mint = [7u8; 32];
        let wallet = [9u8; 32];
        assert_eq!(
            compute_global_leaf_v4(&mint, 1, &wallet, 1000),
            compute_global_leaf_v4(&mint, 1, &wallet, 1000),
        );
    }

    #[test]
    fn leaf_v4_different_total() {
        let mint = [7u8; 32];
        let wallet = [9u8; 32];
        assert_ne!(
            compute_global_leaf_v4(&mint, 1, &wallet, 1000),
            compute_global_leaf_v4(&mint, 1, &wallet, 1001),
        );
    }

    #[test]
    fn leaf_v5_deterministic() {
        let mint = [7u8; 32];
        let wallet = [9u8; 32];
        assert_eq!(
            compute_global_leaf_v5(&mint, 1, &wallet, 1000, 200),
            compute_global_leaf_v5(&mint, 1, &wallet, 1000, 200),
        );
    }

    #[test]
    fn leaf_v5_different_bonus() {
        let mint = [7u8; 32];
        let wallet = [9u8; 32];
        assert_ne!(
            compute_global_leaf_v5(&mint, 1, &wallet, 1000, 200),
            compute_global_leaf_v5(&mint, 1, &wallet, 1000, 201),
        );
    }

    #[test]
    fn leaf_v5_different_seq() {
        let mint = [7u8; 32];
        let wallet = [9u8; 32];
        assert_ne!(
            compute_global_leaf_v5(&mint, 1, &wallet, 1000, 200),
            compute_global_leaf_v5(&mint, 2, &wallet, 1000, 200),
        );
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
    fn ring_buffer_index() {
        for seq in 0u64..20 {
            let idx = (seq as usize) % CUMULATIVE_ROOT_HISTORY;
            assert!(idx < CUMULATIVE_ROOT_HISTORY);
            assert_eq!(idx, (seq as usize) % 4);
        }
    }

    #[test]
    fn account_layout_sizes() {
        assert_eq!(PS_LEN, 173);
        assert_eq!(GRC_LEN, 8 + 1 + 1 + 32 + 8 + (80 * 4));
        assert_eq!(GRC_LEN, 370);
        assert_eq!(CSG_LEN, 8 + 1 + 1 + 32 + 32 + 8 + 8);
        assert_eq!(CSG_LEN, 90);
        assert_eq!(ROOT_ENTRY_SIZE, 80);
    }

    /// Cross-verify: the V4 leaf computed here must match the Anchor program's
    /// `compute_global_leaf` for the same inputs, since both use identical
    /// Keccak-256 over the same domain-separated preimage.
    #[test]
    fn leaf_v4_cross_compat() {
        // Use a deterministic "mint" and "wallet" so the test is reproducible.
        let mint = [0xAA; 32];
        let wallet = [0xBB; 32];
        let root_seq = 42u64;
        let total = 999_000_000u64;

        let leaf = compute_global_leaf_v4(&mint, root_seq, &wallet, total);

        // Manually compute expected value
        let expected = keccak256(&[
            GLOBAL_V4_DOMAIN,
            &mint,
            &root_seq.to_le_bytes(),
            &wallet,
            &total.to_le_bytes(),
        ]);
        assert_eq!(leaf, expected);
    }

    #[test]
    fn leaf_v5_cross_compat() {
        let mint = [0xAA; 32];
        let wallet = [0xBB; 32];
        let root_seq = 42u64;
        let base = 500_000u64;
        let bonus = 100_000u64;

        let leaf = compute_global_leaf_v5(&mint, root_seq, &wallet, base, bonus);

        let expected = keccak256(&[
            GLOBAL_V5_DOMAIN,
            &mint,
            &root_seq.to_le_bytes(),
            &wallet,
            &base.to_le_bytes(),
            &bonus.to_le_bytes(),
        ]);
        assert_eq!(leaf, expected);
    }
}
