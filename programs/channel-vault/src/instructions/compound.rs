//! Compound pending deposits into Oracle stake.
//!
//! This is a permissionless instruction - anyone can call it.
//! The compound strategy:
//! 1. If no active position: stake all pending with 7-day lock
//! 2. If active position and lock expired: unstake, then stake total with 7-day lock
//! 3. If active position and lock not expired: wait (revert)

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint as MintInterface, TokenAccount, TokenInterface},
};

use crate::constants::{TOKEN_2022_PROGRAM_ID, VAULT_CCM_BUFFER_SEED, VAULT_ORACLE_POSITION_SEED, VAULT_SEED};
use crate::errors::VaultError;
use crate::events::Compounded;
use crate::state::{ChannelVault, VaultOraclePosition};

// Import Oracle types, CPI, and error codes
use token_2022::{
    self,
    cpi::accounts::{ClaimChannelRewards, StakeChannel, UnstakeChannel},
    ChannelConfigV2, ChannelStakePool, ProtocolState,
    CHANNEL_STAKE_POOL_SEED, PROTOCOL_SEED, STAKE_VAULT_SEED,
    ORACLE_ERROR_NO_REWARDS_TO_CLAIM, ORACLE_ERROR_POOL_IS_SHUTDOWN,
    ORACLE_ERROR_CLAIM_EXCEEDS_AVAILABLE,
};

/// Check if an Anchor CPI error is an expected Oracle claim failure
/// that can be safely swallowed during compound.
fn is_expected_claim_error(e: &anchor_lang::error::Error) -> bool {
    use anchor_lang::solana_program::program_error::ProgramError;

    let code = match e {
        anchor_lang::error::Error::AnchorError(ae) => Some(ae.error_code_number),
        anchor_lang::error::Error::ProgramError(pe) => match &pe.program_error {
            ProgramError::Custom(c) => Some(*c),
            _ => None,
        },
    };
    matches!(
        code,
        Some(ORACLE_ERROR_NO_REWARDS_TO_CLAIM | ORACLE_ERROR_POOL_IS_SHUTDOWN | ORACLE_ERROR_CLAIM_EXCEEDS_AVAILABLE)
    )
}

#[derive(Accounts)]
pub struct Compound<'info> {
    /// Anyone can call this (permissionless crank)
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [VAULT_SEED, vault.channel_config.as_ref()],
        bump = vault.bump,
        constraint = !vault.paused @ VaultError::VaultPaused,
    )]
    pub vault: Box<Account<'info, ChannelVault>>,

    #[account(
        mut,
        seeds = [VAULT_ORACLE_POSITION_SEED, vault.key().as_ref()],
        bump = vault_oracle_position.bump,
    )]
    pub vault_oracle_position: Box<Account<'info, VaultOraclePosition>>,

    /// Vault's CCM buffer
    #[account(
        mut,
        seeds = [VAULT_CCM_BUFFER_SEED, vault.key().as_ref()],
        bump,
    )]
    pub vault_ccm_buffer: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CCM mint
    #[account(address = vault.ccm_mint)]
    pub ccm_mint: Box<InterfaceAccount<'info, MintInterface>>,

    // -------------------------------------------------------------------------
    // Oracle Accounts
    // -------------------------------------------------------------------------

    /// Oracle program
    /// CHECK: Validated by address
    #[account(address = token_2022::ID)]
    pub oracle_program: AccountInfo<'info>,

    /// Oracle protocol state
    #[account(
        seeds = [PROTOCOL_SEED, oracle_protocol.mint.as_ref()],
        bump = oracle_protocol.bump,
        seeds::program = token_2022::ID,
    )]
    pub oracle_protocol: Box<Account<'info, ProtocolState>>,

    /// Oracle channel config
    #[account(address = vault.channel_config)]
    pub oracle_channel_config: Box<Account<'info, ChannelConfigV2>>,

    /// Oracle stake pool for channel
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_POOL_SEED, oracle_channel_config.key().as_ref()],
        bump = oracle_stake_pool.bump,
        seeds::program = token_2022::ID,
    )]
    pub oracle_stake_pool: Box<Account<'info, ChannelStakePool>>,

    /// Oracle vault holding staked tokens
    #[account(
        mut,
        seeds = [STAKE_VAULT_SEED, oracle_stake_pool.key().as_ref()],
        bump,
        seeds::program = token_2022::ID,
    )]
    pub oracle_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Vault's stake position in Oracle (may need init)
    /// CHECK: Validated/created by Oracle CPI
    #[account(mut)]
    pub oracle_user_stake: UncheckedAccount<'info>,

    /// NFT mint for soulbound receipt
    /// CHECK: Created/validated by Oracle CPI
    #[account(mut)]
    pub oracle_nft_mint: UncheckedAccount<'info>,

    /// Vault's NFT ATA
    /// CHECK: Created by Oracle CPI
    #[account(mut)]
    pub vault_nft_ata: UncheckedAccount<'info>,

    // -------------------------------------------------------------------------
    // Programs
    // -------------------------------------------------------------------------

    #[account(
        constraint = token_2022_program.key() == TOKEN_2022_PROGRAM_ID @ VaultError::InvalidTokenProgram,
    )]
    pub token_2022_program: Interface<'info, TokenInterface>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler<'info>(ctx: Context<'_, '_, '_, 'info, Compound<'info>>) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let position = &ctx.accounts.vault_oracle_position;
    let clock = Clock::get()?;

    // Check if there's anything to compound
    let pending = vault.pending_deposits;
    let reserved_for_withdrawals = vault.pending_withdrawals;
    let stakeable_pending = pending.saturating_sub(reserved_for_withdrawals);

    // Need either stakeable pending deposits OR an active position to roll over
    require!(stakeable_pending > 0 || position.is_active, VaultError::NothingToCompound);

    let channel_config_key = vault.channel_config;
    let vault_bump = vault.bump;

    // Vault signer seeds
    let signer_seeds: &[&[&[u8]]] = &[&[
        VAULT_SEED,
        channel_config_key.as_ref(),
        &[vault_bump],
    ]];

    // Start with stakeable pending (reserve stays in buffer for withdrawals)
    let mut amount_to_stake = stakeable_pending;

    // Track rewards claimed for event
    let mut rewards_claimed: u64 = 0;

    // If there's an active position, we need to check if lock expired
    if position.is_active {
        // Check lock status
        if position.lock_end_slot > clock.slot {
            // Lock not expired - can't compound yet
            msg!("Oracle stake still locked until slot {}", position.lock_end_slot);
            return Err(VaultError::OracleStakeLocked.into());
        }

        // Claim any pending rewards BEFORE unstaking
        // This transfers rewards from Oracle vault to our CCM buffer
        msg!("Claiming pending rewards before unstake...");

        // Get buffer balance before claim
        let buffer_before = ctx.accounts.vault_ccm_buffer.amount;

        let claim_accounts = ClaimChannelRewards {
            user: ctx.accounts.vault.to_account_info(),
            channel_config: ctx.accounts.oracle_channel_config.to_account_info(),
            mint: ctx.accounts.ccm_mint.to_account_info(),
            stake_pool: ctx.accounts.oracle_stake_pool.to_account_info(),
            user_stake: ctx.accounts.oracle_user_stake.to_account_info(),
            vault: ctx.accounts.oracle_vault.to_account_info(),
            user_token_account: ctx.accounts.vault_ccm_buffer.to_account_info(),
            token_program: ctx.accounts.token_2022_program.to_account_info(),
        };

        let claim_ctx = CpiContext::new_with_signer(
            ctx.accounts.oracle_program.to_account_info(),
            claim_accounts,
            signer_seeds,
        );

        // Try to claim - only swallow expected Oracle errors; bubble real failures
        match token_2022::cpi::claim_channel_rewards(claim_ctx) {
            Ok(_) => {
                // Reload buffer to get new balance
                ctx.accounts.vault_ccm_buffer.reload()?;
                let buffer_after = ctx.accounts.vault_ccm_buffer.amount;
                rewards_claimed = buffer_after.saturating_sub(buffer_before);
                msg!("Claimed {} CCM in rewards", rewards_claimed);
            }
            Err(e) => {
                if is_expected_claim_error(&e) {
                    msg!("No rewards to claim (expected), continuing");
                } else {
                    msg!("Claim CPI failed unexpectedly: {:?}", e);
                    return Err(e);
                }
            }
        }

        // Lock expired - unstake
        msg!("Unstaking {} CCM from Oracle", position.stake_amount);

        // Capture buffer balance before unstake (after any claim)
        ctx.accounts.vault_ccm_buffer.reload()?;
        let buffer_before_unstake = ctx.accounts.vault_ccm_buffer.amount;

        let unstake_accounts = UnstakeChannel {
            user: ctx.accounts.vault.to_account_info(),
            channel_config: ctx.accounts.oracle_channel_config.to_account_info(),
            mint: ctx.accounts.ccm_mint.to_account_info(),
            stake_pool: ctx.accounts.oracle_stake_pool.to_account_info(),
            user_stake: ctx.accounts.oracle_user_stake.to_account_info(),
            vault: ctx.accounts.oracle_vault.to_account_info(),
            user_token_account: ctx.accounts.vault_ccm_buffer.to_account_info(),
            nft_mint: ctx.accounts.oracle_nft_mint.to_account_info(),
            nft_ata: ctx.accounts.vault_nft_ata.to_account_info(),
            token_program: ctx.accounts.token_2022_program.to_account_info(),
            associated_token_program: ctx.accounts.associated_token_program.to_account_info(),
        };

        let unstake_ctx = CpiContext::new_with_signer(
            ctx.accounts.oracle_program.to_account_info(),
            unstake_accounts,
            signer_seeds,
        );

        token_2022::cpi::unstake_channel(unstake_ctx)?;

        // Measure actual received from Oracle (net of transfer fees)
        // Using position.stake_amount here would cause phantom inflation:
        // the Oracle holds less than what we recorded due to inbound transfer fee,
        // so the return is smaller than position.stake_amount.
        ctx.accounts.vault_ccm_buffer.reload()?;
        let unstaked_received = ctx.accounts.vault_ccm_buffer.amount
            .checked_sub(buffer_before_unstake)
            .ok_or(VaultError::MathOverflow)?;

        msg!("Unstaked {} CCM from Oracle (actual received)", unstaked_received);

        // Add actual unstaked amount + claimed rewards to what we'll re-stake
        amount_to_stake = amount_to_stake
            .checked_add(unstaked_received)
            .ok_or(VaultError::MathOverflow)?
            .checked_add(rewards_claimed)
            .ok_or(VaultError::MathOverflow)?;
    }

    // Now stake the total amount
    if amount_to_stake > 0 {
        msg!("Staking {} CCM in Oracle with {} slot lock", amount_to_stake, vault.lock_duration_slots);

        let stake_accounts = StakeChannel {
            user: ctx.accounts.vault.to_account_info(),
            payer: ctx.accounts.payer.to_account_info(),
            protocol_state: ctx.accounts.oracle_protocol.to_account_info(),
            channel_config: ctx.accounts.oracle_channel_config.to_account_info(),
            mint: ctx.accounts.ccm_mint.to_account_info(),
            stake_pool: ctx.accounts.oracle_stake_pool.to_account_info(),
            user_stake: ctx.accounts.oracle_user_stake.to_account_info(),
            vault: ctx.accounts.oracle_vault.to_account_info(),
            user_token_account: ctx.accounts.vault_ccm_buffer.to_account_info(),
            nft_mint: ctx.accounts.oracle_nft_mint.to_account_info(),
            nft_ata: ctx.accounts.vault_nft_ata.to_account_info(),
            token_program: ctx.accounts.token_2022_program.to_account_info(),
            associated_token_program: ctx.accounts.associated_token_program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            rent: ctx.accounts.rent.to_account_info(),
        };

        let stake_ctx = CpiContext::new_with_signer(
            ctx.accounts.oracle_program.to_account_info(),
            stake_accounts,
            signer_seeds,
        );

        token_2022::cpi::stake_channel(stake_ctx, amount_to_stake, vault.lock_duration_slots)?;
    }

    // Update vault state
    let vault = &mut ctx.accounts.vault;
    vault.total_staked = amount_to_stake;
    // Reduce pending_deposits by what we staked
    // Remaining in buffer = pending_withdrawals (reserved) + emergency_reserve
    vault.pending_deposits = vault
        .pending_deposits
        .checked_sub(stakeable_pending)
        .ok_or(VaultError::MathOverflow)?;
    vault.compound_count = vault.compound_count.saturating_add(1);
    vault.last_compound_slot = clock.slot;

    // Update position state
    let position = &mut ctx.accounts.vault_oracle_position;
    position.is_active = amount_to_stake > 0;
    position.stake_amount = amount_to_stake;
    position.lock_end_slot = clock.slot.saturating_add(vault.lock_duration_slots);
    position.oracle_user_stake = ctx.accounts.oracle_user_stake.key();
    position.oracle_nft_mint = ctx.accounts.oracle_nft_mint.key();
    position.oracle_nft_ata = ctx.accounts.vault_nft_ata.key();

    emit!(Compounded {
        vault: vault.key(),
        pending_staked: stakeable_pending,
        total_staked: vault.total_staked,
        rewards_claimed,
        compound_count: vault.compound_count,
        caller: ctx.accounts.payer.key(),
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Compounded: staked={} (reserved {} for withdrawals), total_staked={}, lock_end={}",
        stakeable_pending,
        reserved_for_withdrawals,
        vault.total_staked,
        position.lock_end_slot
    );

    Ok(())
}
