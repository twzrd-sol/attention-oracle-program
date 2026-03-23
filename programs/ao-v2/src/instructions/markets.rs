//! Prediction markets instructions (feature-gated behind `prediction_markets`).
//!
//! Ported from the Anchor `attention-oracle` program's `markets.rs`.
//! Wire-compatible: same discriminators, same PDA seeds, same account layouts.
//!
//! Instruction set (9 total):
//!   - create_market               (disc 67e261ebc8bcfbfe)
//!   - initialize_market_tokens    (disc 6e86b40513975049)
//!   - initialize_market_tokens_v2 (disc b4a858f274f7e569)
//!   - mint_shares                 (disc 18c48400b79ed88e)
//!   - redeem_shares               (disc ef9ae059f0c42abb)
//!   - resolve_market              (disc 9b1750ad2e4a17ef)
//!   - settle                      (disc af2ab957908366d4)
//!   - sweep_residual              (disc e6762399ba56e8d13) -- note: first 8 bytes
//!   - close_market                (disc 589af8ba300e7bf4)
//!   - close_market_mints          (disc dee74c62770674b6)

use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Seed, Signer},
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    ProgramResult,
};

use crate::error::OracleError;
use crate::keccak::keccak256;
use crate::state::{
    DISC_GLOBAL_ROOT_CONFIG, DISC_MARKET_STATE, DISC_PROTOCOL_STATE, GLOBAL_ROOT_SEED,
    MARKET_MINT_AUTHORITY_SEED, MARKET_NO_MINT_SEED, MARKET_STATE_SEED, MARKET_YES_MINT_SEED,
    PM_VAULT_SEED, PROTOCOL_STATE_SEED,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MARKET_STATE_VERSION: u8 = 1;
const MAX_PROOF_LEN: usize = 32;
const CCM_DECIMALS: u8 = 9;
const CUMULATIVE_ROOT_HISTORY: usize = 4;
const MARKET_METRIC_ATTENTION_SCORE: u8 = 0;
const GLOBAL_V4_DOMAIN: &[u8] = b"TWZRD:GLOBAL_V4";
const ZERO_PUBKEY: Pubkey = [0u8; 32];

// ProtocolState byte offsets
const PS_LEN: usize = 173;
const PS_ADMIN: usize = 10;
const PS_PUBLISHER: usize = 42;
const PS_MINT: usize = 138;
const PS_PAUSED: usize = 170;
const PS_BUMP: usize = 172;

// GlobalRootConfig byte offsets
const GRC_LEN: usize = 370;
const GRC_VERSION: usize = 8;
const GRC_BUMP: usize = 9;
const GRC_MINT: usize = 10;
const GRC_LATEST_ROOT_SEQ: usize = 42;
const GRC_ROOTS_START: usize = 50;
const ROOT_ENTRY_SIZE: usize = 80;

// MarketState byte offsets (within the 288-byte account)
const MS_LEN: usize = 288;
const MS_VERSION: usize = 8;
const MS_BUMP: usize = 9;
const MS_METRIC: usize = 10;
const MS_RESOLVED: usize = 11;
const MS_OUTCOME: usize = 12;
const MS_TOKENS_INIT: usize = 13;
const MS_MARKET_ID: usize = 16;
const MS_MINT: usize = 24;
const MS_AUTHORITY: usize = 56;
const MS_CREATOR_WALLET: usize = 88;
const MS_TARGET: usize = 120;
const MS_RES_ROOT_SEQ: usize = 128;
const MS_RES_CUM_TOTAL: usize = 136;
const MS_CREATED_SLOT: usize = 144;
const MS_RESOLVED_SLOT: usize = 152;
const MS_VAULT: usize = 160;
const MS_YES_MINT: usize = 192;
const MS_NO_MINT: usize = 224;
const MS_MINT_AUTHORITY: usize = 256;

// Token-2022 program ID
use crate::SPL_TOKEN_ID;
use crate::TOKEN_2022_ID;

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
fn read_pubkey(data: &[u8], offset: usize) -> Pubkey {
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&data[offset..offset + 32]);
    pk
}

/// SPL Mint layout: supply at offset 36 (u64 LE).
#[inline(always)]
fn read_mint_supply(data: &[u8]) -> u64 {
    read_u64(data, 36)
}

/// Token account amount at offset 64 (u64 LE).
#[inline(always)]
fn read_token_amount(data: &[u8]) -> u64 {
    read_u64(data, 64)
}

// ---------------------------------------------------------------------------
// Merkle helpers (copied from global.rs to keep markets self-contained)
// ---------------------------------------------------------------------------

/// Verify a merkle proof against a known root.
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

// ---------------------------------------------------------------------------
// Token CPI helpers
// ---------------------------------------------------------------------------

/// Build and invoke a Token-2022 `transfer_checked` CPI with PDA signer
/// and remaining accounts (for transfer-fee hooks).
#[inline(never)]
fn transfer_checked_signed<'a>(
    token_program: &'a AccountInfo,
    from: &'a AccountInfo,
    mint: &'a AccountInfo,
    to: &'a AccountInfo,
    authority: &'a AccountInfo,
    amount: u64,
    decimals: u8,
    signer_seeds: &[Seed],
    remaining: &'a [AccountInfo],
) -> ProgramResult {
    // TransferChecked instruction data: [12, amount(8), decimals(1)]
    let mut data = [0u8; 10];
    data[0] = 12; // TransferChecked discriminator
    data[1..9].copy_from_slice(&amount.to_le_bytes());
    data[9] = decimals;

    // 4 fixed accounts + remaining
    let n_fixed = 4;
    let n_total = n_fixed + remaining.len();
    if n_total > 36 {
        return Err(ProgramError::InvalidArgument);
    }

    let mut metas_buf: [core::mem::MaybeUninit<AccountMeta>; 36] =
        unsafe { core::mem::MaybeUninit::uninit().assume_init() };
    metas_buf[0].write(AccountMeta::writable(from.key()));
    metas_buf[1].write(AccountMeta::readonly(mint.key()));
    metas_buf[2].write(AccountMeta::writable(to.key()));
    metas_buf[3].write(AccountMeta::readonly_signer(authority.key()));
    for (i, acc) in remaining.iter().enumerate() {
        // Preserve caller-provided writable/signer flags for transfer hooks.
        metas_buf[n_fixed + i].write(AccountMeta::new(
            acc.key(),
            acc.is_writable(),
            acc.is_signer(),
        ));
    }
    let metas =
        unsafe { core::slice::from_raw_parts(metas_buf.as_ptr() as *const AccountMeta, n_total) };

    let mut refs_buf: [core::mem::MaybeUninit<&AccountInfo>; 36] =
        unsafe { core::mem::MaybeUninit::uninit().assume_init() };
    refs_buf[0].write(from);
    refs_buf[1].write(mint);
    refs_buf[2].write(to);
    refs_buf[3].write(authority);
    for (i, acc) in remaining.iter().enumerate() {
        refs_buf[n_fixed + i].write(acc);
    }
    let refs =
        unsafe { core::slice::from_raw_parts(refs_buf.as_ptr() as *const &AccountInfo, n_total) };

    let ix = Instruction {
        program_id: token_program.key(),
        accounts: metas,
        data: &data,
    };

    let signer = Signer::from(signer_seeds);
    pinocchio::cpi::slice_invoke_signed(&ix, refs, &[signer])
}

/// MintTo CPI (for SPL or Token-2022 outcome mints).
#[inline(never)]
fn mint_to_cpi<'a>(
    token_program: &'a AccountInfo,
    mint_account: &'a AccountInfo,
    to: &'a AccountInfo,
    authority: &'a AccountInfo,
    amount: u64,
    signer_seeds: &[Seed],
) -> ProgramResult {
    // MintTo: discriminator = 7
    let mut data = [0u8; 9];
    data[0] = 7;
    data[1..9].copy_from_slice(&amount.to_le_bytes());

    let metas = [
        AccountMeta::writable(mint_account.key()),
        AccountMeta::writable(to.key()),
        AccountMeta::readonly_signer(authority.key()),
    ];

    let ix = Instruction {
        program_id: token_program.key(),
        accounts: &metas,
        data: &data,
    };

    let signer = Signer::from(signer_seeds);
    pinocchio::cpi::slice_invoke_signed(&ix, &[mint_account, to, authority], &[signer])
}

/// Burn CPI (user signs directly, no PDA signer needed).
#[inline(never)]
fn burn_cpi<'a>(
    token_program: &'a AccountInfo,
    mint_account: &'a AccountInfo,
    from: &'a AccountInfo,
    authority: &'a AccountInfo,
    amount: u64,
) -> ProgramResult {
    // Burn: discriminator = 8
    let mut data = [0u8; 9];
    data[0] = 8;
    data[1..9].copy_from_slice(&amount.to_le_bytes());

    let metas = [
        AccountMeta::writable(from.key()),
        AccountMeta::writable(mint_account.key()),
        AccountMeta::readonly_signer(authority.key()),
    ];

    let ix = Instruction {
        program_id: token_program.key(),
        accounts: &metas,
        data: &data,
    };

    // No PDA signer -- user is signer
    pinocchio::cpi::slice_invoke_signed(&ix, &[from, mint_account, authority], &[])
}

/// CloseAccount CPI (PDA-signed).
#[inline(never)]
fn close_account_cpi<'a>(
    token_program: &'a AccountInfo,
    account: &'a AccountInfo,
    destination: &'a AccountInfo,
    authority: &'a AccountInfo,
    signer_seeds: &[Seed],
) -> ProgramResult {
    // CloseAccount: discriminator = 9
    let data = [9u8];

    let metas = [
        AccountMeta::writable(account.key()),
        AccountMeta::writable(destination.key()),
        AccountMeta::readonly_signer(authority.key()),
    ];

    let ix = Instruction {
        program_id: token_program.key(),
        accounts: &metas,
        data: &data,
    };

    let signer = Signer::from(signer_seeds);
    pinocchio::cpi::slice_invoke_signed(&ix, &[account, destination, authority], &[signer])
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// Validate the ProtocolState PDA and return its data.
/// Returns (ps_mint, ps_bump, is_paused).
#[inline(never)]
fn validate_protocol_state(
    protocol_state: &AccountInfo,
    program_id: &Pubkey,
) -> Result<(Pubkey, u8), ProgramError> {
    if !protocol_state.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let data = unsafe { protocol_state.borrow_data_unchecked() };
    if data.len() < PS_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    if data[..8] != DISC_PROTOCOL_STATE {
        return Err(ProgramError::InvalidAccountData);
    }
    let bump = data[PS_BUMP];
    let ps_mint = read_pubkey(&data, PS_MINT);
    // Verify PDA
    let pda = pubkey::create_program_address(&[PROTOCOL_STATE_SEED, &[bump]], program_id)?;
    if !pubkey::pubkey_eq(&pda, protocol_state.key()) {
        return Err(ProgramError::InvalidSeeds);
    }
    Ok((ps_mint, bump))
}

/// Check protocol is not paused.
#[inline(always)]
fn check_not_paused(protocol_state: &AccountInfo) -> Result<(), ProgramError> {
    let data = unsafe { protocol_state.borrow_data_unchecked() };
    if data[PS_PAUSED] != 0 {
        return Err(OracleError::ProtocolPaused.into());
    }
    Ok(())
}

/// Check authority is admin or publisher of protocol_state.
#[inline(always)]
fn check_admin_or_publisher(
    authority: &AccountInfo,
    protocol_state: &AccountInfo,
) -> Result<(), ProgramError> {
    let data = unsafe { protocol_state.borrow_data_unchecked() };
    let admin = read_pubkey(&data, PS_ADMIN);
    let publisher = read_pubkey(&data, PS_PUBLISHER);
    if !pubkey::pubkey_eq(authority.key(), &admin)
        && !pubkey::pubkey_eq(authority.key(), &publisher)
    {
        return Err(OracleError::Unauthorized.into());
    }
    Ok(())
}

/// Check authority is admin of protocol_state.
#[inline(always)]
fn check_admin(authority: &AccountInfo, protocol_state: &AccountInfo) -> Result<(), ProgramError> {
    let data = unsafe { protocol_state.borrow_data_unchecked() };
    let admin = read_pubkey(&data, PS_ADMIN);
    if !pubkey::pubkey_eq(authority.key(), &admin) {
        return Err(OracleError::Unauthorized.into());
    }
    Ok(())
}

/// Derive and verify the MarketState PDA. Returns bump.
#[inline(never)]
fn verify_market_state_pda(
    market_state: &AccountInfo,
    mint: &Pubkey,
    market_id: u64,
    program_id: &Pubkey,
) -> Result<u8, ProgramError> {
    let market_id_bytes = market_id.to_le_bytes();
    let (expected, bump) =
        pubkey::find_program_address(&[MARKET_STATE_SEED, mint, &market_id_bytes], program_id);
    if !pubkey::pubkey_eq(market_state.key(), &expected) {
        return Err(ProgramError::InvalidSeeds);
    }
    Ok(bump)
}

/// Derive the mint_authority PDA and return (pda, bump).
#[inline(never)]
fn derive_mint_authority(mint: &Pubkey, market_id: u64, program_id: &Pubkey) -> (Pubkey, u8) {
    let market_id_bytes = market_id.to_le_bytes();
    pubkey::find_program_address(
        &[MARKET_MINT_AUTHORITY_SEED, mint, &market_id_bytes],
        program_id,
    )
}

/// Build mint_authority PDA signer seeds (stack-friendly).
#[inline(always)]
fn mint_auth_seeds<'a>(
    seed: &'a [u8],
    mint: &'a [u8],
    market_id_bytes: &'a [u8],
    bump: &'a [u8],
) -> [Seed<'a>; 4] {
    [
        Seed::from(seed),
        Seed::from(mint),
        Seed::from(market_id_bytes),
        Seed::from(bump),
    ]
}

// ============================================================================
// 1. CREATE MARKET
// ============================================================================
//
// Accounts:
//   0. [SIGNER, WRITE] authority
//   1. []               protocol_state PDA (seeds=["protocol_state"])
//   2. []               global_root_config PDA (seeds=["global_root", mint])
//   3. [WRITE]          market_state PDA (will be created)
//   4. []               system_program
//
// Instruction data:
//   [0..8]    market_id (u64 LE)
//   [8..40]   creator_wallet (Pubkey)
//   [40]      metric (u8)
//   [41..49]  target (u64 LE)
//   [49..57]  resolution_root_seq (u64 LE)

#[inline(never)]
pub fn create_market(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 5 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    if ix_data.len() < 57 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let authority = &accounts[0];
    let protocol_state = &accounts[1];
    let global_root_config = &accounts[2];
    let market_state = &accounts[3];
    let _system_program = &accounts[4];

    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Validate protocol state
    let (ps_mint, _ps_bump) = validate_protocol_state(protocol_state, program_id)?;
    check_not_paused(protocol_state)?;
    check_admin_or_publisher(authority, protocol_state)?;

    // Parse instruction data
    let market_id = read_u64(ix_data, 0);
    let creator_wallet = read_pubkey(ix_data, 8);
    let metric = ix_data[40];
    let target = read_u64(ix_data, 41);
    let resolution_root_seq = read_u64(ix_data, 49);

    // Validate inputs
    if pubkey::pubkey_eq(&creator_wallet, &ZERO_PUBKEY) {
        return Err(OracleError::InvalidPubkey.into());
    }
    if metric != MARKET_METRIC_ATTENTION_SCORE {
        return Err(OracleError::UnsupportedMarketMetric.into());
    }
    if resolution_root_seq == 0 {
        return Err(OracleError::InvalidRootSeq.into());
    }

    // Validate global root config
    validate_global_root_config(global_root_config, &ps_mint, program_id)?;

    // Derive MarketState PDA
    let market_id_bytes = market_id.to_le_bytes();
    let (expected_market, ms_bump) =
        pubkey::find_program_address(&[MARKET_STATE_SEED, &ps_mint, &market_id_bytes], program_id);
    if !pubkey::pubkey_eq(market_state.key(), &expected_market) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Create MarketState account
    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(MS_LEN);
    let bump_ref = [ms_bump];
    let seeds = [
        Seed::from(MARKET_STATE_SEED),
        Seed::from(ps_mint.as_ref()),
        Seed::from(market_id_bytes.as_ref()),
        Seed::from(bump_ref.as_ref()),
    ];
    let pda_signer = Signer::from(&seeds);

    crate::cpi_create_account(
        authority,
        market_state,
        lamports,
        MS_LEN as u64,
        program_id,
        &[pda_signer],
    )?;

    // Write MarketState data
    let slot = Clock::get()?.slot;
    {
        let data = unsafe { market_state.borrow_mut_data_unchecked() };
        data[..8].copy_from_slice(&DISC_MARKET_STATE);
        data[MS_VERSION] = MARKET_STATE_VERSION;
        data[MS_BUMP] = ms_bump;
        data[MS_METRIC] = metric;
        data[MS_RESOLVED] = 0;
        data[MS_OUTCOME] = 0;
        data[MS_TOKENS_INIT] = 0;
        data[14..16].copy_from_slice(&[0u8; 2]); // _padding
        data[MS_MARKET_ID..MS_MARKET_ID + 8].copy_from_slice(&market_id.to_le_bytes());
        data[MS_MINT..MS_MINT + 32].copy_from_slice(&ps_mint);
        data[MS_AUTHORITY..MS_AUTHORITY + 32].copy_from_slice(authority.key());
        data[MS_CREATOR_WALLET..MS_CREATOR_WALLET + 32].copy_from_slice(&creator_wallet);
        data[MS_TARGET..MS_TARGET + 8].copy_from_slice(&target.to_le_bytes());
        data[MS_RES_ROOT_SEQ..MS_RES_ROOT_SEQ + 8]
            .copy_from_slice(&resolution_root_seq.to_le_bytes());
        data[MS_RES_CUM_TOTAL..MS_RES_CUM_TOTAL + 8].copy_from_slice(&0u64.to_le_bytes());
        data[MS_CREATED_SLOT..MS_CREATED_SLOT + 8].copy_from_slice(&slot.to_le_bytes());
        data[MS_RESOLVED_SLOT..MS_RESOLVED_SLOT + 8].copy_from_slice(&0u64.to_le_bytes());
        // Token fields zeroed (already zeroed by CreateAccount)
    }

    Ok(())
}

/// Validate global root config PDA.
#[inline(never)]
fn validate_global_root_config(
    grc: &AccountInfo,
    mint: &Pubkey,
    program_id: &Pubkey,
) -> Result<(), ProgramError> {
    if !grc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let data = unsafe { grc.borrow_data_unchecked() };
    if data.len() < GRC_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    if data[..8] != DISC_GLOBAL_ROOT_CONFIG {
        return Err(ProgramError::InvalidAccountData);
    }
    let version = data[GRC_VERSION];
    if version == 0 {
        return Err(OracleError::GlobalRootNotInitialized.into());
    }
    let grc_bump = data[GRC_BUMP];
    let grc_mint = read_pubkey(&data, GRC_MINT);
    if !pubkey::pubkey_eq(&grc_mint, mint) {
        return Err(OracleError::InvalidMint.into());
    }
    // Verify PDA
    let pda = pubkey::create_program_address(&[GLOBAL_ROOT_SEED, mint, &[grc_bump]], program_id)?;
    if !pubkey::pubkey_eq(grc.key(), &pda) {
        return Err(ProgramError::InvalidSeeds);
    }
    Ok(())
}

// ============================================================================
// 2. INITIALIZE MARKET TOKENS (legacy SPL mints)
// ============================================================================
//
// Creates vault (Token-2022 ATA for CCM) + YES/NO mints (standard SPL).
// This is the legacy V1 path. New markets should use V2.
//
// Accounts:
//   0. [SIGNER, WRITE] payer
//   1. []               protocol_state PDA
//   2. [WRITE]          market_state PDA
//   3. []               ccm_mint (Token-2022)
//   4. [WRITE]          vault (PDA token account, will be init)
//   5. [WRITE]          yes_mint (PDA, will be init)
//   6. [WRITE]          no_mint (PDA, will be init)
//   7. []               mint_authority (PDA, no data)
//   8. []               token_program (Token-2022, for vault)
//   9. []               standard_token_program (SPL Token, for YES/NO)
//  10. []               system_program
//  11. []               rent sysvar

#[inline(never)]
pub fn initialize_market_tokens(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 12 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let payer = &accounts[0];
    let protocol_state = &accounts[1];
    let market_state = &accounts[2];
    let ccm_mint = &accounts[3];
    let vault = &accounts[4];
    let yes_mint = &accounts[5];
    let no_mint = &accounts[6];
    let mint_authority = &accounts[7];
    let token_program = &accounts[8];
    let standard_token_program = &accounts[9];
    let _system_program = &accounts[10];
    let _rent = &accounts[11];

    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let (ps_mint, _) = validate_protocol_state(protocol_state, program_id)?;
    check_not_paused(protocol_state)?;

    // Read market_state to get market_id and authority
    if !market_state.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let market_id = {
        let data = unsafe { market_state.borrow_data_unchecked() };
        if data.len() < MS_LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        if data[..8] != DISC_MARKET_STATE {
            return Err(ProgramError::InvalidAccountData);
        }
        let auth = read_pubkey(&data, MS_AUTHORITY);
        if !pubkey::pubkey_eq(payer.key(), &auth) {
            return Err(OracleError::Unauthorized.into());
        }
        if data[MS_TOKENS_INIT] != 0 {
            return Err(OracleError::MarketTokensAlreadyInitialized.into());
        }
        read_u64(&data, MS_MARKET_ID)
    };

    // Verify market_state PDA
    let _ = verify_market_state_pda(market_state, &ps_mint, market_id, program_id)?;

    // Verify ccm_mint matches protocol_state.mint
    if !pubkey::pubkey_eq(ccm_mint.key(), &ps_mint) {
        return Err(OracleError::InvalidMint.into());
    }

    // token_program must be Token-2022
    if !pubkey::pubkey_eq(token_program.key(), &TOKEN_2022_ID) {
        return Err(OracleError::InvalidTokenProgram.into());
    }
    // standard_token_program must be SPL Token
    if !pubkey::pubkey_eq(standard_token_program.key(), &SPL_TOKEN_ID) {
        return Err(OracleError::InvalidTokenProgram.into());
    }

    let market_id_bytes = market_id.to_le_bytes();
    let (expected_mint_auth, _mint_auth_bump) =
        derive_mint_authority(&ps_mint, market_id, program_id);
    if !pubkey::pubkey_eq(mint_authority.key(), &expected_mint_auth) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Init vault (Token-2022 token account for CCM, owned by mint_authority PDA)
    init_vault_ata(
        payer,
        vault,
        ccm_mint,
        mint_authority,
        token_program,
        &ps_mint,
        &market_id_bytes,
        program_id,
    )?;

    // Init YES mint (standard SPL)
    init_spl_mint(
        payer,
        yes_mint,
        mint_authority,
        standard_token_program,
        MARKET_YES_MINT_SEED,
        &ps_mint,
        &market_id_bytes,
        program_id,
    )?;

    // Init NO mint (standard SPL)
    init_spl_mint(
        payer,
        no_mint,
        mint_authority,
        standard_token_program,
        MARKET_NO_MINT_SEED,
        &ps_mint,
        &market_id_bytes,
        program_id,
    )?;

    // Update MarketState
    {
        let data = unsafe { market_state.borrow_mut_data_unchecked() };
        data[MS_VAULT..MS_VAULT + 32].copy_from_slice(vault.key());
        data[MS_YES_MINT..MS_YES_MINT + 32].copy_from_slice(yes_mint.key());
        data[MS_NO_MINT..MS_NO_MINT + 32].copy_from_slice(no_mint.key());
        data[MS_MINT_AUTHORITY..MS_MINT_AUTHORITY + 32].copy_from_slice(mint_authority.key());
        data[MS_TOKENS_INIT] = 1;
    }

    Ok(())
}

/// Initialize a Token-2022 token account (vault ATA) as PDA.
#[inline(never)]
fn init_vault_ata<'a>(
    payer: &'a AccountInfo,
    vault: &'a AccountInfo,
    mint: &'a AccountInfo,
    owner: &'a AccountInfo,
    token_program: &'a AccountInfo,
    ps_mint: &[u8],
    market_id_bytes: &[u8],
    program_id: &Pubkey,
) -> ProgramResult {
    // Derive vault PDA
    let (expected, vault_bump) =
        pubkey::find_program_address(&[PM_VAULT_SEED, ps_mint, market_id_bytes], program_id);
    if !pubkey::pubkey_eq(vault.key(), &expected) {
        return Err(ProgramError::InvalidSeeds);
    }

    let rent = Rent::get()?;
    // Token-2022 account size = 165 (standard token account)
    let space: usize = 165;
    let lamports = rent.minimum_balance(space);

    let bump_ref = [vault_bump];
    let seeds = [
        Seed::from(PM_VAULT_SEED),
        Seed::from(ps_mint),
        Seed::from(market_id_bytes),
        Seed::from(bump_ref.as_ref()),
    ];
    let pda_signer = Signer::from(&seeds);

    // Create account owned by Token-2022
    {
        let mut create_data = [0u8; 52];
        create_data[4..12].copy_from_slice(&lamports.to_le_bytes());
        create_data[12..20].copy_from_slice(&(space as u64).to_le_bytes());
        create_data[20..52].copy_from_slice(token_program.key());
        let metas = [
            AccountMeta::writable_signer(payer.key()),
            AccountMeta::writable_signer(vault.key()),
        ];
        let ix = Instruction {
            program_id: &crate::SYSTEM_ID,
            accounts: &metas,
            data: &create_data,
        };
        pinocchio::cpi::slice_invoke_signed(&ix, &[payer, vault], &[pda_signer])?;
    }

    // InitializeAccount3 (discriminator = 18, no rent sysvar needed)
    // Data: [18, owner(32)]
    let mut init_data = [0u8; 33];
    init_data[0] = 18; // InitializeAccount3
    init_data[1..33].copy_from_slice(owner.key());

    let init_metas = [
        AccountMeta::writable(vault.key()),
        AccountMeta::readonly(mint.key()),
    ];
    let init_ix = Instruction {
        program_id: token_program.key(),
        accounts: &init_metas,
        data: &init_data,
    };
    pinocchio::cpi::slice_invoke_signed(&init_ix, &[vault, mint], &[])
}

/// Initialize a standard SPL mint (for YES/NO outcome tokens).
#[inline(never)]
fn init_spl_mint<'a>(
    payer: &'a AccountInfo,
    mint_account: &'a AccountInfo,
    mint_authority: &'a AccountInfo,
    token_program: &'a AccountInfo,
    seed_prefix: &[u8],
    ps_mint: &[u8],
    market_id_bytes: &[u8],
    program_id: &Pubkey,
) -> ProgramResult {
    let (expected, mint_bump) =
        pubkey::find_program_address(&[seed_prefix, ps_mint, market_id_bytes], program_id);
    if !pubkey::pubkey_eq(mint_account.key(), &expected) {
        return Err(ProgramError::InvalidSeeds);
    }

    let rent = Rent::get()?;
    let space: usize = 82; // SPL Mint size
    let lamports = rent.minimum_balance(space);

    let bump_ref = [mint_bump];
    let seeds = [
        Seed::from(seed_prefix),
        Seed::from(ps_mint),
        Seed::from(market_id_bytes),
        Seed::from(bump_ref.as_ref()),
    ];
    let pda_signer = Signer::from(&seeds);

    // Create account owned by SPL Token
    {
        let mut create_data = [0u8; 52];
        create_data[4..12].copy_from_slice(&lamports.to_le_bytes());
        create_data[12..20].copy_from_slice(&(space as u64).to_le_bytes());
        create_data[20..52].copy_from_slice(token_program.key());
        let metas = [
            AccountMeta::writable_signer(payer.key()),
            AccountMeta::writable_signer(mint_account.key()),
        ];
        let ix = Instruction {
            program_id: &crate::SYSTEM_ID,
            accounts: &metas,
            data: &create_data,
        };
        pinocchio::cpi::slice_invoke_signed(&ix, &[payer, mint_account], &[pda_signer])?;
    }

    // InitializeMint2 (discriminator = 20): [20, decimals, authority(32), freeze_option(1), freeze_auth(32)]
    let mut init_data = [0u8; 67];
    init_data[0] = 20; // InitializeMint2
    init_data[1] = CCM_DECIMALS;
    init_data[2..34].copy_from_slice(mint_authority.key());
    init_data[34] = 0; // No freeze authority

    let init_metas = [AccountMeta::writable(mint_account.key())];
    let init_ix = Instruction {
        program_id: token_program.key(),
        accounts: &init_metas,
        data: &init_data,
    };
    pinocchio::cpi::slice_invoke_signed(&init_ix, &[mint_account], &[])
}

// ============================================================================
// 3. INITIALIZE MARKET TOKENS V2 (Token-2022 with MintCloseAuthority)
// ============================================================================
//
// Accounts:
//   0. [SIGNER, WRITE] payer
//   1. []               protocol_state PDA
//   2. [WRITE]          market_state PDA
//   3. []               ccm_mint (Token-2022)
//   4. [WRITE]          vault (PDA token account, will be init)
//   5. [WRITE]          yes_mint (PDA, Token-2022 with MintCloseAuthority)
//   6. [WRITE]          no_mint (PDA, Token-2022 with MintCloseAuthority)
//   7. []               mint_authority (PDA, no data)
//   8. []               token_program (Token-2022, for ALL accounts)
//   9. []               system_program
//  10. []               rent sysvar

#[inline(never)]
pub fn initialize_market_tokens_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 11 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let payer = &accounts[0];
    let protocol_state = &accounts[1];
    let market_state = &accounts[2];
    let ccm_mint = &accounts[3];
    let vault = &accounts[4];
    let yes_mint = &accounts[5];
    let no_mint = &accounts[6];
    let mint_authority = &accounts[7];
    let token_program = &accounts[8];
    let _system_program = &accounts[9];
    let _rent = &accounts[10];

    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let (ps_mint, _) = validate_protocol_state(protocol_state, program_id)?;
    check_not_paused(protocol_state)?;

    // token_program MUST be Token-2022
    if !pubkey::pubkey_eq(token_program.key(), &TOKEN_2022_ID) {
        return Err(OracleError::InvalidTokenProgram.into());
    }

    // Verify ccm_mint
    if !pubkey::pubkey_eq(ccm_mint.key(), &ps_mint) {
        return Err(OracleError::InvalidMint.into());
    }

    // Read market_state
    if !market_state.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let market_id = {
        let data = unsafe { market_state.borrow_data_unchecked() };
        if data.len() < MS_LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        if data[..8] != DISC_MARKET_STATE {
            return Err(ProgramError::InvalidAccountData);
        }
        let auth = read_pubkey(&data, MS_AUTHORITY);
        if !pubkey::pubkey_eq(payer.key(), &auth) {
            return Err(OracleError::Unauthorized.into());
        }
        if data[MS_TOKENS_INIT] != 0 {
            return Err(OracleError::MarketTokensAlreadyInitialized.into());
        }
        read_u64(&data, MS_MARKET_ID)
    };

    let _ = verify_market_state_pda(market_state, &ps_mint, market_id, program_id)?;

    let market_id_bytes = market_id.to_le_bytes();
    let (expected_mint_auth, _) = derive_mint_authority(&ps_mint, market_id, program_id);
    if !pubkey::pubkey_eq(mint_authority.key(), &expected_mint_auth) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Init vault (Token-2022 token account)
    init_vault_ata(
        payer,
        vault,
        ccm_mint,
        mint_authority,
        token_program,
        &ps_mint,
        &market_id_bytes,
        program_id,
    )?;

    // Init YES mint (Token-2022 + MintCloseAuthority)
    init_t22_mint_with_close_auth(
        payer,
        yes_mint,
        mint_authority,
        token_program,
        MARKET_YES_MINT_SEED,
        &ps_mint,
        &market_id_bytes,
        program_id,
    )?;

    // Init NO mint (Token-2022 + MintCloseAuthority)
    init_t22_mint_with_close_auth(
        payer,
        no_mint,
        mint_authority,
        token_program,
        MARKET_NO_MINT_SEED,
        &ps_mint,
        &market_id_bytes,
        program_id,
    )?;

    // Update MarketState
    {
        let data = unsafe { market_state.borrow_mut_data_unchecked() };
        data[MS_VAULT..MS_VAULT + 32].copy_from_slice(vault.key());
        data[MS_YES_MINT..MS_YES_MINT + 32].copy_from_slice(yes_mint.key());
        data[MS_NO_MINT..MS_NO_MINT + 32].copy_from_slice(no_mint.key());
        data[MS_MINT_AUTHORITY..MS_MINT_AUTHORITY + 32].copy_from_slice(mint_authority.key());
        data[MS_TOKENS_INIT] = 1;
    }

    Ok(())
}

/// Initialize a Token-2022 mint with MintCloseAuthority extension.
#[inline(never)]
fn init_t22_mint_with_close_auth<'a>(
    payer: &'a AccountInfo,
    mint_account: &'a AccountInfo,
    mint_authority: &'a AccountInfo,
    _token_program: &'a AccountInfo,
    seed_prefix: &[u8],
    ps_mint: &[u8],
    market_id_bytes: &[u8],
    program_id: &Pubkey,
) -> ProgramResult {
    let (expected, mint_bump) =
        pubkey::find_program_address(&[seed_prefix, ps_mint, market_id_bytes], program_id);
    if !pubkey::pubkey_eq(mint_account.key(), &expected) {
        return Err(ProgramError::InvalidSeeds);
    }

    let rent = Rent::get()?;
    // Token-2022 Mint (82 base) + MintCloseAuthority extension:
    //   account_type(1) + padding(3) + ext_type(2) + ext_len(2) + close_auth(32) = 40
    // Total: 82 + 83 padding/multisig area + extension = ~166.
    // Actual: ExtensionType::try_calculate_account_len for MintCloseAuthority = 234
    // Use the known size for Token-2022 Mint + MintCloseAuthority extension.
    // Base mint = 82 bytes. Account type byte = 1. MintCloseAuthority = 4 (header) + 32 (pubkey) = 36.
    // Padded to multisig size: 82 pad to 165, then +1 account_type +36 = 202.
    // Exact: 165 (padded to multisig) + 1 (account_type) + 2 (ext_type) + 2 (ext_len) + 32 (close_auth) + 32 (optional padding) = varies.
    // We use the same size as the Anchor program which uses try_calculate_account_len.
    // For Token-2022: Mint(82) padded to MultisigLen(355? No...) Actually:
    // SPL Token-2022 base Mint = 82 bytes, padded to 165 (BASE_ACCOUNT_LENGTH), +1 (AccountType)
    // + MintCloseAuthority: type_u16(2) + length_u16(2) + pubkey(32) = 36
    // Total: 165 + 1 + 36 = 202
    let space: usize = 202;
    let lamports = rent.minimum_balance(space);

    let bump_ref = [mint_bump];
    let seeds = [
        Seed::from(seed_prefix),
        Seed::from(ps_mint),
        Seed::from(market_id_bytes),
        Seed::from(bump_ref.as_ref()),
    ];
    let pda_signer = Signer::from(&seeds);

    // 1. Create account owned by Token-2022
    {
        let mut create_data = [0u8; 52];
        create_data[4..12].copy_from_slice(&lamports.to_le_bytes());
        create_data[12..20].copy_from_slice(&(space as u64).to_le_bytes());
        create_data[20..52].copy_from_slice(&TOKEN_2022_ID);
        let metas = [
            AccountMeta::writable_signer(payer.key()),
            AccountMeta::writable_signer(mint_account.key()),
        ];
        let ix = Instruction {
            program_id: &crate::SYSTEM_ID,
            accounts: &metas,
            data: &create_data,
        };
        pinocchio::cpi::slice_invoke_signed(&ix, &[payer, mint_account], &[pda_signer])?;
    }

    // 2. InitializeMintCloseAuthority (Token-2022 instruction 25)
    // Data: [25, has_close_auth(1), close_auth(32)]
    let mut close_auth_data = [0u8; 34];
    close_auth_data[0] = 25; // InitializeMintCloseAuthority
    close_auth_data[1] = 1; // COption::Some
    close_auth_data[2..34].copy_from_slice(mint_authority.key());

    let close_auth_metas = [AccountMeta::writable(mint_account.key())];
    let close_auth_ix = Instruction {
        program_id: &TOKEN_2022_ID,
        accounts: &close_auth_metas,
        data: &close_auth_data,
    };
    pinocchio::cpi::slice_invoke_signed(&close_auth_ix, &[mint_account], &[])?;

    // 3. InitializeMint2 (discriminator = 20)
    let mut init_data = [0u8; 67];
    init_data[0] = 20; // InitializeMint2
    init_data[1] = CCM_DECIMALS;
    init_data[2..34].copy_from_slice(mint_authority.key());
    init_data[34] = 0; // No freeze authority

    let init_metas = [AccountMeta::writable(mint_account.key())];
    let init_ix = Instruction {
        program_id: &TOKEN_2022_ID,
        accounts: &init_metas,
        data: &init_data,
    };
    pinocchio::cpi::slice_invoke_signed(&init_ix, &[mint_account], &[])
}

// ============================================================================
// 4. MINT SHARES (deposit CCM -> get YES + NO)
// ============================================================================
//
// Accounts:
//   0. [SIGNER, WRITE] depositor
//   1. []               protocol_state PDA
//   2. []               market_state PDA
//   3. []               ccm_mint (Token-2022)
//   4. [WRITE]          depositor_ccm (Token-2022 ATA)
//   5. [WRITE]          vault (Token-2022 ATA)
//   6. [WRITE]          yes_mint
//   7. [WRITE]          no_mint
//   8. [WRITE]          depositor_yes
//   9. [WRITE]          depositor_no
//  10. []               mint_authority PDA
//  11. []               token_program (Token-2022, for CCM)
//  12. []               outcome_token_program (SPL or Token-2022, for YES/NO)
//  13..N []             remaining_accounts (transfer fee hooks)
//
// Instruction data:
//   [0..8]  amount (u64 LE)

#[inline(never)]
pub fn mint_shares(program_id: &Pubkey, accounts: &[AccountInfo], ix_data: &[u8]) -> ProgramResult {
    if accounts.len() < 13 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    if ix_data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let depositor = &accounts[0];
    let protocol_state = &accounts[1];
    let market_state = &accounts[2];
    let ccm_mint = &accounts[3];
    let depositor_ccm = &accounts[4];
    let vault = &accounts[5];
    let yes_mint = &accounts[6];
    let no_mint = &accounts[7];
    let depositor_yes = &accounts[8];
    let depositor_no = &accounts[9];
    let mint_authority = &accounts[10];
    let token_program = &accounts[11];
    let outcome_token_program = &accounts[12];
    let remaining = if accounts.len() > 13 {
        &accounts[13..]
    } else {
        &[]
    };

    if !depositor.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let amount = read_u64(ix_data, 0);
    if amount == 0 {
        return Err(OracleError::ZeroSharesMinted.into());
    }

    let (ps_mint, _) = validate_protocol_state(protocol_state, program_id)?;
    check_not_paused(protocol_state)?;

    // Read and validate market_state
    let (market_id, mint_auth_bump) = validate_market_for_trading(
        market_state,
        &ps_mint,
        program_id,
        vault,
        yes_mint,
        no_mint,
        mint_authority,
        ccm_mint,
    )?;

    // Snapshot vault balance BEFORE transfer
    let vault_before = {
        let data = unsafe { vault.borrow_data_unchecked() };
        read_token_amount(&data)
    };

    // Transfer CCM from depositor to vault (Token-2022 transfer_checked)
    {
        // Depositor signs directly (no PDA signer)
        let mut data = [0u8; 10];
        data[0] = 12; // TransferChecked
        data[1..9].copy_from_slice(&amount.to_le_bytes());
        data[9] = CCM_DECIMALS;

        let n_total = 4 + remaining.len();
        let mut metas_buf: [core::mem::MaybeUninit<AccountMeta>; 36] =
            unsafe { core::mem::MaybeUninit::uninit().assume_init() };
        metas_buf[0].write(AccountMeta::writable(depositor_ccm.key()));
        metas_buf[1].write(AccountMeta::readonly(ccm_mint.key()));
        metas_buf[2].write(AccountMeta::writable(vault.key()));
        metas_buf[3].write(AccountMeta::readonly_signer(depositor.key()));
        for (i, acc) in remaining.iter().enumerate() {
            // Preserve caller-provided writable/signer flags for transfer hooks.
            metas_buf[4 + i].write(AccountMeta::new(
                acc.key(),
                acc.is_writable(),
                acc.is_signer(),
            ));
        }
        let metas = unsafe {
            core::slice::from_raw_parts(metas_buf.as_ptr() as *const AccountMeta, n_total)
        };

        let mut refs_buf: [core::mem::MaybeUninit<&AccountInfo>; 36] =
            unsafe { core::mem::MaybeUninit::uninit().assume_init() };
        refs_buf[0].write(depositor_ccm);
        refs_buf[1].write(ccm_mint);
        refs_buf[2].write(vault);
        refs_buf[3].write(depositor);
        for (i, acc) in remaining.iter().enumerate() {
            refs_buf[4 + i].write(acc);
        }
        let refs = unsafe {
            core::slice::from_raw_parts(refs_buf.as_ptr() as *const &AccountInfo, n_total)
        };

        let ix = Instruction {
            program_id: token_program.key(),
            accounts: metas,
            data: &data,
        };
        pinocchio::cpi::slice_invoke_signed(&ix, refs, &[])?;
    }

    // Read vault balance AFTER transfer to get net_received
    let vault_after = {
        let data = unsafe { vault.borrow_data_unchecked() };
        read_token_amount(&data)
    };
    let net_received = vault_after
        .checked_sub(vault_before)
        .ok_or(ProgramError::Custom(6041))?; // MathOverflow
    if net_received == 0 {
        return Err(OracleError::ZeroSharesMinted.into());
    }

    // Mint YES and NO shares (1:1 with net_received)
    let market_id_bytes = market_id.to_le_bytes();
    let bump_ref = [mint_auth_bump];
    let seeds = mint_auth_seeds(
        MARKET_MINT_AUTHORITY_SEED,
        &ps_mint,
        &market_id_bytes,
        &bump_ref,
    );

    mint_to_cpi(
        outcome_token_program,
        yes_mint,
        depositor_yes,
        mint_authority,
        net_received,
        &seeds,
    )?;
    mint_to_cpi(
        outcome_token_program,
        no_mint,
        depositor_no,
        mint_authority,
        net_received,
        &seeds,
    )?;

    Ok(())
}

/// Validate market_state for trading (mint_shares, redeem_shares, settle).
/// Returns (market_id, mint_authority_bump).
#[inline(never)]
fn validate_market_for_trading(
    market_state: &AccountInfo,
    ps_mint: &Pubkey,
    program_id: &Pubkey,
    vault: &AccountInfo,
    yes_mint: &AccountInfo,
    no_mint: &AccountInfo,
    mint_authority: &AccountInfo,
    ccm_mint: &AccountInfo,
) -> Result<(u64, u8), ProgramError> {
    if !market_state.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let data = unsafe { market_state.borrow_data_unchecked() };
    if data.len() < MS_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    if data[..8] != DISC_MARKET_STATE {
        return Err(ProgramError::InvalidAccountData);
    }

    if data[MS_TOKENS_INIT] == 0 {
        return Err(OracleError::MarketTokensNotInitialized.into());
    }

    let market_id = read_u64(&data, MS_MARKET_ID);
    let ms_mint = read_pubkey(&data, MS_MINT);
    if !pubkey::pubkey_eq(&ms_mint, ps_mint) {
        return Err(OracleError::InvalidMint.into());
    }

    // Verify vault, yes_mint, no_mint, mint_authority match stored values
    let ms_vault = read_pubkey(&data, MS_VAULT);
    if !pubkey::pubkey_eq(vault.key(), &ms_vault) {
        return Err(OracleError::InvalidMarketState.into());
    }
    let ms_yes = read_pubkey(&data, MS_YES_MINT);
    if !pubkey::pubkey_eq(yes_mint.key(), &ms_yes) {
        return Err(OracleError::InvalidMarketState.into());
    }
    let ms_no = read_pubkey(&data, MS_NO_MINT);
    if !pubkey::pubkey_eq(no_mint.key(), &ms_no) {
        return Err(OracleError::InvalidMarketState.into());
    }
    let ms_auth = read_pubkey(&data, MS_MINT_AUTHORITY);
    if !pubkey::pubkey_eq(mint_authority.key(), &ms_auth) {
        return Err(OracleError::InvalidMarketState.into());
    }

    // Verify ccm_mint
    if !pubkey::pubkey_eq(ccm_mint.key(), ps_mint) {
        return Err(OracleError::InvalidMint.into());
    }

    let _ = verify_market_state_pda(market_state, ps_mint, market_id, program_id)?;

    let (_, mint_auth_bump) = derive_mint_authority(ps_mint, market_id, program_id);

    Ok((market_id, mint_auth_bump))
}

// ============================================================================
// 5. REDEEM SHARES (burn equal YES + NO -> get CCM back, pre-resolution)
// ============================================================================
//
// Accounts:
//   0. [SIGNER, WRITE] redeemer
//   1. []               protocol_state PDA
//   2. []               market_state PDA
//   3. []               ccm_mint (Token-2022)
//   4. [WRITE]          vault
//   5. [WRITE]          yes_mint
//   6. [WRITE]          no_mint
//   7. [WRITE]          redeemer_yes
//   8. [WRITE]          redeemer_no
//   9. [WRITE]          redeemer_ccm
//  10. []               mint_authority PDA
//  11. []               token_program (Token-2022, for CCM)
//  12. []               outcome_token_program (for YES/NO)
//  13..N []             remaining_accounts
//
// Instruction data:
//   [0..8]  shares (u64 LE)

#[inline(never)]
pub fn redeem_shares(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 13 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    if ix_data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let redeemer = &accounts[0];
    let protocol_state = &accounts[1];
    let market_state = &accounts[2];
    let ccm_mint = &accounts[3];
    let vault = &accounts[4];
    let yes_mint = &accounts[5];
    let no_mint = &accounts[6];
    let redeemer_yes = &accounts[7];
    let redeemer_no = &accounts[8];
    let redeemer_ccm = &accounts[9];
    let mint_authority = &accounts[10];
    let token_program = &accounts[11];
    let outcome_token_program = &accounts[12];
    let remaining = if accounts.len() > 13 {
        &accounts[13..]
    } else {
        &[]
    };

    if !redeemer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let shares = read_u64(ix_data, 0);
    if shares == 0 {
        return Err(OracleError::ZeroSharesMinted.into());
    }

    let (ps_mint, _) = validate_protocol_state(protocol_state, program_id)?;
    check_not_paused(protocol_state)?;

    // Market must not be resolved
    {
        let data = unsafe { market_state.borrow_data_unchecked() };
        if data[MS_RESOLVED] != 0 {
            return Err(OracleError::MarketAlreadyResolved.into());
        }
    }

    let (market_id, mint_auth_bump) = validate_market_for_trading(
        market_state,
        &ps_mint,
        program_id,
        vault,
        yes_mint,
        no_mint,
        mint_authority,
        ccm_mint,
    )?;

    // Burn YES shares
    burn_cpi(
        outcome_token_program,
        yes_mint,
        redeemer_yes,
        redeemer,
        shares,
    )?;

    // Burn NO shares
    burn_cpi(
        outcome_token_program,
        no_mint,
        redeemer_no,
        redeemer,
        shares,
    )?;

    // Transfer CCM from vault to redeemer
    let market_id_bytes = market_id.to_le_bytes();
    let bump_ref = [mint_auth_bump];
    let seeds = mint_auth_seeds(
        MARKET_MINT_AUTHORITY_SEED,
        &ps_mint,
        &market_id_bytes,
        &bump_ref,
    );
    transfer_checked_signed(
        token_program,
        vault,
        ccm_mint,
        redeemer_ccm,
        mint_authority,
        shares,
        CCM_DECIMALS,
        &seeds,
        remaining,
    )?;

    Ok(())
}

// ============================================================================
// 6. RESOLVE MARKET (set outcome via merkle proof)
// ============================================================================
//
// Accounts:
//   0. [SIGNER]  resolver
//   1. []        protocol_state PDA
//   2. []        global_root_config PDA
//   3. [WRITE]   market_state PDA
//
// Instruction data:
//   [0..8]      cumulative_total (u64 LE)
//   [8..10]     proof_len (u16 LE)
//   [10..10+proof_len*32]  proof entries (each 32 bytes)

#[inline(never)]
pub fn resolve_market(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 4 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    if ix_data.len() < 10 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let _resolver = &accounts[0];
    let protocol_state = &accounts[1];
    let global_root_config = &accounts[2];
    let market_state = &accounts[3];

    if !_resolver.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let (ps_mint, _) = validate_protocol_state(protocol_state, program_id)?;
    check_not_paused(protocol_state)?;

    // Parse instruction data
    let cumulative_total = read_u64(ix_data, 0);
    let proof_len = u16::from_le_bytes([ix_data[8], ix_data[9]]) as usize;
    if proof_len > MAX_PROOF_LEN {
        return Err(OracleError::InvalidProofLength.into());
    }
    let proof_data_len = proof_len * 32;
    if ix_data.len() < 10 + proof_data_len {
        return Err(ProgramError::InvalidInstructionData);
    }

    // Parse proof
    let mut proof = [[0u8; 32]; 32]; // MAX_PROOF_LEN
    for i in 0..proof_len {
        let off = 10 + i * 32;
        proof[i].copy_from_slice(&ix_data[off..off + 32]);
    }
    let proof_slice = &proof[..proof_len];

    // Validate global root config
    validate_global_root_config(global_root_config, &ps_mint, program_id)?;

    // Read and validate market_state
    if !market_state.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }

    let (
        market_id,
        resolution_root_seq,
        _ms_bump,
        _ms_mint,
        _ms_metric,
        ms_creator_wallet,
        ms_target,
    ) = {
        let data = unsafe { market_state.borrow_data_unchecked() };
        if data.len() < MS_LEN || data[..8] != DISC_MARKET_STATE {
            return Err(ProgramError::InvalidAccountData);
        }
        if data[MS_VERSION] != MARKET_STATE_VERSION {
            return Err(OracleError::InvalidMarketState.into());
        }
        if data[MS_RESOLVED] != 0 {
            return Err(OracleError::MarketAlreadyResolved.into());
        }
        let ms_mint_val = read_pubkey(&data, MS_MINT);
        if !pubkey::pubkey_eq(&ms_mint_val, &ps_mint) {
            return Err(OracleError::InvalidMint.into());
        }
        let ms_metric_val = data[MS_METRIC];
        if ms_metric_val != MARKET_METRIC_ATTENTION_SCORE {
            return Err(OracleError::UnsupportedMarketMetric.into());
        }
        (
            read_u64(&data, MS_MARKET_ID),
            read_u64(&data, MS_RES_ROOT_SEQ),
            data[MS_BUMP],
            ms_mint_val,
            ms_metric_val,
            read_pubkey(&data, MS_CREATOR_WALLET),
            read_u64(&data, MS_TARGET),
        )
    };

    let _ = verify_market_state_pda(market_state, &ps_mint, market_id, program_id)?;

    // Check that resolution_root_seq is available in global root config
    let grc_data = unsafe { global_root_config.borrow_data_unchecked() };
    let grc_latest_seq = read_u64(&grc_data, GRC_LATEST_ROOT_SEQ);
    if resolution_root_seq > grc_latest_seq {
        return Err(OracleError::MarketNotResolvableYet.into());
    }

    // Look up root entry
    let idx = (resolution_root_seq as usize) % CUMULATIVE_ROOT_HISTORY;
    let entry_off = GRC_ROOTS_START + idx * ROOT_ENTRY_SIZE;
    let entry_seq = read_u64(&grc_data, entry_off);
    if entry_seq != resolution_root_seq {
        return Err(OracleError::RootTooOldOrMissing.into());
    }
    let mut root = [0u8; 32];
    root.copy_from_slice(&grc_data[entry_off + 8..entry_off + 40]);
    let _ = grc_data;

    // Compute leaf and verify proof
    let leaf = compute_global_leaf_v4(
        &ps_mint,
        resolution_root_seq,
        &ms_creator_wallet,
        cumulative_total,
    );
    if !verify_proof(proof_slice, leaf, root) {
        return Err(OracleError::InvalidProof.into());
    }

    // Set outcome
    let outcome = cumulative_total >= ms_target;
    let slot = Clock::get()?.slot;

    {
        let data = unsafe { market_state.borrow_mut_data_unchecked() };
        data[MS_RESOLVED] = 1;
        data[MS_OUTCOME] = if outcome { 1 } else { 0 };
        data[MS_RES_CUM_TOTAL..MS_RES_CUM_TOTAL + 8]
            .copy_from_slice(&cumulative_total.to_le_bytes());
        data[MS_RESOLVED_SLOT..MS_RESOLVED_SLOT + 8].copy_from_slice(&slot.to_le_bytes());
    }

    Ok(())
}

// ============================================================================
// 7. SETTLE (burn winning shares -> claim CCM, post-resolution)
// ============================================================================
//
// Accounts:
//   0. [SIGNER, WRITE] settler
//   1. []               protocol_state PDA
//   2. []               market_state PDA
//   3. []               ccm_mint (Token-2022)
//   4. [WRITE]          vault
//   5. [WRITE]          winning_mint (YES or NO depending on outcome)
//   6. [WRITE]          settler_winning (settler's winning token account)
//   7. [WRITE]          settler_ccm (settler's CCM token account)
//   8. []               mint_authority PDA
//   9. []               token_program (Token-2022)
//  10. []               outcome_token_program (for YES/NO)
//  11..N []             remaining_accounts
//
// Instruction data:
//   [0..8]  shares (u64 LE)

#[inline(never)]
pub fn settle(program_id: &Pubkey, accounts: &[AccountInfo], ix_data: &[u8]) -> ProgramResult {
    if accounts.len() < 11 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    if ix_data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let settler = &accounts[0];
    let protocol_state = &accounts[1];
    let market_state = &accounts[2];
    let ccm_mint = &accounts[3];
    let vault = &accounts[4];
    let winning_mint = &accounts[5];
    let settler_winning = &accounts[6];
    let settler_ccm = &accounts[7];
    let mint_authority = &accounts[8];
    let token_program = &accounts[9];
    let outcome_token_program = &accounts[10];
    let remaining = if accounts.len() > 11 {
        &accounts[11..]
    } else {
        &[]
    };

    if !settler.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let shares = read_u64(ix_data, 0);
    if shares == 0 {
        return Err(OracleError::ZeroSharesMinted.into());
    }

    let (ps_mint, _) = validate_protocol_state(protocol_state, program_id)?;
    check_not_paused(protocol_state)?;

    // Read market_state
    if !market_state.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let (market_id, mint_auth_bump, expected_winning) = {
        let data = unsafe { market_state.borrow_data_unchecked() };
        if data.len() < MS_LEN || data[..8] != DISC_MARKET_STATE {
            return Err(ProgramError::InvalidAccountData);
        }
        if data[MS_TOKENS_INIT] == 0 {
            return Err(OracleError::MarketTokensNotInitialized.into());
        }
        if data[MS_RESOLVED] == 0 {
            return Err(OracleError::MarketNotResolved.into());
        }
        let outcome = data[MS_OUTCOME] != 0;
        let expected = if outcome {
            read_pubkey(&data, MS_YES_MINT)
        } else {
            read_pubkey(&data, MS_NO_MINT)
        };
        let ms_mint = read_pubkey(&data, MS_MINT);
        if !pubkey::pubkey_eq(&ms_mint, &ps_mint) {
            return Err(OracleError::InvalidMint.into());
        }
        let ms_vault = read_pubkey(&data, MS_VAULT);
        if !pubkey::pubkey_eq(vault.key(), &ms_vault) {
            return Err(OracleError::InvalidMarketState.into());
        }
        let ms_auth = read_pubkey(&data, MS_MINT_AUTHORITY);
        if !pubkey::pubkey_eq(mint_authority.key(), &ms_auth) {
            return Err(OracleError::InvalidMarketState.into());
        }
        let mid = read_u64(&data, MS_MARKET_ID);
        let (_, mab) = derive_mint_authority(&ps_mint, mid, program_id);
        (mid, mab, expected)
    };

    let _ = verify_market_state_pda(market_state, &ps_mint, market_id, program_id)?;

    // Verify winning_mint matches expected
    if !pubkey::pubkey_eq(winning_mint.key(), &expected_winning) {
        return Err(OracleError::WrongOutcomeToken.into());
    }

    // Verify ccm_mint
    if !pubkey::pubkey_eq(ccm_mint.key(), &ps_mint) {
        return Err(OracleError::InvalidMint.into());
    }

    // Verify vault has enough CCM
    {
        let data = unsafe { vault.borrow_data_unchecked() };
        let vault_amount = read_token_amount(&data);
        if vault_amount < shares {
            return Err(OracleError::InsufficientVaultBalance.into());
        }
    }

    // Burn winning shares
    burn_cpi(
        outcome_token_program,
        winning_mint,
        settler_winning,
        settler,
        shares,
    )?;

    // Transfer CCM from vault to settler
    let market_id_bytes = market_id.to_le_bytes();
    let bump_ref = [mint_auth_bump];
    let seeds = mint_auth_seeds(
        MARKET_MINT_AUTHORITY_SEED,
        &ps_mint,
        &market_id_bytes,
        &bump_ref,
    );
    transfer_checked_signed(
        token_program,
        vault,
        ccm_mint,
        settler_ccm,
        mint_authority,
        shares,
        CCM_DECIMALS,
        &seeds,
        remaining,
    )?;

    Ok(())
}

// ============================================================================
// 8. SWEEP RESIDUAL (admin recovers leftover CCM after all winners settle)
// ============================================================================
//
// Accounts:
//   0. [SIGNER]  admin
//   1. []        protocol_state PDA
//   2. []        market_state PDA
//   3. []        ccm_mint (Token-2022)
//   4. [WRITE]   vault
//   5. []        winning_mint (supply must be 0)
//   6. [WRITE]   treasury_ccm
//   7. []        mint_authority PDA
//   8. []        token_program (Token-2022)
//   9..N []      remaining_accounts

#[inline(never)]
pub fn sweep_residual(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 9 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let admin = &accounts[0];
    let protocol_state = &accounts[1];
    let market_state = &accounts[2];
    let ccm_mint = &accounts[3];
    let vault = &accounts[4];
    let winning_mint = &accounts[5];
    let treasury_ccm = &accounts[6];
    let mint_authority = &accounts[7];
    let token_program = &accounts[8];
    let remaining = if accounts.len() > 9 {
        &accounts[9..]
    } else {
        &[]
    };

    if !admin.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let (ps_mint, _) = validate_protocol_state(protocol_state, program_id)?;
    check_admin(admin, protocol_state)?;

    // Read market_state
    if !market_state.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let (market_id, mint_auth_bump, expected_winning) = {
        let data = unsafe { market_state.borrow_data_unchecked() };
        if data.len() < MS_LEN || data[..8] != DISC_MARKET_STATE {
            return Err(ProgramError::InvalidAccountData);
        }
        if data[MS_TOKENS_INIT] == 0 {
            return Err(OracleError::MarketTokensNotInitialized.into());
        }
        if data[MS_RESOLVED] == 0 {
            return Err(OracleError::MarketNotResolved.into());
        }
        let outcome = data[MS_OUTCOME] != 0;
        let expected = if outcome {
            read_pubkey(&data, MS_YES_MINT)
        } else {
            read_pubkey(&data, MS_NO_MINT)
        };
        let ms_vault = read_pubkey(&data, MS_VAULT);
        if !pubkey::pubkey_eq(vault.key(), &ms_vault) {
            return Err(OracleError::InvalidMarketState.into());
        }
        let ms_auth = read_pubkey(&data, MS_MINT_AUTHORITY);
        if !pubkey::pubkey_eq(mint_authority.key(), &ms_auth) {
            return Err(OracleError::InvalidMarketState.into());
        }
        let mid = read_u64(&data, MS_MARKET_ID);
        let (_, mab) = derive_mint_authority(&ps_mint, mid, program_id);
        (mid, mab, expected)
    };

    let _ = verify_market_state_pda(market_state, &ps_mint, market_id, program_id)?;

    // Verify winning_mint
    if !pubkey::pubkey_eq(winning_mint.key(), &expected_winning) {
        return Err(OracleError::WrongOutcomeToken.into());
    }

    // Winning mint supply must be 0
    {
        let data = unsafe { winning_mint.borrow_data_unchecked() };
        let supply = read_mint_supply(&data);
        if supply != 0 {
            return Err(OracleError::WinningSharesStillOutstanding.into());
        }
    }

    // Get residual amount from vault
    let residual = {
        let data = unsafe { vault.borrow_data_unchecked() };
        read_token_amount(&data)
    };
    if residual == 0 {
        return Err(OracleError::InsufficientVaultBalance.into());
    }

    if !pubkey::pubkey_eq(ccm_mint.key(), &ps_mint) {
        return Err(OracleError::InvalidMint.into());
    }

    // Transfer all remaining CCM to treasury
    let market_id_bytes = market_id.to_le_bytes();
    let bump_ref = [mint_auth_bump];
    let seeds = mint_auth_seeds(
        MARKET_MINT_AUTHORITY_SEED,
        &ps_mint,
        &market_id_bytes,
        &bump_ref,
    );
    transfer_checked_signed(
        token_program,
        vault,
        ccm_mint,
        treasury_ccm,
        mint_authority,
        residual,
        CCM_DECIMALS,
        &seeds,
        remaining,
    )?;

    Ok(())
}

// ============================================================================
// 9. CLOSE MARKET (reclaim rent from MarketState + vault, post-resolution)
// ============================================================================
//
// Accounts:
//   0. [SIGNER, WRITE] admin
//   1. []               protocol_state PDA
//   2. [WRITE]          market_state PDA (will be closed)
//   3. [WRITE]          vault (must be empty, will be closed)
//   4. []               ccm_mint
//   5. []               yes_mint (supply must be 0)
//   6. []               no_mint (supply must be 0)
//   7. []               mint_authority PDA
//   8. []               token_program (Token-2022)

#[inline(never)]
pub fn close_market(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 9 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let admin = &accounts[0];
    let protocol_state = &accounts[1];
    let market_state = &accounts[2];
    let vault = &accounts[3];
    let ccm_mint = &accounts[4];
    let yes_mint = &accounts[5];
    let no_mint = &accounts[6];
    let mint_authority = &accounts[7];
    let token_program = &accounts[8];

    if !admin.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let (ps_mint, _) = validate_protocol_state(protocol_state, program_id)?;

    // Check admin or market authority
    {
        let ms_data = unsafe { market_state.borrow_data_unchecked() };
        if ms_data.len() < MS_LEN || ms_data[..8] != DISC_MARKET_STATE {
            return Err(ProgramError::InvalidAccountData);
        }
        let ms_authority = read_pubkey(&ms_data, MS_AUTHORITY);
        let ps_data = unsafe { protocol_state.borrow_data_unchecked() };
        let ps_admin = read_pubkey(&ps_data, PS_ADMIN);
        if !pubkey::pubkey_eq(admin.key(), &ps_admin)
            && !pubkey::pubkey_eq(admin.key(), &ms_authority)
        {
            return Err(OracleError::Unauthorized.into());
        }
    }

    // Validate market_state
    if !market_state.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let (market_id, mint_auth_bump) = {
        let data = unsafe { market_state.borrow_data_unchecked() };
        if data[MS_RESOLVED] == 0 {
            return Err(OracleError::MarketNotResolved.into());
        }
        if data[MS_TOKENS_INIT] == 0 {
            return Err(OracleError::MarketTokensNotInitialized.into());
        }
        let ms_vault = read_pubkey(&data, MS_VAULT);
        if !pubkey::pubkey_eq(vault.key(), &ms_vault) {
            return Err(OracleError::InvalidMarketState.into());
        }
        let ms_yes = read_pubkey(&data, MS_YES_MINT);
        if !pubkey::pubkey_eq(yes_mint.key(), &ms_yes) {
            return Err(OracleError::InvalidMarketState.into());
        }
        let ms_no = read_pubkey(&data, MS_NO_MINT);
        if !pubkey::pubkey_eq(no_mint.key(), &ms_no) {
            return Err(OracleError::InvalidMarketState.into());
        }
        let ms_auth = read_pubkey(&data, MS_MINT_AUTHORITY);
        if !pubkey::pubkey_eq(mint_authority.key(), &ms_auth) {
            return Err(OracleError::InvalidMarketState.into());
        }

        if !pubkey::pubkey_eq(ccm_mint.key(), &ps_mint) {
            return Err(OracleError::InvalidMint.into());
        }

        let mid = read_u64(&data, MS_MARKET_ID);
        let (_, mab) = derive_mint_authority(&ps_mint, mid, program_id);
        (mid, mab)
    };

    let _ = verify_market_state_pda(market_state, &ps_mint, market_id, program_id)?;

    // Vault must be empty
    {
        let data = unsafe { vault.borrow_data_unchecked() };
        let vault_amount = read_token_amount(&data);
        if vault_amount != 0 {
            return Err(OracleError::VaultNotEmpty.into());
        }
    }

    // YES and NO mint supply must be 0
    {
        let data = unsafe { yes_mint.borrow_data_unchecked() };
        if read_mint_supply(&data) != 0 {
            return Err(OracleError::WinningSharesStillOutstanding.into());
        }
    }
    {
        let data = unsafe { no_mint.borrow_data_unchecked() };
        if read_mint_supply(&data) != 0 {
            return Err(OracleError::WinningSharesStillOutstanding.into());
        }
    }

    // Close vault ATA via Token-2022 CloseAccount CPI
    let market_id_bytes = market_id.to_le_bytes();
    let bump_ref = [mint_auth_bump];
    let seeds = mint_auth_seeds(
        MARKET_MINT_AUTHORITY_SEED,
        &ps_mint,
        &market_id_bytes,
        &bump_ref,
    );
    close_account_cpi(token_program, vault, admin, mint_authority, &seeds)?;

    // Close MarketState PDA (transfer lamports to admin, zero data)
    {
        let lamports = market_state.lamports();
        // Transfer lamports from market_state to admin
        unsafe {
            *market_state.borrow_mut_lamports_unchecked() = 0;
            *admin.borrow_mut_lamports_unchecked() = admin
                .lamports()
                .checked_add(lamports)
                .ok_or(ProgramError::ArithmeticOverflow)?;
        }
        // Zero account data
        let data = unsafe { market_state.borrow_mut_data_unchecked() };
        for byte in data.iter_mut() {
            *byte = 0;
        }
    }

    Ok(())
}

// ============================================================================
// 10. CLOSE MARKET MINTS (reclaim rent from zero-supply YES/NO mints)
// ============================================================================
//
// Accounts:
//   0. [SIGNER, WRITE] admin
//   1. []               protocol_state PDA
//   2. [WRITE]          yes_mint (supply must be 0)
//   3. [WRITE]          no_mint (supply must be 0)
//   4. []               mint_authority PDA
//   5. []               outcome_token_program (SPL or Token-2022)
//
// Instruction data:
//   [0..8]  market_id (u64 LE)

#[inline(never)]
pub fn close_market_mints(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 6 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    if ix_data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let admin = &accounts[0];
    let protocol_state = &accounts[1];
    let yes_mint = &accounts[2];
    let no_mint = &accounts[3];
    let mint_authority = &accounts[4];
    let outcome_token_program = &accounts[5];

    if !admin.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let (ps_mint, _) = validate_protocol_state(protocol_state, program_id)?;
    check_admin(admin, protocol_state)?;

    let market_id = read_u64(ix_data, 0);
    let market_id_bytes = market_id.to_le_bytes();

    // Verify YES mint PDA
    let (expected_yes, _yes_bump) = pubkey::find_program_address(
        &[MARKET_YES_MINT_SEED, &ps_mint, &market_id_bytes],
        program_id,
    );
    if !pubkey::pubkey_eq(yes_mint.key(), &expected_yes) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Verify NO mint PDA
    let (expected_no, _no_bump) = pubkey::find_program_address(
        &[MARKET_NO_MINT_SEED, &ps_mint, &market_id_bytes],
        program_id,
    );
    if !pubkey::pubkey_eq(no_mint.key(), &expected_no) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Verify mint_authority PDA
    let (expected_auth, mint_auth_bump) = derive_mint_authority(&ps_mint, market_id, program_id);
    if !pubkey::pubkey_eq(mint_authority.key(), &expected_auth) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Both mints must have supply == 0
    {
        let data = unsafe { yes_mint.borrow_data_unchecked() };
        if read_mint_supply(&data) != 0 {
            return Err(OracleError::WinningSharesStillOutstanding.into());
        }
    }
    {
        let data = unsafe { no_mint.borrow_data_unchecked() };
        if read_mint_supply(&data) != 0 {
            return Err(OracleError::WinningSharesStillOutstanding.into());
        }
    }

    // Close YES mint
    let bump_ref = [mint_auth_bump];
    let seeds = mint_auth_seeds(
        MARKET_MINT_AUTHORITY_SEED,
        &ps_mint,
        &market_id_bytes,
        &bump_ref,
    );
    close_account_cpi(
        outcome_token_program,
        yes_mint,
        admin,
        mint_authority,
        &seeds,
    )?;

    // Close NO mint
    close_account_cpi(
        outcome_token_program,
        no_mint,
        admin,
        mint_authority,
        &seeds,
    )?;

    Ok(())
}
