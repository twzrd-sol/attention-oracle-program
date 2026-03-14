//! Price feed instructions — Switchboard bridge (permissionless cranker).
//!
//! Pinocchio port of `programs/attention-oracle/src/instructions/price_feed.rs`.
//! Wire-compatible: same PDA seeds, same account layout, same Anchor discriminators.
//!
//! Handlers:
//!   - `initialize_price_feed` — admin creates a PriceFeedState PDA
//!   - `update_price`          — registered updater pushes a new price
//!   - `set_price_updater`     — authority rotates the updater key

use pinocchio::{
    account_info::AccountInfo,
    instruction::Signer,
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    ProgramResult,
};

use crate::error::OracleError;
use crate::state::{
    PriceFeedState, ProtocolState,
    DISC_PRICE_FEED_STATE,
    PRICE_FEED_SEED, PROTOCOL_STATE_SEED,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum price deviation between updates: 20% (2000 BPS).
const MAX_DEVIATION_BPS: u64 = 2_000;

// ---------------------------------------------------------------------------
// INITIALIZE PRICE FEED
// ---------------------------------------------------------------------------

/// Admin creates a new PriceFeedState PDA.
///
/// Accounts:
///   0. `[signer, writable]` admin
///   1. `[]`                 protocol_state PDA
///   2. `[writable]`         price_feed PDA (uninitialized, will be created)
///   3. `[]`                 system_program
///
/// Instruction data (Anchor serialization order):
///   0..32   label ([u8; 32])
///   32..64  updater (Pubkey)
///   64..72  max_staleness_slots (u64 LE)
pub fn initialize_price_feed(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 4 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let admin = &accounts[0];
    let protocol_state = &accounts[1];
    let price_feed = &accounts[2];
    let _system_program = &accounts[3];

    // Parse instruction data: 32 + 32 + 8 = 72 bytes
    if ix_data.len() < 72 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let label: [u8; 32] = ix_data[0..32]
        .try_into()
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    let updater: [u8; 32] = ix_data[32..64]
        .try_into()
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    let max_staleness_slots = u64::from_le_bytes(
        ix_data[64..72]
            .try_into()
            .map_err(|_| ProgramError::InvalidInstructionData)?,
    );

    // Auth: admin must be signer and match protocol_state.admin
    if !admin.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if !protocol_state.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let ps = ProtocolState::from_account(protocol_state)?;
    if ps.admin != *admin.key() {
        return Err(ProgramError::Custom(
            crate::error::ANCHOR_ERROR_OFFSET + OracleError::Unauthorized as u32,
        ));
    }
    // Verify protocol_state PDA
    let ps_pda = pubkey::create_program_address(
        &[PROTOCOL_STATE_SEED, &[ps.bump]],
        program_id,
    )?;
    if !pubkey::pubkey_eq(&ps_pda, protocol_state.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Derive PriceFeedState PDA: ["price_feed", &label]
    let (expected_pda, bump) =
        pubkey::find_program_address(&[PRICE_FEED_SEED, &label], program_id);
    if !pubkey::pubkey_eq(&expected_pda, price_feed.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Create account via system program CPI
    let bump_ref = [bump];
    let seeds = [
        pinocchio::instruction::Seed::from(PRICE_FEED_SEED),
        pinocchio::instruction::Seed::from(label.as_ref()),
        pinocchio::instruction::Seed::from(bump_ref.as_ref()),
    ];
    let pda_signer = Signer::from(&seeds);

    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(PriceFeedState::LEN);

    crate::cpi_create_account(
        admin,
        price_feed,
        lamports,
        PriceFeedState::LEN as u64,
        program_id,
        &[pda_signer],
    )?;

    // Write initial data
    {
        let data = unsafe { price_feed.borrow_mut_data_unchecked() };

        // Discriminator
        data[0..8].copy_from_slice(&DISC_PRICE_FEED_STATE);
        // bump (offset 8)
        data[8] = bump;
        // version = 1 (offset 9)
        data[9] = 1;
        // label (offset 10)
        data[10..42].copy_from_slice(&label);
        // authority = admin (offset 42)
        data[42..74].copy_from_slice(admin.key());
        // updater (offset 74)
        data[74..106].copy_from_slice(&updater);
        // price = 0 (offset 106) — already zeroed by CreateAccount
        // last_update_slot = 0 (offset 114) — already zeroed
        // last_update_ts = 0 (offset 122) — already zeroed
        // max_staleness_slots (offset 130)
        data[130..138].copy_from_slice(&max_staleness_slots.to_le_bytes());
        // num_updates = 0 (offset 138) — already zeroed
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// UPDATE PRICE
// ---------------------------------------------------------------------------

/// Registered updater pushes a new price with 20% deviation guard.
///
/// Accounts:
///   0. `[signer, writable]` updater
///   1. `[writable]`         price_feed PDA
///
/// Instruction data:
///   0..32   label ([u8; 32])
///   32..40  price (i64 LE)
pub fn update_price(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 2 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let updater = &accounts[0];
    let price_feed = &accounts[1];

    // Parse instruction data: 32 + 8 = 40 bytes
    if ix_data.len() < 40 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let _label: [u8; 32] = ix_data[0..32]
        .try_into()
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    let price = i64::from_le_bytes(
        ix_data[32..40]
            .try_into()
            .map_err(|_| ProgramError::InvalidInstructionData)?,
    );

    // Validate signer
    if !updater.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    // Validate ownership
    if !price_feed.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }

    // Read and validate PriceFeedState
    let feed = PriceFeedState::from_account(price_feed)?;

    // Verify PDA derivation
    let pda = pubkey::create_program_address(
        &[PRICE_FEED_SEED, &feed.label, &[feed.bump]],
        program_id,
    )?;
    if !pubkey::pubkey_eq(&pda, price_feed.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Constraint: updater must match
    if feed.updater != *updater.key() {
        return Err(OracleError::Unauthorized.into());
    }

    // Price must be positive
    if price <= 0 {
        return Err(OracleError::InvalidInputLength.into());
    }

    // Deviation guard: if we have a previous price, reject > 20% deviation
    let prev_price = feed.get_price();
    if prev_price > 0 {
        let prev = prev_price as u64;
        let curr = price as u64;
        let diff = if curr > prev {
            curr.saturating_sub(prev)
        } else {
            prev.saturating_sub(curr)
        };
        let deviation_bps = diff
            .checked_mul(10_000)
            .and_then(|n| n.checked_div(prev))
            .unwrap_or(u64::MAX);
        if deviation_bps > MAX_DEVIATION_BPS {
            return Err(OracleError::PriceDeviationTooLarge.into());
        }
    }

    let clock = Clock::get()?;

    // Write updated fields
    {
        let feed_mut = PriceFeedState::from_account_mut(price_feed)?;
        feed_mut.set_price(price);
        feed_mut.set_last_update_slot(clock.slot);
        feed_mut.set_last_update_ts(clock.unix_timestamp);
        let new_count = feed_mut.get_num_updates().saturating_add(1);
        feed_mut.set_num_updates(new_count);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// SET PRICE UPDATER
// ---------------------------------------------------------------------------

/// Authority rotates the cranker key.
///
/// Accounts:
///   0. `[signer]`   authority
///   1. `[writable]` price_feed PDA
///
/// Instruction data:
///   0..32   label ([u8; 32])
///   32..64  new_updater (Pubkey)
pub fn set_price_updater(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 2 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let authority = &accounts[0];
    let price_feed = &accounts[1];

    // Parse instruction data: 32 + 32 = 64 bytes
    if ix_data.len() < 64 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let _label: [u8; 32] = ix_data[0..32]
        .try_into()
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    let new_updater: [u8; 32] = ix_data[32..64]
        .try_into()
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    // Validate signer
    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    // Validate ownership
    if !price_feed.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }

    // Read and validate PriceFeedState
    let feed = PriceFeedState::from_account(price_feed)?;

    // Verify PDA derivation
    let pda = pubkey::create_program_address(
        &[PRICE_FEED_SEED, &feed.label, &[feed.bump]],
        program_id,
    )?;
    if !pubkey::pubkey_eq(&pda, price_feed.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Constraint: authority must match
    if feed.authority != *authority.key() {
        return Err(OracleError::Unauthorized.into());
    }

    // Write new updater
    {
        let feed_mut = PriceFeedState::from_account_mut(price_feed)?;
        feed_mut.updater = new_updater;
    }

    Ok(())
}
