//! AO v2 — Attention Oracle (Pinocchio)
//!
//! Drop-in replacement for the Anchor-based AO program.
//! Same program ID, same account layouts, same discriminators.

#![cfg_attr(not(test), no_std)]

use pinocchio::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

// ============================================================================
// PROGRAM ID
// ============================================================================

/// Program ID: GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
pub const ID: Pubkey = [
    0xea, 0x78, 0x84, 0xf3, 0x5f, 0xf2, 0xba, 0xec, 0xf9, 0x23, 0x05, 0xc3, 0x2e, 0x3e, 0xa1, 0x36,
    0xce, 0x84, 0xfb, 0xdf, 0x19, 0x4c, 0xf1, 0x8b, 0x9a, 0xfd, 0xa3, 0x82, 0x1d, 0xa4, 0xb0, 0x6b,
];

// ============================================================================
// MODULES
// ============================================================================

pub mod error;
pub mod instructions;
pub mod keccak;
pub mod state;

#[cfg(feature = "strategy")]
pub mod klend;

/// Token-2022 program ID.
pub const TOKEN_2022_ID: Pubkey = [
    0x06, 0xdd, 0xf6, 0xe1, 0xee, 0x75, 0x8f, 0xde, 0x18, 0x42, 0x5d, 0xbc, 0xe4, 0x6c, 0xcd, 0xda,
    0xb6, 0x1a, 0xfc, 0x4d, 0x83, 0xb9, 0x0d, 0x27, 0xfe, 0xbd, 0xf9, 0x28, 0xd8, 0xa1, 0x8b, 0xfc,
];

/// System program ID (all zeros).
pub const SYSTEM_ID: Pubkey = [0u8; 32];

/// SPL Token program ID.
pub const SPL_TOKEN_ID: Pubkey = [
    0x06, 0xdd, 0xf6, 0xe1, 0xd7, 0x65, 0xa1, 0x93, 0xd9, 0xcb, 0xe1, 0x46, 0xce, 0xeb, 0x79, 0xac,
    0x1c, 0xb4, 0x85, 0xed, 0x5f, 0x5b, 0x37, 0x91, 0x3a, 0x8c, 0xf5, 0x85, 0x7e, 0xff, 0x00, 0xa9,
];

/// Shared CreateAccount CPI helper — avoids const-generic monomorphization.
#[inline(never)]
pub fn cpi_create_account(
    from: &pinocchio::account_info::AccountInfo,
    to: &pinocchio::account_info::AccountInfo,
    lamports: u64,
    space: u64,
    owner: &pinocchio::pubkey::Pubkey,
    signers: &[pinocchio::instruction::Signer],
) -> ProgramResult {
    // System program CreateAccount instruction layout:
    // [0..4]  = 0u32 (instruction index)
    // [4..12] = lamports (u64)
    // [12..20] = space (u64)
    // [20..52] = owner (Pubkey)
    let mut data = [0u8; 52];
    // instruction index 0 is already zero
    data[4..12].copy_from_slice(&lamports.to_le_bytes());
    data[12..20].copy_from_slice(&space.to_le_bytes());
    data[20..52].copy_from_slice(owner);
    let metas = [
        pinocchio::instruction::AccountMeta::writable_signer(from.key()),
        pinocchio::instruction::AccountMeta::writable_signer(to.key()),
    ];
    let ix = pinocchio::instruction::Instruction {
        program_id: &crate::SYSTEM_ID,
        accounts: &metas,
        data: &data,
    };
    pinocchio::cpi::slice_invoke_signed(&ix, &[from, to], signers)
}

// Re-export instruction sub-modules for ergonomic access.
pub use instructions::{admin, global, governance, signal, vault, velocity_feed};

#[cfg(feature = "strategy")]
pub use instructions::strategy;

#[cfg(feature = "channel_staking")]
pub use instructions::channel_staking;

#[cfg(feature = "prediction_markets")]
pub use instructions::markets;

#[cfg(feature = "price_feed")]
pub use instructions::price_feed;

// ============================================================================
// ENTRYPOINT
// ============================================================================

pinocchio::program_entrypoint!(process_instruction);
pinocchio::default_allocator!();
pinocchio::nostd_panic_handler!();

/// Top-level instruction router.
///
/// Matches on 8-byte Anchor discriminators (`SHA-256("global:<ix_name>")[..8]`).
/// The remaining bytes after the discriminator are forwarded to the handler as
/// `ix_data`.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // Verify program ID.
    if program_id != &crate::ID {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Split discriminator from payload.
    let disc: &[u8] = instruction_data
        .get(..8)
        .ok_or(ProgramError::InvalidInstructionData)?;
    let ix_data: &[u8] = &instruction_data[8..];

    match disc {
        // ==================================================================
        // VAULT
        // ==================================================================

        // initialize_protocol_state
        [0xe5, 0xa8, 0x78, 0xa6, 0x07, 0x1f, 0x3b, 0xed] => {
            vault::initialize_protocol_state(program_id, accounts, ix_data)
        }

        // initialize_market_vault
        [0x19, 0x66, 0xcb, 0x77, 0x97, 0x14, 0x8f, 0xde] => {
            vault::initialize_market_vault(program_id, accounts, ix_data)
        }

        // realloc_market_vault
        [0x89, 0x52, 0x34, 0x4f, 0xde, 0x32, 0x52, 0x97] => {
            vault::realloc_market_vault(program_id, accounts, ix_data)
        }

        // deposit_market
        [0xd4, 0x35, 0xba, 0xc1, 0x93, 0x35, 0x8f, 0x7b] => {
            vault::deposit_market(program_id, accounts, ix_data)
        }

        // update_attention
        [0x7b, 0xf7, 0x75, 0x86, 0xd0, 0x6b, 0x6c, 0x32] => {
            vault::update_attention(program_id, accounts, ix_data)
        }

        // update_nav
        [0x38, 0x10, 0xea, 0x6d, 0x9b, 0xa5, 0x05, 0x00] => {
            vault::update_nav(program_id, accounts, ix_data)
        }

        // claim_yield
        [0x31, 0x4a, 0x6f, 0x07, 0xba, 0x16, 0x3d, 0xa5] => {
            vault::claim_yield(program_id, accounts, ix_data)
        }

        // settle_market
        [0xc1, 0x99, 0x5f, 0xd8, 0xa6, 0x06, 0x90, 0xd9] => {
            vault::settle_market(program_id, accounts, ix_data)
        }

        // ==================================================================
        // GLOBAL (merkle claims)
        // ==================================================================

        // initialize_global_root
        [0xca, 0x36, 0x6b, 0xf6, 0x18, 0xf7, 0x4b, 0xfd] => {
            global::initialize_global_root(program_id, accounts, ix_data)
        }

        // publish_global_root
        [0x51, 0x8d, 0xe2, 0x16, 0xfe, 0xa7, 0x62, 0xff] => {
            global::publish_global_root(program_id, accounts, ix_data)
        }

        // claim_global_v2
        [0xf8, 0x2c, 0xaa, 0x65, 0x31, 0xaa, 0x8c, 0x7e] => {
            global::claim_global_v2(program_id, accounts, ix_data)
        }

        // claim_global_sponsored_v2
        [0x59, 0x54, 0x84, 0x50, 0x8b, 0x5c, 0x5e, 0x04] => {
            global::claim_global_sponsored_v2(program_id, accounts, ix_data)
        }

        // ------------------------------------------------------------------
        // BACKWARD COMPAT: V1 claims (existing claim receipts)
        // ------------------------------------------------------------------

        // claim_global (V1)
        [0x36, 0xb4, 0x97, 0x5b, 0x48, 0xf3, 0x6e, 0xf7] => {
            global::claim_global_v1(program_id, accounts, ix_data)
        }

        // claim_global_sponsored (V1)
        [0x15, 0x73, 0xd0, 0x49, 0x78, 0xf0, 0xf4, 0x94] => {
            global::claim_global_sponsored_v1(program_id, accounts, ix_data)
        }

        // ==================================================================
        // GOVERNANCE
        // ==================================================================

        // harvest_fees
        [0x5a, 0x95, 0x9e, 0xf1, 0xa3, 0xba, 0x9b, 0xca] => {
            governance::harvest_fees(program_id, accounts, ix_data)
        }

        // withdraw_fees_from_mint
        [0x2a, 0xc3, 0x96, 0x0a, 0xb5, 0xb1, 0x5e, 0x83] => {
            governance::withdraw_fees_from_mint(program_id, accounts, ix_data)
        }

        // route_treasury
        [0x58, 0x65, 0x6a, 0x36, 0x5a, 0x28, 0x43, 0x39] => {
            governance::route_treasury(program_id, accounts, ix_data)
        }

        // ==================================================================
        // ADMIN
        // ==================================================================

        // realloc_legacy_protocol
        [0xa4, 0xe6, 0xc5, 0xe0, 0xfe, 0xd9, 0x6b, 0xaa] => {
            admin::realloc_legacy_protocol(program_id, accounts, ix_data)
        }

        // admin_fix_ccm_authority
        [0x90, 0x67, 0xae, 0xdb, 0x12, 0x27, 0x9d, 0x35] => {
            admin::admin_fix_ccm_authority(program_id, accounts, ix_data)
        }

        // set_treasury
        [0x39, 0x61, 0xc4, 0x5f, 0xc3, 0xce, 0x6a, 0x88] => {
            admin::set_treasury(program_id, accounts, ix_data)
        }

        // update_protocol_state
        [0x57, 0x15, 0x8e, 0xb0, 0xba, 0xcd, 0x59, 0x16] => {
            admin::update_protocol_state(program_id, accounts, ix_data)
        }

        // ==================================================================
        // STRATEGY (feature-gated)
        // ==================================================================
        #[cfg(feature = "strategy")]
        // initialize_strategy_vault
        [0xd2, 0xf3, 0x8d, 0x00, 0xcb, 0xf6, 0x04, 0xe1] => {
            strategy::initialize_strategy_vault(program_id, accounts, ix_data)
        }

        #[cfg(feature = "strategy")]
        // deploy_to_strategy
        [0xd7, 0x31, 0x3d, 0xde, 0xb4, 0x3c, 0x09, 0x76] => {
            strategy::deploy_to_strategy(program_id, accounts, ix_data)
        }

        #[cfg(feature = "strategy")]
        // withdraw_from_strategy
        [0x8c, 0xef, 0x41, 0x36, 0x7d, 0x80, 0xdf, 0x7d] => {
            strategy::withdraw_from_strategy(program_id, accounts, ix_data)
        }

        #[cfg(feature = "strategy")]
        // harvest_strategy_yield
        [0x43, 0xd3, 0xf9, 0x57, 0x20, 0xb1, 0xe3, 0xd5] => {
            strategy::harvest_strategy_yield(program_id, accounts, ix_data)
        }

        #[cfg(feature = "strategy")]
        // emergency_unwind
        [0x89, 0xab, 0x54, 0x7d, 0x98, 0x6b, 0x31, 0xf8] => {
            strategy::emergency_unwind(program_id, accounts, ix_data)
        }

        // ==================================================================
        // CHANNEL STAKING (feature-gated)
        // ==================================================================
        #[cfg(feature = "channel_staking")]
        // initialize_fee_config
        [0x3e, 0xa2, 0x14, 0x85, 0x79, 0x41, 0x91, 0x1b] => {
            channel_staking::initialize_fee_config(program_id, accounts, ix_data)
        }

        #[cfg(feature = "channel_staking")]
        // create_channel_config_v2
        [0x79, 0x4d, 0xda, 0x3e, 0xab, 0x2f, 0xcc, 0xb6] => {
            admin::create_channel_config_v2(program_id, accounts, ix_data)
        }

        #[cfg(feature = "channel_staking")]
        // initialize_stake_pool
        [0x30, 0xbd, 0xf3, 0x49, 0x13, 0x43, 0x24, 0x53] => {
            channel_staking::initialize_stake_pool(program_id, accounts, ix_data)
        }

        #[cfg(feature = "channel_staking")]
        // stake_channel
        [0x2b, 0xdb, 0xa0, 0x73, 0x0d, 0xc8, 0x49, 0xde] => {
            channel_staking::stake_channel(program_id, accounts, ix_data)
        }

        #[cfg(feature = "channel_staking")]
        // unstake_channel
        [0xe9, 0x36, 0x5a, 0x1d, 0x81, 0x8a, 0x17, 0x64] => {
            channel_staking::unstake_channel(program_id, accounts, ix_data)
        }

        #[cfg(feature = "channel_staking")]
        // claim_channel_rewards
        [0x6a, 0xc5, 0x9e, 0x9c, 0x16, 0xd1, 0x5c, 0x51] => {
            channel_staking::claim_channel_rewards(program_id, accounts, ix_data)
        }

        // ==================================================================
        // PREDICTION MARKETS (feature-gated)
        // ==================================================================
        #[cfg(feature = "prediction_markets")]
        // create_market
        [0x67, 0xe2, 0x61, 0xeb, 0xc8, 0xbc, 0xfb, 0xfe] => {
            markets::create_market(program_id, accounts, ix_data)
        }

        #[cfg(feature = "prediction_markets")]
        // initialize_market_tokens
        [0x6e, 0x86, 0xb4, 0x05, 0x13, 0x97, 0x50, 0x49] => {
            markets::initialize_market_tokens(program_id, accounts, ix_data)
        }

        #[cfg(feature = "prediction_markets")]
        // initialize_market_tokens_v2
        [0xb4, 0xa8, 0x58, 0xf2, 0x74, 0xf7, 0xe5, 0x69] => {
            markets::initialize_market_tokens_v2(program_id, accounts, ix_data)
        }

        #[cfg(feature = "prediction_markets")]
        // mint_shares
        [0x18, 0xc4, 0x84, 0x00, 0xb7, 0x9e, 0xd8, 0x8e] => {
            markets::mint_shares(program_id, accounts, ix_data)
        }

        #[cfg(feature = "prediction_markets")]
        // redeem_shares
        [0xef, 0x9a, 0xe0, 0x59, 0xf0, 0xc4, 0x2a, 0xbb] => {
            markets::redeem_shares(program_id, accounts, ix_data)
        }

        #[cfg(feature = "prediction_markets")]
        // resolve_market
        [0x9b, 0x17, 0x50, 0xad, 0x2e, 0x4a, 0x17, 0xef] => {
            markets::resolve_market(program_id, accounts, ix_data)
        }

        #[cfg(feature = "prediction_markets")]
        // settle
        [0xaf, 0x2a, 0xb9, 0x57, 0x90, 0x83, 0x66, 0xd4] => {
            markets::settle(program_id, accounts, ix_data)
        }

        #[cfg(feature = "prediction_markets")]
        // sweep_residual
        [0xe6, 0x76, 0x23, 0x9b, 0xa5, 0x6e, 0x8d, 0x13] => {
            markets::sweep_residual(program_id, accounts, ix_data)
        }

        #[cfg(feature = "prediction_markets")]
        // close_market
        [0x58, 0x9a, 0xf8, 0xba, 0x30, 0x0e, 0x7b, 0xf4] => {
            markets::close_market(program_id, accounts, ix_data)
        }

        #[cfg(feature = "prediction_markets")]
        // close_market_mints
        [0xde, 0xe7, 0x4c, 0x62, 0x77, 0x06, 0x74, 0xb6] => {
            markets::close_market_mints(program_id, accounts, ix_data)
        }

        // ==================================================================
        // PRICE FEED (feature-gated)
        // ==================================================================
        #[cfg(feature = "price_feed")]
        // initialize_price_feed
        [0x44, 0xb4, 0x51, 0x14, 0x66, 0xd5, 0x91, 0xe9] => {
            price_feed::initialize_price_feed(program_id, accounts, ix_data)
        }

        #[cfg(feature = "price_feed")]
        // update_price
        [0x3d, 0x22, 0x75, 0x9b, 0x4b, 0x22, 0x7b, 0xd0] => {
            price_feed::update_price(program_id, accounts, ix_data)
        }

        #[cfg(feature = "price_feed")]
        // set_price_updater
        [0xfc, 0xfc, 0x0b, 0x9d, 0x92, 0xfd, 0x7c, 0x31] => {
            price_feed::set_price_updater(program_id, accounts, ix_data)
        }

        // ==================================================================
        // SIGNAL CONSUMER (on-chain velocity reads for CPI callers)
        // ==================================================================

        // read_velocity — read per-position attention multiplier
        [0x83, 0xd9, 0x4b, 0x3e, 0x2f, 0x51, 0x15, 0x31] => {
            signal::read_velocity(program_id, accounts, ix_data)
        }

        // read_market_velocity — read aggregate market velocity
        [0x3b, 0xd6, 0x86, 0xf3, 0x6a, 0x96, 0x05, 0xcc] => {
            signal::read_market_velocity(program_id, accounts, ix_data)
        }

        // ==================================================================
        // VELOCITY FEED (native on-chain oracle)
        // ==================================================================

        // initialize_velocity_feed
        [0xf4, 0xa2, 0x54, 0x05, 0xed, 0x5e, 0xc1, 0x61] => {
            velocity_feed::initialize_velocity_feed(program_id, accounts, ix_data)
        }

        // update_velocity
        [0x8e, 0xd5, 0xd9, 0xac, 0xb4, 0x8b, 0x6d, 0xa4] => {
            velocity_feed::update_velocity(program_id, accounts, ix_data)
        }

        // set_velocity_updater
        [0x75, 0xd7, 0xff, 0x3f, 0x81, 0xe3, 0xd3, 0x71] => {
            velocity_feed::set_velocity_updater(program_id, accounts, ix_data)
        }

        // ==================================================================
        // UNKNOWN
        // ==================================================================
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
