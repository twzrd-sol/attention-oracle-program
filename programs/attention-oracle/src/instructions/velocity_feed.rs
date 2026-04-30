//! Velocity feed instructions — WZRD native on-chain oracle.
//!
//! This is the primitive. Any Solana program can read WZRD velocity data
//! by deserializing a VelocityFeedState account. No CPI needed — just
//! pass the PDA as a remaining account and read the bytes.
//!
//! Data flow:
//!   Server (EMA scoring) → Keeper (velocity_feed cranker) → On-chain PDA
//!   External program reads PDA → trustless velocity signal in same tx
//!
//! This replaces the Switchboard dependency with a native feed owned by
//! the AO program. Same trust model (server computes, keeper pushes),
//! but the data lives in WZRD PDAs — no third-party program dependency.
//!
//! Handlers:
//!   - `initialize_velocity_feed` — admin creates a VelocityFeedState PDA
//!   - `update_velocity`          — registered updater pushes new velocity data
//!   - `set_velocity_updater`     — authority rotates the updater key
//!
//! CPI consumers read the PDA directly:
//!   ```ignore
//!   let feed = VelocityFeedState::from_account(velocity_feed_account)?;
//!   let velocity = feed.get_velocity_ema();
//!   let trend = feed.get_trend();       // 0=cooling, 1=stable, 2=accelerating, 3=surging
//!   let stale = clock.slot - feed.get_last_update_slot() > feed.get_max_staleness_slots();
//!   ```

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
    ProtocolState, VelocityFeedState, DISC_VELOCITY_FEED_STATE, PROTOCOL_STATE_SEED,
    VELOCITY_FEED_SEED,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum velocity deviation between updates: 50% (5000 BPS).
/// Velocity is more volatile than price, so wider band.
const MAX_DEVIATION_BPS: u64 = 5_000;

// ---------------------------------------------------------------------------
// INITIALIZE VELOCITY FEED
// ---------------------------------------------------------------------------

/// Admin creates a new VelocityFeedState PDA for a model/market.
///
/// Accounts:
///   0. `[signer, writable]` admin
///   1. `[]`                 protocol_state PDA
///   2. `[writable]`         velocity_feed PDA (uninitialized, will be created)
///   3. `[]`                 system_program
///
/// Instruction data (after 8-byte discriminator):
///   0..32   label ([u8; 32]) — model identifier (e.g., "qwen3.5-9b\0\0\0...")
///   32..64  updater (Pubkey) — keeper that can push updates
///   64..72  max_staleness_slots (u64 LE) — max age before feed is stale
///   72..74  market_id (u16 LE) — link to market vault (0 = standalone)
pub fn initialize_velocity_feed(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 4 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let admin = &accounts[0];
    let protocol_state = &accounts[1];
    let velocity_feed = &accounts[2];
    let _system_program = &accounts[3];

    // Parse instruction data: 32 + 32 + 8 + 2 = 74 bytes
    if ix_data.len() < 74 {
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
    let market_id = u16::from_le_bytes(
        ix_data[72..74]
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
    let ps_pda = pubkey::create_program_address(&[PROTOCOL_STATE_SEED, &[ps.bump]], program_id)?;
    if !pubkey::pubkey_eq(&ps_pda, protocol_state.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Derive VelocityFeedState PDA: ["velocity_feed", &label]
    let (expected_pda, bump) =
        pubkey::find_program_address(&[VELOCITY_FEED_SEED, &label], program_id);
    if !pubkey::pubkey_eq(&expected_pda, velocity_feed.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Create account via system program CPI
    let bump_ref = [bump];
    let seeds = [
        pinocchio::instruction::Seed::from(VELOCITY_FEED_SEED),
        pinocchio::instruction::Seed::from(label.as_ref()),
        pinocchio::instruction::Seed::from(bump_ref.as_ref()),
    ];
    let pda_signer = Signer::from(&seeds);

    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(VelocityFeedState::LEN);

    crate::cpi_create_account(
        admin,
        velocity_feed,
        lamports,
        VelocityFeedState::LEN as u64,
        program_id,
        &[pda_signer],
    )?;

    // Write initial data
    {
        let data = unsafe { velocity_feed.borrow_mut_data_unchecked() };

        // Discriminator
        data[0..8].copy_from_slice(&DISC_VELOCITY_FEED_STATE);
        // bump (offset 8)
        data[8] = bump;
        // version = 1 (offset 9)
        data[9] = 1;
        // label (offset 10..42)
        data[10..42].copy_from_slice(&label);
        // authority = admin (offset 42..74)
        data[42..74].copy_from_slice(admin.key());
        // updater (offset 74..106)
        data[74..106].copy_from_slice(&updater);
        // velocity_ema = 0 (offset 106..114) — already zeroed
        // trend = 0 (offset 114) — 0=unknown
        // confidence = 0 (offset 115) — 0=insufficient
        // score = 0 (offset 116..124) — already zeroed
        // last_update_slot = 0 (offset 124..132) — already zeroed
        // last_update_ts = 0 (offset 132..140) — already zeroed
        // max_staleness_slots (offset 140..148)
        data[140..148].copy_from_slice(&max_staleness_slots.to_le_bytes());
        // market_id (offset 148..150)
        data[148..150].copy_from_slice(&market_id.to_le_bytes());
        // num_updates = 0 (offset 150..158) — already zeroed
        // platform = 0 (offset 158) — 0=unknown
        // _reserved = zeroed (offset 159..174) — already zeroed
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// UPDATE VELOCITY
// ---------------------------------------------------------------------------

/// Registered updater pushes new velocity data.
///
/// Accounts:
///   0. `[signer, writable]` updater (keeper)
///   1. `[writable]`         velocity_feed PDA
///
/// Instruction data (after 8-byte discriminator):
///   0..32   label ([u8; 32])
///   32..40  velocity_ema (i64 LE) — scaled by 1e6 for precision
///   40      trend (u8) — 0=cooling, 1=stable, 2=accelerating, 3=surging
///   41      confidence (u8) — 0=insufficient, 1=low, 2=normal, 3=high
///   42..50  score (i64 LE) — raw score scaled by 1e6
///   50      platform (u8) — 0=unknown, 1=huggingface, 2=github, 3=openrouter, 4=artificialanalysis
pub fn update_velocity(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 2 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let updater = &accounts[0];
    let velocity_feed = &accounts[1];

    // Parse instruction data: 32 + 8 + 1 + 1 + 8 + 1 = 51 bytes
    if ix_data.len() < 51 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let _label: [u8; 32] = ix_data[0..32]
        .try_into()
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    let velocity_ema = i64::from_le_bytes(
        ix_data[32..40]
            .try_into()
            .map_err(|_| ProgramError::InvalidInstructionData)?,
    );
    let trend = ix_data[40];
    let confidence = ix_data[41];
    let score = i64::from_le_bytes(
        ix_data[42..50]
            .try_into()
            .map_err(|_| ProgramError::InvalidInstructionData)?,
    );
    let platform = ix_data[50];

    // Validate signer
    if !updater.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    // Validate ownership
    if !velocity_feed.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }

    // Read and validate VelocityFeedState
    let feed = VelocityFeedState::from_account(velocity_feed)?;

    // Verify PDA derivation
    let pda = pubkey::create_program_address(
        &[VELOCITY_FEED_SEED, &feed.label, &[feed.bump]],
        program_id,
    )?;
    if !pubkey::pubkey_eq(&pda, velocity_feed.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Constraint: updater must match
    if feed.updater != *updater.key() {
        return Err(OracleError::Unauthorized.into());
    }

    // Validate trend (0-3) and confidence (0-3)
    if trend > 3 || confidence > 3 {
        return Err(ProgramError::InvalidInstructionData);
    }

    // Velocity must be non-negative
    if velocity_ema < 0 {
        return Err(OracleError::InvalidInputLength.into());
    }

    // Deviation guard: if we have a previous velocity, reject > 50% deviation
    let prev_velocity = feed.get_velocity_ema();
    if prev_velocity > 0 {
        let prev = prev_velocity as u64;
        let curr = velocity_ema as u64;
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
        let feed_mut = VelocityFeedState::from_account_mut(velocity_feed)?;
        feed_mut.set_velocity_ema(velocity_ema);
        feed_mut.trend = trend;
        feed_mut.confidence = confidence;
        feed_mut.set_score(score);
        feed_mut.set_last_update_slot(clock.slot);
        feed_mut.set_last_update_ts(clock.unix_timestamp);
        feed_mut.platform = platform;
        let new_count = feed_mut.get_num_updates().saturating_add(1);
        feed_mut.set_num_updates(new_count);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// SET VELOCITY UPDATER
// ---------------------------------------------------------------------------

/// Authority rotates the keeper key.
///
/// Accounts:
///   0. `[signer]`   authority
///   1. `[writable]` velocity_feed PDA
///
/// Instruction data (after 8-byte discriminator):
///   0..32   label ([u8; 32])
///   32..64  new_updater (Pubkey)
pub fn set_velocity_updater(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 2 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let authority = &accounts[0];
    let velocity_feed = &accounts[1];

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
    if !velocity_feed.is_owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }

    // Read and validate VelocityFeedState
    let feed = VelocityFeedState::from_account(velocity_feed)?;

    // Verify PDA derivation
    let pda = pubkey::create_program_address(
        &[VELOCITY_FEED_SEED, &feed.label, &[feed.bump]],
        program_id,
    )?;
    if !pubkey::pubkey_eq(&pda, velocity_feed.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Constraint: authority must match
    if feed.authority != *authority.key() {
        return Err(OracleError::Unauthorized.into());
    }

    // Write new updater
    {
        let feed_mut = VelocityFeedState::from_account_mut(velocity_feed)?;
        feed_mut.updater = new_updater;
    }

    Ok(())
}
