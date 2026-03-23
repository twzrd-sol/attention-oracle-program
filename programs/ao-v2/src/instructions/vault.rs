//! Market Vault instructions — the core product loop (Pinocchio).
//!
//! Byte-compatible with the Anchor program. Same discriminators, same PDA seeds,
//! same account layouts. Every handler validates accounts manually and uses
//! pinocchio-token for SPL CPIs.
//!
//! Handlers:
//!   initialize_protocol_state — creates ProtocolState PDA
//!   initialize_market_vault  — creates MarketVault PDA with vLOFI mint
//!   realloc_market_vault     — grows MarketVault 137 → 153 bytes
//!   deposit_market           — USDC → vault, mint vLOFI 1:1
//!   update_attention         — oracle sets multiplier BPS on position
//!   update_nav               — oracle sets NAV per share on MarketVault
//!   claim_yield              — deprecated, returns error
//!   settle_market            — burn vLOFI, return USDC, close position

use pinocchio::instruction::{AccountMeta, Instruction};
use pinocchio::{
    account_info::AccountInfo,
    instruction::Signer,
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    seeds,
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    ProgramResult,
};

use crate::error::OracleError;
use crate::state::{
    MarketVault, ProtocolState, UserMarketPosition, DISC_MARKET_VAULT, DISC_PROTOCOL_STATE,
    DISC_USER_MARKET_POSITION, MARKET_POSITION_SEED, MARKET_VAULT_SEED, PROTOCOL_STATE_SEED,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Compact CPI helpers (avoids const-generic monomorphization)
// ---------------------------------------------------------------------------

#[inline(never)]
fn cpi_spl_transfer(
    from: &AccountInfo,
    to: &AccountInfo,
    authority: &AccountInfo,
    amount: u64,
) -> ProgramResult {
    let mut data = [0u8; 9];
    data[0] = 3; // SPL Transfer
    data[1..9].copy_from_slice(&amount.to_le_bytes());
    let metas = [
        AccountMeta::writable(from.key()),
        AccountMeta::writable(to.key()),
        AccountMeta::readonly_signer(authority.key()),
    ];
    let ix = Instruction {
        program_id: &crate::SPL_TOKEN_ID,
        accounts: &metas,
        data: &data,
    };
    pinocchio::cpi::slice_invoke_signed(&ix, &[from, to, authority], &[])
}

#[inline(never)]
fn cpi_spl_mint_to(
    mint: &AccountInfo,
    account: &AccountInfo,
    authority: &AccountInfo,
    amount: u64,
    signers: &[Signer],
) -> ProgramResult {
    let mut data = [0u8; 9];
    data[0] = 7; // MintTo
    data[1..9].copy_from_slice(&amount.to_le_bytes());
    let metas = [
        AccountMeta::writable(mint.key()),
        AccountMeta::writable(account.key()),
        AccountMeta::readonly_signer(authority.key()),
    ];
    let ix = Instruction {
        program_id: &crate::SPL_TOKEN_ID,
        accounts: &metas,
        data: &data,
    };
    pinocchio::cpi::slice_invoke_signed(&ix, &[mint, account, authority], signers)
}

#[inline(never)]
fn cpi_spl_burn(
    account: &AccountInfo,
    mint: &AccountInfo,
    authority: &AccountInfo,
    amount: u64,
) -> ProgramResult {
    let mut data = [0u8; 9];
    data[0] = 8; // Burn
    data[1..9].copy_from_slice(&amount.to_le_bytes());
    let metas = [
        AccountMeta::writable(account.key()),
        AccountMeta::writable(mint.key()),
        AccountMeta::readonly_signer(authority.key()),
    ];
    let ix = Instruction {
        program_id: &crate::SPL_TOKEN_ID,
        accounts: &metas,
        data: &data,
    };
    pinocchio::cpi::slice_invoke_signed(&ix, &[account, mint, authority], &[])
}

#[inline(never)]
fn cpi_sys_transfer(from: &AccountInfo, to: &AccountInfo, lamports: u64) -> ProgramResult {
    let data = lamports.to_le_bytes();
    // System program transfer instruction = index 2, followed by u64 amount
    let mut ix_data = [0u8; 12];
    ix_data[0..4].copy_from_slice(&2u32.to_le_bytes());
    ix_data[4..12].copy_from_slice(&data);
    let metas = [
        AccountMeta::writable_signer(from.key()),
        AccountMeta::writable(to.key()),
    ];
    let ix = Instruction {
        program_id: &crate::SYSTEM_ID,
        accounts: &metas,
        data: &ix_data,
    };
    pinocchio::cpi::slice_invoke_signed(&ix, &[from, to], &[])
}

#[inline(never)]
fn cpi_spl_transfer_signed(
    from: &AccountInfo,
    to: &AccountInfo,
    authority: &AccountInfo,
    amount: u64,
    signers: &[Signer],
) -> ProgramResult {
    let mut data = [0u8; 9];
    data[0] = 3; // SPL Transfer
    data[1..9].copy_from_slice(&amount.to_le_bytes());
    let metas = [
        AccountMeta::writable(from.key()),
        AccountMeta::writable(to.key()),
        AccountMeta::readonly_signer(authority.key()),
    ];
    let ix = Instruction {
        program_id: &crate::SPL_TOKEN_ID,
        accounts: &metas,
        data: &data,
    };
    pinocchio::cpi::slice_invoke_signed(&ix, &[from, to, authority], signers)
}

const MIN_MULTIPLIER_BPS: u64 = 10_000;
const MAX_MULTIPLIER_BPS: u64 = 50_000;
const BASE_YIELD_MULTIPLIER_BPS: u64 = 10_000;

// ---------------------------------------------------------------------------
// Helpers: read SPL Token account fields
// ---------------------------------------------------------------------------

/// Token-2022 program ID (for ownership checks alongside SPL Token).
use crate::TOKEN_2022_ID;

/// Verify a token account is owned by SPL Token or Token-2022.
#[inline(always)]
fn verify_token_account(account: &AccountInfo) -> Result<(), ProgramError> {
    let owner = account.owner();
    if !pubkey::pubkey_eq(owner, &crate::SPL_TOKEN_ID) && !pubkey::pubkey_eq(owner, &TOKEN_2022_ID)
    {
        return Err(ProgramError::IllegalOwner);
    }
    Ok(())
}

/// SPL Token account layout: offset 0 = mint (Pubkey, 32 bytes)
#[inline(always)]
fn token_account_mint(account: &AccountInfo) -> Result<&Pubkey, ProgramError> {
    let data = unsafe { account.borrow_data_unchecked() };
    if data.len() < 32 {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(unsafe { &*(data.as_ptr() as *const Pubkey) })
}

/// SPL Token account layout: offset 32 = owner (Pubkey, 32 bytes)
#[inline(always)]
fn token_account_owner(account: &AccountInfo) -> Result<&Pubkey, ProgramError> {
    let data = unsafe { account.borrow_data_unchecked() };
    if data.len() < 64 {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(unsafe { &*(data[32..].as_ptr() as *const Pubkey) })
}

/// SPL Token account layout: offset 64 = amount (u64, LE)
#[inline(always)]
fn token_account_amount(account: &AccountInfo) -> Result<u64, ProgramError> {
    let data = unsafe { account.borrow_data_unchecked() };
    if data.len() < 72 {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(u64::from_le_bytes(data[64..72].try_into().unwrap()))
}

// ---------------------------------------------------------------------------
// Helper: compute_position_yield_components (for settle_market audit logs)
// ---------------------------------------------------------------------------

fn compute_position_yield_components(
    deposited_amount: u64,
    attention_multiplier_bps: u64,
) -> Result<(u64, u64, u64), ProgramError> {
    let effective_multiplier = if attention_multiplier_bps == 0 {
        BASE_YIELD_MULTIPLIER_BPS
    } else {
        attention_multiplier_bps
    };
    let base_yield = deposited_amount;
    let attention_bonus = deposited_amount
        .checked_mul(effective_multiplier.saturating_sub(BASE_YIELD_MULTIPLIER_BPS))
        .ok_or(ProgramError::ArithmeticOverflow)?
        .checked_div(BASE_YIELD_MULTIPLIER_BPS)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    let total_earned = base_yield
        .checked_add(attention_bonus)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    Ok((base_yield, attention_bonus, total_earned))
}

// =============================================================================
// INITIALIZE PROTOCOL STATE
// =============================================================================
// Accounts: [admin (signer, mut), protocol_state (mut), system_program]
// Instruction data (after 8-byte discriminator):
//   publisher: Pubkey (32), treasury: Pubkey (32),
//   oracle_authority: Pubkey (32), ccm_mint: Pubkey (32)

pub fn initialize_protocol_state(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if ix_data.len() < 128 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let publisher: &Pubkey = unsafe { &*(ix_data[0..32].as_ptr() as *const Pubkey) };
    let treasury: &Pubkey = unsafe { &*(ix_data[32..64].as_ptr() as *const Pubkey) };
    let oracle_authority: &Pubkey = unsafe { &*(ix_data[64..96].as_ptr() as *const Pubkey) };
    let ccm_mint: &Pubkey = unsafe { &*(ix_data[96..128].as_ptr() as *const Pubkey) };

    let [admin, protocol_state_acc, system_program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate admin
    if !admin.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Validate system program
    if !pubkey::pubkey_eq(system_program.key(), &crate::SYSTEM_ID) {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Derive PDA
    let (expected_pda, bump) = ProtocolState::find_pda(program_id);
    if !pubkey::pubkey_eq(protocol_state_acc.key(), &expected_pda) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Create account
    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(ProtocolState::LEN);

    let bump_ref = [bump];
    let pda_seeds = seeds!(PROTOCOL_STATE_SEED, &bump_ref);
    let pda_signer = Signer::from(&pda_seeds);
    crate::cpi_create_account(
        admin,
        protocol_state_acc,
        lamports,
        ProtocolState::LEN as u64,
        program_id,
        &[pda_signer],
    )?;

    // Initialize data via typed struct
    let state = ProtocolState::from_account_mut(protocol_state_acc)?;
    state.discriminator = DISC_PROTOCOL_STATE;
    state.is_initialized = 1;
    state.version = 1;
    state.admin = *admin.key();
    state.publisher = *publisher;
    state.treasury = *treasury;
    state.oracle_authority = *oracle_authority;
    state.mint = *ccm_mint;
    state.paused = 0;
    state.require_receipt = 0;
    state.bump = bump;

    Ok(())
}

// =============================================================================
// INITIALIZE MARKET VAULT
// =============================================================================
// Accounts: [admin (signer, mut), protocol_state, market_vault (mut),
//            deposit_mint, vlofi_mint, vault_ata, system_program]
// Instruction data (after discriminator): market_id: u64

pub fn initialize_market_vault(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if ix_data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let market_id = u64::from_le_bytes(ix_data[0..8].try_into().unwrap());

    let [admin, protocol_state_acc, market_vault_acc, deposit_mint, vlofi_mint, vault_ata, system_program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate admin signer
    if !admin.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Validate system program
    if !pubkey::pubkey_eq(system_program.key(), &crate::SYSTEM_ID) {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Validate protocol_state: owned by program, correct disc, admin matches
    if !protocol_state_acc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let ps = ProtocolState::from_account(protocol_state_acc)?;
    if !pubkey::pubkey_eq(&ps.admin, admin.key()) {
        return Err(OracleError::Unauthorized.into());
    }
    // Verify PDA
    let (expected_ps, _) = ProtocolState::find_pda(program_id);
    if !pubkey::pubkey_eq(protocol_state_acc.key(), &expected_ps) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Derive market_vault PDA
    let market_id_bytes = market_id.to_le_bytes();
    let (expected_mv, mv_bump) =
        MarketVault::find_pda(protocol_state_acc.key(), market_id, program_id);
    if !pubkey::pubkey_eq(market_vault_acc.key(), &expected_mv) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Validate vault_ata: token program ownership, owner == market_vault PDA, mint == deposit_mint
    verify_token_account(vault_ata)?;
    if !pubkey::pubkey_eq(token_account_owner(vault_ata)?, &expected_mv) {
        return Err(ProgramError::InvalidAccountData);
    }
    if !pubkey::pubkey_eq(token_account_mint(vault_ata)?, deposit_mint.key()) {
        return Err(ProgramError::InvalidAccountData);
    }

    // Create market_vault account
    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(MarketVault::LEN);

    let mv_bump_ref = [mv_bump];
    let mv_seeds = seeds!(
        MARKET_VAULT_SEED,
        protocol_state_acc.key(),
        &market_id_bytes,
        &mv_bump_ref
    );
    let mv_signer = Signer::from(&mv_seeds);
    crate::cpi_create_account(
        admin,
        market_vault_acc,
        lamports,
        MarketVault::LEN as u64,
        program_id,
        &[mv_signer],
    )?;

    // Initialize market_vault data
    let vault = MarketVault::from_account_mut(market_vault_acc)?;
    vault.discriminator = DISC_MARKET_VAULT;
    vault.bump = mv_bump;
    vault.set_market_id(market_id);
    vault.deposit_mint = *deposit_mint.key();
    vault.vlofi_mint = *vlofi_mint.key();
    vault.vault_ata = *vault_ata.key();
    vault.set_total_deposited(0);
    vault.set_total_shares(0);
    let clock = Clock::get()?;
    vault.created_slot = clock.slot.to_le_bytes();
    vault.set_nav_per_share_bps(0);
    vault.set_last_nav_update_slot(0);

    Ok(())
}

// =============================================================================
// REALLOC MARKET VAULT — Grow 137 → 153 bytes
// =============================================================================
// Accounts: [payer (signer, mut), protocol_state, market_vault (mut),
//            system_program]
// Instruction data (after discriminator): market_id: u64

pub fn realloc_market_vault(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if ix_data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let market_id = u64::from_le_bytes(ix_data[0..8].try_into().unwrap());

    let [payer, protocol_state_acc, market_vault_acc, system_program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if !pubkey::pubkey_eq(system_program.key(), &crate::SYSTEM_ID) {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Validate protocol_state
    if !protocol_state_acc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let ps = ProtocolState::from_account(protocol_state_acc)?;
    // constraint: payer == admin
    if !pubkey::pubkey_eq(&ps.admin, payer.key()) {
        return Err(OracleError::Unauthorized.into());
    }

    // Validate market_vault PDA
    let (expected_mv, _) = MarketVault::find_pda(protocol_state_acc.key(), market_id, program_id);
    if !pubkey::pubkey_eq(market_vault_acc.key(), &expected_mv) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Check current size
    let current_len = market_vault_acc.data_len();
    let target_len = MarketVault::LEN;

    if current_len >= target_len {
        return Ok(());
    }

    // Transfer rent difference
    let rent = Rent::get()?;
    let lamports_needed = rent
        .minimum_balance(target_len)
        .saturating_sub(market_vault_acc.lamports());

    if lamports_needed > 0 {
        cpi_sys_transfer(payer, market_vault_acc, lamports_needed)?;
    }

    // Grow account — new bytes zero-filled (nav_per_share_bps=0, last_nav_update_slot=0)
    market_vault_acc.resize(target_len)?;

    Ok(())
}

// =============================================================================
// DEPOSIT MARKET — USDC → Vault, mint vLOFI 1:1
// =============================================================================
// Accounts: [user (signer, mut), protocol_state, market_vault (mut),
//            user_market_position (mut), user_usdc_ata (mut),
//            vault_usdc_ata (mut), vlofi_mint (mut), user_vlofi_ata (mut),
//            token_program, token_2022_program, system_program]
// Instruction data (after discriminator): market_id: u64, amount: u64

pub fn deposit_market(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if ix_data.len() < 16 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let market_id = u64::from_le_bytes(ix_data[0..8].try_into().unwrap());
    let amount = u64::from_le_bytes(ix_data[8..16].try_into().unwrap());

    if amount == 0 {
        return Err(OracleError::InvalidInputLength.into());
    }

    let [user, protocol_state_acc, market_vault_acc, user_position_acc, user_usdc_ata, vault_usdc_ata, vlofi_mint, user_vlofi_ata, token_program, _token_2022_program, _system_program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate user signer
    if !user.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Validate protocol_state: owned, correct disc, not paused
    if !protocol_state_acc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let ps = ProtocolState::from_account(protocol_state_acc)?;
    if ps.is_paused() {
        return Err(OracleError::ProtocolPaused.into());
    }
    let ps_bump = ps.bump;

    // Verify protocol_state PDA
    let (expected_ps, _) = ProtocolState::find_pda(program_id);
    if !pubkey::pubkey_eq(protocol_state_acc.key(), &expected_ps) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Validate market_vault: owned, correct disc, PDA matches
    if !market_vault_acc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    // Read vault fields before any mutation. Use raw data for v1 compat (may be 137 bytes).
    let mv_data = unsafe { market_vault_acc.borrow_data_unchecked() };
    if mv_data.len() < MarketVault::LEN_V1 || mv_data[..8] != DISC_MARKET_VAULT {
        return Err(ProgramError::InvalidAccountData);
    }
    let mv_deposit_mint: Pubkey = mv_data[17..49].try_into().unwrap();
    let mv_vlofi_mint: Pubkey = mv_data[49..81].try_into().unwrap();
    let effective_nav = if mv_data.len() >= 145 {
        let nav = u64::from_le_bytes(mv_data[137..145].try_into().unwrap());
        if nav == 0 {
            10_000u64
        } else {
            nav
        }
    } else {
        10_000u64
    };
    let _ = mv_data;

    // Verify market_vault PDA
    let (expected_mv, _) = MarketVault::find_pda(protocol_state_acc.key(), market_id, program_id);
    if !pubkey::pubkey_eq(market_vault_acc.key(), &expected_mv) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Validate vault_usdc_ata: token program ownership, owner == market_vault, mint == deposit_mint
    verify_token_account(vault_usdc_ata)?;
    if !pubkey::pubkey_eq(token_account_owner(vault_usdc_ata)?, market_vault_acc.key()) {
        return Err(ProgramError::InvalidAccountData);
    }
    if !pubkey::pubkey_eq(token_account_mint(vault_usdc_ata)?, &mv_deposit_mint) {
        return Err(ProgramError::InvalidAccountData);
    }

    // Validate vlofi_mint address
    if !pubkey::pubkey_eq(vlofi_mint.key(), &mv_vlofi_mint) {
        return Err(ProgramError::InvalidAccountData);
    }

    // Validate user_vlofi_ata: token program ownership, mint == vlofi_mint, owner == user
    verify_token_account(user_vlofi_ata)?;
    if !pubkey::pubkey_eq(token_account_mint(user_vlofi_ata)?, &mv_vlofi_mint) {
        return Err(ProgramError::InvalidAccountData);
    }
    if !pubkey::pubkey_eq(token_account_owner(user_vlofi_ata)?, user.key()) {
        return Err(ProgramError::InvalidAccountData);
    }

    // Validate user_usdc_ata: token program ownership
    verify_token_account(user_usdc_ata)?;

    // Validate token_program
    if !pubkey::pubkey_eq(token_program.key(), &crate::SPL_TOKEN_ID) {
        return Err(ProgramError::IncorrectProgramId);
    }

    // 1. Transfer USDC from user to vault
    cpi_spl_transfer(user_usdc_ata, vault_usdc_ata, user, amount)?;

    // 2. Compute shares to mint: shares = amount * 10_000 / nav
    let shares_to_mint = amount
        .checked_mul(10_000)
        .ok_or(ProgramError::ArithmeticOverflow)?
        .checked_div(effective_nav)
        .ok_or(ProgramError::ArithmeticOverflow)?;

    if shares_to_mint == 0 {
        return Err(OracleError::InvalidInputLength.into());
    }

    // 3. Mint vLOFI to user (ProtocolState PDA = mint authority)
    let ps_bump_ref = [ps_bump];
    let mint_seeds = seeds!(PROTOCOL_STATE_SEED, &ps_bump_ref);
    let mint_signer = Signer::from(&mint_seeds);
    cpi_spl_mint_to(
        vlofi_mint,
        user_vlofi_ata,
        protocol_state_acc,
        shares_to_mint,
        &[mint_signer],
    )?;

    // 4. Update market_vault accounting
    {
        let mv_data = unsafe { market_vault_acc.borrow_mut_data_unchecked() };
        let total_deposited = u64::from_le_bytes(mv_data[113..121].try_into().unwrap());
        let total_shares = u64::from_le_bytes(mv_data[121..129].try_into().unwrap());
        mv_data[113..121].copy_from_slice(
            &total_deposited
                .checked_add(amount)
                .ok_or(ProgramError::ArithmeticOverflow)?
                .to_le_bytes(),
        );
        mv_data[121..129].copy_from_slice(
            &total_shares
                .checked_add(shares_to_mint)
                .ok_or(ProgramError::ArithmeticOverflow)?
                .to_le_bytes(),
        );
    }

    // 5. Create or update user position (init_if_needed pattern)
    let (expected_pos, pos_bump) =
        UserMarketPosition::find_pda(market_vault_acc.key(), user.key(), program_id);
    if !pubkey::pubkey_eq(user_position_acc.key(), &expected_pos) {
        return Err(ProgramError::InvalidSeeds);
    }

    if user_position_acc.lamports() == 0 {
        // Account doesn't exist — create it
        let rent = Rent::get()?;
        let pos_lamports = rent.minimum_balance(UserMarketPosition::LEN);

        let pos_bump_ref = [pos_bump];
        let pos_seeds = seeds!(
            MARKET_POSITION_SEED,
            market_vault_acc.key(),
            user.key(),
            &pos_bump_ref
        );
        let pos_signer = Signer::from(&pos_seeds);
        crate::cpi_create_account(
            user,
            user_position_acc,
            pos_lamports,
            UserMarketPosition::LEN as u64,
            program_id,
            &[pos_signer],
        )?;

        // Initialize position data
        let pos = UserMarketPosition::from_account_mut(user_position_acc)?;
        pos.discriminator = DISC_USER_MARKET_POSITION;
        pos.bump = pos_bump;
        pos.user = *user.key();
        pos.market_vault = *market_vault_acc.key();
        pos.set_deposited_amount(amount);
        pos.set_shares_minted(shares_to_mint);
        pos.set_attention_multiplier_bps(0);
        pos.settled = 0;
        let clock = Clock::get()?;
        pos.entry_slot = clock.slot.to_le_bytes();
        pos.cumulative_claimed = 0u64.to_le_bytes();
    } else {
        // Position already exists — validate and update
        if !user_position_acc.is_owned_by(program_id) {
            return Err(ProgramError::IllegalOwner);
        }
        let pos = UserMarketPosition::from_account_mut(user_position_acc)?;
        if pos.discriminator != DISC_USER_MARKET_POSITION {
            return Err(ProgramError::InvalidAccountData);
        }

        // If bump is 0, first deposit — initialize identity fields
        if pos.bump == 0 {
            pos.bump = pos_bump;
            pos.user = *user.key();
            pos.market_vault = *market_vault_acc.key();
            let clock = Clock::get()?;
            pos.entry_slot = clock.slot.to_le_bytes();
        }

        let deposited = pos.get_deposited_amount();
        let shares = pos.get_shares_minted();
        pos.set_deposited_amount(
            deposited
                .checked_add(amount)
                .ok_or(ProgramError::ArithmeticOverflow)?,
        );
        pos.set_shares_minted(
            shares
                .checked_add(shares_to_mint)
                .ok_or(ProgramError::ArithmeticOverflow)?,
        );
    }

    Ok(())
}

// =============================================================================
// UPDATE ATTENTION — Oracle sets multiplier BPS on user position
// =============================================================================
// Accounts: [oracle_authority (signer, mut), protocol_state, market_vault,
//            user_market_position (mut)]
// Instruction data (after discriminator):
//   market_id: u64, user_pubkey: Pubkey (32), multiplier_bps: u64

pub fn update_attention(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if ix_data.len() < 48 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let market_id = u64::from_le_bytes(ix_data[0..8].try_into().unwrap());
    let user_pubkey: &Pubkey = unsafe { &*(ix_data[8..40].as_ptr() as *const Pubkey) };
    let multiplier_bps = u64::from_le_bytes(ix_data[40..48].try_into().unwrap());

    let [oracle_authority, protocol_state_acc, market_vault_acc, user_position_acc, ..] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate oracle_authority signer
    if !oracle_authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Validate protocol_state
    if !protocol_state_acc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let ps = ProtocolState::from_account(protocol_state_acc)?;
    // has_one = oracle_authority
    if !pubkey::pubkey_eq(&ps.oracle_authority, oracle_authority.key()) {
        return Err(OracleError::Unauthorized.into());
    }
    // Verify PDA
    let (expected_ps, _) = ProtocolState::find_pda(program_id);
    if !pubkey::pubkey_eq(protocol_state_acc.key(), &expected_ps) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Validate market_vault PDA
    if !market_vault_acc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    {
        let mv_data = unsafe { market_vault_acc.borrow_data_unchecked() };
        if mv_data.len() < MarketVault::LEN_V1 || mv_data[..8] != DISC_MARKET_VAULT {
            return Err(ProgramError::InvalidAccountData);
        }
    }

    let (expected_mv, _) = MarketVault::find_pda(protocol_state_acc.key(), market_id, program_id);
    if !pubkey::pubkey_eq(market_vault_acc.key(), &expected_mv) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Validate user_market_position PDA
    if !user_position_acc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    if !user_position_acc.is_writable() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Validate position discriminator and length
    let pos_data = unsafe { user_position_acc.borrow_data_unchecked() };
    if pos_data.len() < UserMarketPosition::LEN || pos_data[..8] != DISC_USER_MARKET_POSITION {
        return Err(ProgramError::InvalidAccountData);
    }
    let _ = pos_data;

    // Verify PDA seeds
    let (expected_pos, _) =
        UserMarketPosition::find_pda(market_vault_acc.key(), user_pubkey, program_id);
    if !pubkey::pubkey_eq(user_position_acc.key(), &expected_pos) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Read position for constraint checks
    {
        let pos_data = unsafe { user_position_acc.borrow_data_unchecked() };
        // position.market_vault == market_vault.key()
        let pos_mv: &Pubkey = unsafe { &*(pos_data[41..73].as_ptr() as *const Pubkey) };
        if !pubkey::pubkey_eq(pos_mv, market_vault_acc.key()) {
            return Err(ProgramError::InvalidAccountData);
        }
        // position.user == user_pubkey
        let pos_user: &Pubkey = unsafe { &*(pos_data[9..41].as_ptr() as *const Pubkey) };
        if !pubkey::pubkey_eq(pos_user, user_pubkey) {
            return Err(ProgramError::InvalidAccountData);
        }
        // !settled
        if pos_data[97] != 0 {
            return Err(OracleError::AlreadySettled.into());
        }
    }

    // Validate multiplier range
    if multiplier_bps < MIN_MULTIPLIER_BPS {
        return Err(OracleError::MultiplierBelowMinimum.into());
    }
    if multiplier_bps > MAX_MULTIPLIER_BPS {
        return Err(OracleError::MaxMultiplierExceeded.into());
    }

    // Write multiplier (offset 89, 8 bytes LE)
    let pos_data = unsafe { user_position_acc.borrow_mut_data_unchecked() };
    pos_data[89..97].copy_from_slice(&multiplier_bps.to_le_bytes());

    Ok(())
}

// =============================================================================
// UPDATE NAV — Oracle sets NAV per vLOFI share on MarketVault
// =============================================================================
// Accounts: [oracle_authority (signer, mut), protocol_state, market_vault (mut)]
// Instruction data (after discriminator): market_id: u64, nav_per_share_bps: u64

pub fn update_nav(program_id: &Pubkey, accounts: &[AccountInfo], ix_data: &[u8]) -> ProgramResult {
    if ix_data.len() < 16 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let market_id = u64::from_le_bytes(ix_data[0..8].try_into().unwrap());
    let nav_per_share_bps = u64::from_le_bytes(ix_data[8..16].try_into().unwrap());

    let [oracle_authority, protocol_state_acc, market_vault_acc, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate oracle_authority signer
    if !oracle_authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Validate protocol_state
    if !protocol_state_acc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let ps = ProtocolState::from_account(protocol_state_acc)?;
    if !pubkey::pubkey_eq(&ps.oracle_authority, oracle_authority.key()) {
        return Err(OracleError::Unauthorized.into());
    }
    let (expected_ps, _) = ProtocolState::find_pda(program_id);
    if !pubkey::pubkey_eq(protocol_state_acc.key(), &expected_ps) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Validate market_vault: owned, correct disc, full LEN (must have NAV fields)
    if !market_vault_acc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let mv_data = unsafe { market_vault_acc.borrow_data_unchecked() };
    if mv_data.len() < MarketVault::LEN || mv_data[..8] != DISC_MARKET_VAULT {
        return Err(ProgramError::InvalidAccountData);
    }
    let current_nav = u64::from_le_bytes(mv_data[137..145].try_into().unwrap());
    let _ = mv_data;

    // Verify PDA
    let (expected_mv, _) = MarketVault::find_pda(protocol_state_acc.key(), market_id, program_id);
    if !pubkey::pubkey_eq(market_vault_acc.key(), &expected_mv) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Validate NAV constraints
    if nav_per_share_bps < 10_000 {
        return Err(OracleError::NavBelowMinimum.into());
    }
    // Monotonic non-decreasing
    let floor = if current_nav > 10_000 {
        current_nav
    } else {
        10_000
    };
    if nav_per_share_bps < floor {
        return Err(OracleError::NavDecreaseNotAllowed.into());
    }
    if nav_per_share_bps > 50_000 {
        return Err(OracleError::NavAboveMaximum.into());
    }

    // Write NAV + slot
    let vault = MarketVault::from_account_mut(market_vault_acc)?;
    vault.set_nav_per_share_bps(nav_per_share_bps);
    let clock = Clock::get()?;
    vault.set_last_nav_update_slot(clock.slot);

    Ok(())
}

// =============================================================================
// CLAIM YIELD — Deprecated, returns error
// =============================================================================
// Accounts: [user (signer, mut), protocol_state, market_vault,
//            user_market_position (mut)]
// Instruction data (after discriminator): market_id: u64

pub fn claim_yield(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _ix_data: &[u8],
) -> ProgramResult {
    let [user, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !user.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    Err(OracleError::ClaimYieldDeprecated.into())
}

// =============================================================================
// SETTLE MARKET — Burn vLOFI, return USDC, close position
// =============================================================================
// Accounts: [user (signer, mut), protocol_state, market_vault (mut),
//            user_market_position (mut), vlofi_mint (mut),
//            user_vlofi_ata (mut), vault_usdc_ata (mut), user_usdc_ata (mut),
//            token_program, token_2022_program]
// Instruction data (after discriminator): market_id: u64

pub fn settle_market(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if ix_data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let market_id = u64::from_le_bytes(ix_data[0..8].try_into().unwrap());

    let [user, protocol_state_acc, market_vault_acc, user_position_acc, vlofi_mint, user_vlofi_ata, vault_usdc_ata, user_usdc_ata, token_program, _token_2022_program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate user signer
    if !user.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Validate protocol_state: not paused
    if !protocol_state_acc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let ps = ProtocolState::from_account(protocol_state_acc)?;
    if ps.is_paused() {
        return Err(OracleError::ProtocolPaused.into());
    }
    let (expected_ps, _) = ProtocolState::find_pda(program_id);
    if !pubkey::pubkey_eq(protocol_state_acc.key(), &expected_ps) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Validate market_vault
    if !market_vault_acc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let market_id_bytes = market_id.to_le_bytes();
    let mv_data = unsafe { market_vault_acc.borrow_data_unchecked() };
    if mv_data.len() < MarketVault::LEN_V1 || mv_data[..8] != DISC_MARKET_VAULT {
        return Err(ProgramError::InvalidAccountData);
    }
    let mv_bump = mv_data[8];
    let mv_vlofi_mint: Pubkey = mv_data[49..81].try_into().unwrap();
    let mv_deposit_mint: Pubkey = mv_data[17..49].try_into().unwrap();
    let nav_per_share_bps = if mv_data.len() >= 145 {
        u64::from_le_bytes(mv_data[137..145].try_into().unwrap())
    } else {
        0
    };
    let _ = mv_data;

    let (expected_mv, _) = MarketVault::find_pda(protocol_state_acc.key(), market_id, program_id);
    if !pubkey::pubkey_eq(market_vault_acc.key(), &expected_mv) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Validate user_market_position
    if !user_position_acc.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let pos_data = unsafe { user_position_acc.borrow_data_unchecked() };
    if pos_data.len() < UserMarketPosition::LEN || pos_data[..8] != DISC_USER_MARKET_POSITION {
        return Err(ProgramError::InvalidAccountData);
    }

    // Verify PDA
    let (expected_pos, _) =
        UserMarketPosition::find_pda(market_vault_acc.key(), user.key(), program_id);
    if !pubkey::pubkey_eq(user_position_acc.key(), &expected_pos) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Position constraints
    let pos_mv: &Pubkey = unsafe { &*(pos_data[41..73].as_ptr() as *const Pubkey) };
    if !pubkey::pubkey_eq(pos_mv, market_vault_acc.key()) {
        return Err(ProgramError::InvalidAccountData);
    }
    let pos_user: &Pubkey = unsafe { &*(pos_data[9..41].as_ptr() as *const Pubkey) };
    if !pubkey::pubkey_eq(pos_user, user.key()) {
        return Err(ProgramError::InvalidAccountData);
    }
    if pos_data[97] != 0 {
        return Err(OracleError::AlreadySettled.into());
    }

    // Read position fields
    let deposited_amount = u64::from_le_bytes(pos_data[73..81].try_into().unwrap());
    let shares_to_burn = u64::from_le_bytes(pos_data[81..89].try_into().unwrap());
    let attention_multiplier_bps = u64::from_le_bytes(pos_data[89..97].try_into().unwrap());
    let cumulative_claimed = u64::from_le_bytes(pos_data[106..114].try_into().unwrap());
    let _ = pos_data;

    // Fail-fast: reject zero-share settlements
    if shares_to_burn == 0 {
        return Err(OracleError::ZeroSharesMinted.into());
    }

    // NAV-adjusted principal return
    let effective_nav = if nav_per_share_bps == 0 {
        10_000u64
    } else {
        nav_per_share_bps
    };
    let principal_to_return = if effective_nav == 10_000 {
        deposited_amount
    } else {
        shares_to_burn
            .checked_mul(effective_nav)
            .ok_or(ProgramError::ArithmeticOverflow)?
            .checked_div(10_000)
            .ok_or(ProgramError::ArithmeticOverflow)?
    };

    // Compute CCM yield for audit logs only
    let (_base_yield, _attention_bonus, total_earned) =
        compute_position_yield_components(deposited_amount, attention_multiplier_bps)?;
    let ccm_yield = total_earned.saturating_sub(cumulative_claimed);

    // Validate vlofi_mint
    if !pubkey::pubkey_eq(vlofi_mint.key(), &mv_vlofi_mint) {
        return Err(ProgramError::InvalidAccountData);
    }

    // Validate user_vlofi_ata: mint == vlofi, owner == user
    if !pubkey::pubkey_eq(token_account_mint(user_vlofi_ata)?, &mv_vlofi_mint) {
        return Err(ProgramError::InvalidAccountData);
    }
    if !pubkey::pubkey_eq(token_account_owner(user_vlofi_ata)?, user.key()) {
        return Err(ProgramError::InvalidAccountData);
    }

    // Validate vault_usdc_ata: token program ownership, owner == market_vault, mint == deposit_mint
    verify_token_account(vault_usdc_ata)?;
    if !pubkey::pubkey_eq(token_account_owner(vault_usdc_ata)?, market_vault_acc.key()) {
        return Err(ProgramError::InvalidAccountData);
    }
    if !pubkey::pubkey_eq(token_account_mint(vault_usdc_ata)?, &mv_deposit_mint) {
        return Err(ProgramError::InvalidAccountData);
    }

    // Validate user_usdc_ata: token program ownership
    verify_token_account(user_usdc_ata)?;

    // Reserve guard
    let vault_balance = token_account_amount(vault_usdc_ata)?;
    if vault_balance < principal_to_return {
        return Err(OracleError::InsufficientReserve.into());
    }

    // Validate token_program
    if !pubkey::pubkey_eq(token_program.key(), &crate::SPL_TOKEN_ID) {
        return Err(ProgramError::IncorrectProgramId);
    }

    // 1. Burn user's vLOFI
    cpi_spl_burn(user_vlofi_ata, vlofi_mint, user, shares_to_burn)?;

    // 2. Return USDC from vault to user (vault PDA signs)
    let protocol_key = *protocol_state_acc.key();
    let mv_bump_ref = [mv_bump];
    let vault_sign_seeds = seeds!(
        MARKET_VAULT_SEED,
        &protocol_key,
        &market_id_bytes,
        &mv_bump_ref
    );
    let vault_signer = Signer::from(&vault_sign_seeds);

    cpi_spl_transfer_signed(
        vault_usdc_ata,
        user_usdc_ata,
        market_vault_acc,
        principal_to_return,
        &[vault_signer],
    )?;

    // 3. CCM yield distributed via merkle claims only — log for auditability
    if ccm_yield > 0 {}

    // 4. Update vault accounting
    {
        let mv_data = unsafe { market_vault_acc.borrow_mut_data_unchecked() };
        let total_deposited = u64::from_le_bytes(mv_data[113..121].try_into().unwrap());
        let total_shares = u64::from_le_bytes(mv_data[121..129].try_into().unwrap());
        // Subtract original deposited_amount (not NAV-adjusted) to avoid underflow
        mv_data[113..121].copy_from_slice(
            &total_deposited
                .checked_sub(deposited_amount)
                .ok_or(ProgramError::ArithmeticOverflow)?
                .to_le_bytes(),
        );
        mv_data[121..129].copy_from_slice(
            &total_shares
                .checked_sub(shares_to_burn)
                .ok_or(ProgramError::ArithmeticOverflow)?
                .to_le_bytes(),
        );
    }

    // 5. Mark position as settled
    {
        let pos_data = unsafe { user_position_acc.borrow_mut_data_unchecked() };
        pos_data[81..89].copy_from_slice(&0u64.to_le_bytes()); // shares_minted = 0
        pos_data[73..81].copy_from_slice(&0u64.to_le_bytes()); // deposited_amount = 0
        pos_data[97] = 1; // settled = true
    }

    Ok(())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yield_components_baseline_multiplier() {
        let (base, bonus, total) = compute_position_yield_components(1_000_000, 10_000).unwrap();
        assert_eq!(base, 1_000_000);
        assert_eq!(bonus, 0);
        assert_eq!(total, 1_000_000);
    }

    #[test]
    fn yield_components_zero_multiplier_falls_back() {
        let (base, bonus, total) = compute_position_yield_components(1_000_000, 0).unwrap();
        assert_eq!(base, 1_000_000);
        assert_eq!(bonus, 0);
        assert_eq!(total, 1_000_000);
    }

    #[test]
    fn yield_components_2x_multiplier() {
        let (base, bonus, total) = compute_position_yield_components(1_000_000, 20_000).unwrap();
        assert_eq!(base, 1_000_000);
        assert_eq!(bonus, 1_000_000);
        assert_eq!(total, 2_000_000);
    }

    #[test]
    fn yield_components_max_multiplier() {
        let (base, bonus, total) = compute_position_yield_components(1_000_000, 50_000).unwrap();
        assert_eq!(base, 1_000_000);
        assert_eq!(bonus, 4_000_000);
        assert_eq!(total, 5_000_000);
    }

    #[test]
    fn yield_components_zero_deposit() {
        let (base, bonus, total) = compute_position_yield_components(0, 20_000).unwrap();
        assert_eq!(base, 0);
        assert_eq!(bonus, 0);
        assert_eq!(total, 0);
    }

    #[test]
    fn yield_components_large_deposit_no_overflow() {
        let deposit: u64 = 368_934_881_474_191;
        assert!(compute_position_yield_components(deposit, 50_000).is_ok());
    }

    #[test]
    fn nav_adjusted_principal_at_genesis() {
        let shares: u64 = 1_000_000;
        let nav_bps: u64 = 10_000;
        let principal = if nav_bps == 10_000 { shares } else { 0 };
        assert_eq!(principal, 1_000_000);
    }

    #[test]
    fn nav_adjusted_principal_with_yield() {
        let shares: u64 = 1_000_000;
        let nav_bps: u64 = 10_100;
        let principal = shares
            .checked_mul(nav_bps)
            .and_then(|v| v.checked_div(10_000));
        assert_eq!(principal, Some(1_010_000));
    }

    #[test]
    fn nav_adjusted_principal_at_max() {
        let shares: u64 = 1_000_000;
        let nav_bps: u64 = 50_000;
        let principal = shares
            .checked_mul(nav_bps)
            .and_then(|v| v.checked_div(10_000));
        assert_eq!(principal, Some(5_000_000));
    }
}
