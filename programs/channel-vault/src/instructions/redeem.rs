//! Withdrawal and redemption instructions.
//!
//! Exit paths:
//! 1. request_withdraw -> wait queue duration -> complete_withdraw (full value)
//! 2. instant_redeem (from buffer/reserve, 20% penalty, no Oracle touch)
//! 3. Swap vLOFI on DEX (not handled here)
//! 4. admin_emergency_unstake (admin-only, triggers Oracle emergency unstake)

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Burn, Token},
    token_interface::{Mint as MintInterface, TokenAccount, TokenInterface, TransferChecked},
};

use crate::constants::{
    TOKEN_2022_PROGRAM_ID, VAULT_CCM_BUFFER_SEED, VAULT_ORACLE_POSITION_SEED, VAULT_SEED,
    WITHDRAW_REQUEST_SEED,
};
use crate::errors::VaultError;
use crate::events::{WithdrawCompleted, WithdrawRequested};
use crate::state::{ChannelVault, UserVaultState, VaultOraclePosition, WithdrawRequest};

// Oracle CPI types
use token_2022::{
    self,
    cpi::accounts::{EmergencyUnstakeChannel, UnstakeChannel},
    ChannelConfigV2, ChannelStakePool,
    CHANNEL_STAKE_POOL_SEED, STAKE_VAULT_SEED,
};

// =============================================================================
// REQUEST WITHDRAW
// =============================================================================

/// User state seed
pub const USER_VAULT_STATE_SEED: &[u8] = b"user_state";

#[derive(Accounts)]
pub struct RequestWithdraw<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [VAULT_SEED, vault.channel_config.as_ref()],
        bump = vault.bump,
    )]
    pub vault: Box<Account<'info, ChannelVault>>,

    /// User's vault state (tracks request IDs)
    #[account(
        init_if_needed,
        payer = user,
        space = UserVaultState::LEN,
        seeds = [USER_VAULT_STATE_SEED, vault.key().as_ref(), user.key().as_ref()],
        bump
    )]
    pub user_vault_state: Box<Account<'info, UserVaultState>>,

    /// vLOFI mint
    #[account(mut, address = vault.vlofi_mint)]
    pub vlofi_mint: Box<Account<'info, anchor_spl::token::Mint>>,

    /// User's vLOFI account
    #[account(
        mut,
        constraint = user_vlofi.owner == user.key() @ VaultError::Unauthorized,
        constraint = user_vlofi.mint == vlofi_mint.key() @ VaultError::InvalidMint,
    )]
    pub user_vlofi: Box<Account<'info, anchor_spl::token::TokenAccount>>,

    /// Withdraw request PDA
    #[account(
        init,
        payer = user,
        space = WithdrawRequest::LEN,
        seeds = [
            WITHDRAW_REQUEST_SEED,
            vault.key().as_ref(),
            user.key().as_ref(),
            &user_vault_state.next_request_id.to_le_bytes()
        ],
        bump
    )]
    pub withdraw_request: Box<Account<'info, WithdrawRequest>>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn request_withdraw(ctx: Context<RequestWithdraw>, shares: u64, min_amount: u64) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let clock = Clock::get()?;

    // Validate shares
    require!(shares > 0, VaultError::InsufficientShares);
    require!(
        ctx.accounts.user_vlofi.amount >= shares,
        VaultError::InsufficientShares
    );

    // Calculate CCM amount at current exchange rate
    let ccm_amount = vault.calculate_redeem_amount(shares)?;
    require!(ccm_amount >= min_amount, VaultError::SlippageExceeded);

    // Burn vLOFI shares
    let burn_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        Burn {
            mint: ctx.accounts.vlofi_mint.to_account_info(),
            from: ctx.accounts.user_vlofi.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        },
    );
    token::burn(burn_ctx, shares)?;

    // Initialize user state if new
    let user_state = &mut ctx.accounts.user_vault_state;
    if user_state.user == Pubkey::default() {
        user_state.bump = ctx.bumps.user_vault_state;
        user_state.user = ctx.accounts.user.key();
        user_state.vault = vault.key();
        user_state.next_request_id = 0;
        user_state.total_deposited = 0;
        user_state.total_redeemed = 0;
    }

    let request_id = user_state.next_request_id;
    let completion_slot = clock.slot.saturating_add(vault.withdraw_queue_slots);

    // Create withdraw request
    let request = &mut ctx.accounts.withdraw_request;
    request.bump = ctx.bumps.withdraw_request;
    request.user = ctx.accounts.user.key();
    request.vault = vault.key();
    request.request_id = request_id;
    request.shares_burned = shares;
    request.ccm_amount = ccm_amount;
    request.request_slot = clock.slot;
    request.completion_slot = completion_slot;
    request.completed = false;

    // Update user state
    user_state.next_request_id = request_id.saturating_add(1);
    user_state.total_redeemed = user_state.total_redeemed.saturating_add(shares);

    // Update vault state
    let vault = &mut ctx.accounts.vault;
    vault.total_shares = vault
        .total_shares
        .checked_sub(shares)
        .ok_or(VaultError::MathOverflow)?;
    vault.pending_withdrawals = vault
        .pending_withdrawals
        .checked_add(ccm_amount)
        .ok_or(VaultError::MathOverflow)?;

    emit!(WithdrawRequested {
        user: ctx.accounts.user.key(),
        vault: vault.key(),
        request_id,
        shares_burned: shares,
        ccm_amount,
        completion_slot,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Withdraw requested: {} vLOFI -> {} CCM, completes at slot {}",
        shares,
        ccm_amount,
        completion_slot
    );

    Ok(())
}

// =============================================================================
// COMPLETE WITHDRAW
// =============================================================================

#[derive(Accounts)]
#[instruction(request_id: u64)]
pub struct CompleteWithdraw<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [VAULT_SEED, vault.channel_config.as_ref()],
        bump = vault.bump,
    )]
    pub vault: Box<Account<'info, ChannelVault>>,

    #[account(
        mut,
        seeds = [VAULT_ORACLE_POSITION_SEED, vault.key().as_ref()],
        bump = vault_oracle_position.bump,
    )]
    pub vault_oracle_position: Box<Account<'info, VaultOraclePosition>>,

    /// Withdraw request to complete
    #[account(
        mut,
        close = user,
        seeds = [
            WITHDRAW_REQUEST_SEED,
            vault.key().as_ref(),
            user.key().as_ref(),
            &request_id.to_le_bytes()
        ],
        bump = withdraw_request.bump,
        constraint = withdraw_request.user == user.key() @ VaultError::Unauthorized,
        constraint = !withdraw_request.completed @ VaultError::WithdrawAlreadyCompleted,
    )]
    pub withdraw_request: Box<Account<'info, WithdrawRequest>>,

    /// CCM mint
    #[account(address = vault.ccm_mint)]
    pub ccm_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// Vault's CCM buffer
    #[account(
        mut,
        seeds = [VAULT_CCM_BUFFER_SEED, vault.key().as_ref()],
        bump,
    )]
    pub vault_ccm_buffer: Box<InterfaceAccount<'info, TokenAccount>>,

    /// User's CCM token account
    #[account(
        mut,
        constraint = user_ccm.owner == user.key() @ VaultError::Unauthorized,
        constraint = user_ccm.mint == ccm_mint.key() @ VaultError::InvalidMint,
    )]
    pub user_ccm: Box<InterfaceAccount<'info, TokenAccount>>,

    // -------------------------------------------------------------------------
    // Oracle Accounts (for potential unstake)
    // -------------------------------------------------------------------------

    /// CHECK: Oracle program
    #[account(address = token_2022::ID)]
    pub oracle_program: AccountInfo<'info>,

    /// Oracle channel config
    #[account(address = vault.channel_config)]
    pub oracle_channel_config: Box<Account<'info, ChannelConfigV2>>,

    /// Oracle stake pool
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_POOL_SEED, oracle_channel_config.key().as_ref()],
        bump = oracle_stake_pool.bump,
        seeds::program = token_2022::ID,
    )]
    pub oracle_stake_pool: Box<Account<'info, ChannelStakePool>>,

    /// Oracle vault
    #[account(
        mut,
        seeds = [STAKE_VAULT_SEED, oracle_stake_pool.key().as_ref()],
        bump,
        seeds::program = token_2022::ID,
    )]
    pub oracle_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Vault's stake position in Oracle
    /// CHECK: May be empty if no active stake
    #[account(mut)]
    pub oracle_user_stake: UncheckedAccount<'info>,

    /// NFT mint
    /// CHECK: Used for unstake CPI
    #[account(mut)]
    pub oracle_nft_mint: UncheckedAccount<'info>,

    /// Vault's NFT ATA
    /// CHECK: Used for unstake CPI
    #[account(mut)]
    pub vault_nft_ata: UncheckedAccount<'info>,

    #[account(
        constraint = token_2022_program.key() == TOKEN_2022_PROGRAM_ID @ VaultError::InvalidTokenProgram,
    )]
    pub token_2022_program: Interface<'info, TokenInterface>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn complete_withdraw(ctx: Context<CompleteWithdraw>) -> Result<()> {
    let request = &ctx.accounts.withdraw_request;
    let clock = Clock::get()?;

    // Check queue period complete
    require!(
        clock.slot >= request.completion_slot,
        VaultError::WithdrawQueueNotComplete
    );

    let ccm_amount = request.ccm_amount;
    let vault = &ctx.accounts.vault;
    let position = &ctx.accounts.vault_oracle_position;

    let channel_config_key = vault.channel_config;
    let vault_bump = vault.bump;

    // Check if we have enough in buffer
    let buffer_balance_before = ctx.accounts.vault_ccm_buffer.amount;

    if buffer_balance_before < ccm_amount {
        // Need to unstake from Oracle
        require!(position.is_active, VaultError::InsufficientVaultBalance);
        require!(
            clock.slot >= position.lock_end_slot,
            VaultError::OracleStakeLocked
        );

        // Unstake all from Oracle
        let signer_seeds: &[&[&[u8]]] = &[&[
            VAULT_SEED,
            channel_config_key.as_ref(),
            &[vault_bump],
        ]];

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

        // Reload buffer to get actual received amount (may differ due to transfer fees)
        ctx.accounts.vault_ccm_buffer.reload()?;
        let buffer_balance_after = ctx.accounts.vault_ccm_buffer.amount;
        let actual_received = buffer_balance_after.saturating_sub(buffer_balance_before);

        // Update position
        let position = &mut ctx.accounts.vault_oracle_position;
        let vault = &mut ctx.accounts.vault;
        vault.total_staked = 0;
        position.is_active = false;
        position.stake_amount = 0;

        // Track excess CCM that returned (actual_received - ccm_amount we'll transfer out)
        // This goes back to pending_deposits so it's available for share pricing
        let excess = actual_received.saturating_sub(ccm_amount);
        if excess > 0 {
            vault.pending_deposits = vault
                .pending_deposits
                .checked_add(excess)
                .ok_or(VaultError::MathOverflow)?;
            msg!("Unstake returned {} CCM, using {}, excess {} to pending",
                 actual_received, ccm_amount, excess);
        }
    }

    // Transfer CCM to user
    // Note: CCM has 0.5% transfer fee. User receives ccm_amount - fee.
    // Vault accounting is based on what we send (ccm_amount), not what user receives.
    // This is correct: the fee is borne by the recipient on exit, not the vault.
    let signer_seeds: &[&[&[u8]]] = &[&[
        VAULT_SEED,
        channel_config_key.as_ref(),
        &[vault_bump],
    ]];

    let transfer_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_2022_program.to_account_info(),
        TransferChecked {
            from: ctx.accounts.vault_ccm_buffer.to_account_info(),
            mint: ctx.accounts.ccm_mint.to_account_info(),
            to: ctx.accounts.user_ccm.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        },
        signer_seeds,
    );
    anchor_spl::token_interface::transfer_checked(
        transfer_ctx,
        ccm_amount,
        ctx.accounts.ccm_mint.decimals,
    )?;

    // Update vault
    let vault = &mut ctx.accounts.vault;
    vault.pending_withdrawals = vault
        .pending_withdrawals
        .checked_sub(ccm_amount)
        .ok_or(VaultError::MathOverflow)?;

    emit!(WithdrawCompleted {
        user: ctx.accounts.user.key(),
        vault: vault.key(),
        request_id: request.request_id,
        ccm_returned: ccm_amount,
        timestamp: clock.unix_timestamp,
    });

    msg!("Withdraw completed: {} CCM returned", ccm_amount);

    Ok(())
}

// =============================================================================
// INSTANT REDEEM (From Buffer + Reserve, 20% Penalty)
// =============================================================================

/// Instant redemption with 20% penalty.
/// Only available when Oracle stake is locked.
/// Draws from buffer + emergency reserve (does NOT touch Oracle).
#[derive(Accounts)]
pub struct InstantRedeem<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [VAULT_SEED, vault.channel_config.as_ref()],
        bump = vault.bump,
        constraint = !vault.paused @ VaultError::VaultPaused,
    )]
    pub vault: Box<Account<'info, ChannelVault>>,

    #[account(
        seeds = [VAULT_ORACLE_POSITION_SEED, vault.key().as_ref()],
        bump = vault_oracle_position.bump,
    )]
    pub vault_oracle_position: Box<Account<'info, VaultOraclePosition>>,

    /// vLOFI mint
    #[account(mut, address = vault.vlofi_mint)]
    pub vlofi_mint: Box<Account<'info, anchor_spl::token::Mint>>,

    /// User's vLOFI account
    #[account(
        mut,
        constraint = user_vlofi.owner == user.key() @ VaultError::Unauthorized,
        constraint = user_vlofi.mint == vlofi_mint.key() @ VaultError::InvalidMint,
    )]
    pub user_vlofi: Box<Account<'info, anchor_spl::token::TokenAccount>>,

    /// CCM mint
    #[account(address = vault.ccm_mint)]
    pub ccm_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// Vault's CCM buffer
    #[account(
        mut,
        seeds = [VAULT_CCM_BUFFER_SEED, vault.key().as_ref()],
        bump,
    )]
    pub vault_ccm_buffer: Box<InterfaceAccount<'info, TokenAccount>>,

    /// User's CCM token account
    #[account(
        mut,
        constraint = user_ccm.owner == user.key() @ VaultError::Unauthorized,
        constraint = user_ccm.mint == ccm_mint.key() @ VaultError::InvalidMint,
    )]
    pub user_ccm: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,

    #[account(
        constraint = token_2022_program.key() == TOKEN_2022_PROGRAM_ID @ VaultError::InvalidTokenProgram,
    )]
    pub token_2022_program: Interface<'info, TokenInterface>,

    pub system_program: Program<'info, System>,
}

pub fn instant_redeem(ctx: Context<InstantRedeem>, shares: u64, min_amount: u64) -> Result<()> {
    use crate::constants::{BPS_DENOMINATOR, EMERGENCY_PENALTY_BPS};
    use crate::events::InstantRedeemed;

    let vault = &ctx.accounts.vault;
    let position = &ctx.accounts.vault_oracle_position;
    let clock = Clock::get()?;

    // Validate shares
    require!(shares > 0, VaultError::InsufficientShares);
    require!(
        ctx.accounts.user_vlofi.amount >= shares,
        VaultError::InsufficientShares
    );

    // Instant redeem only available when Oracle stake is locked
    // (otherwise user should use normal withdrawal queue which is cheaper)
    require!(
        position.is_active && position.lock_end_slot > clock.slot,
        VaultError::InstantRedeemNotAvailable
    );

    // Calculate CCM amount at current rate
    let ccm_gross = vault.calculate_redeem_amount(shares)?;

    // Apply 20% penalty: user receives 80%
    let penalty_bps = EMERGENCY_PENALTY_BPS;
    let return_bps = BPS_DENOMINATOR.saturating_sub(penalty_bps);
    let ccm_returned = (ccm_gross as u128)
        .checked_mul(return_bps as u128)
        .ok_or(VaultError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(VaultError::MathOverflow)? as u64;
    let penalty_amount = ccm_gross.saturating_sub(ccm_returned);

    // Slippage protection (accounts for 20% penalty + potential transfer fee)
    require!(ccm_returned >= min_amount, VaultError::SlippageExceeded);

    // Check available liquidity: buffer minus queue-reserved funds
    // This prevents instant redeems from consuming funds reserved for queued withdrawals
    let buffer_balance = ctx.accounts.vault_ccm_buffer.amount;
    let available = vault.available_for_instant_redeem(buffer_balance)?;
    require!(available >= ccm_returned, VaultError::InsufficientReserve);

    let channel_config_key = vault.channel_config;
    let vault_bump = vault.bump;

    // Burn vLOFI shares
    let burn_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        Burn {
            mint: ctx.accounts.vlofi_mint.to_account_info(),
            from: ctx.accounts.user_vlofi.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        },
    );
    token::burn(burn_ctx, shares)?;

    // Transfer CCM to user (80%)
    let user_balance_before = ctx.accounts.user_ccm.amount;
    let signer_seeds: &[&[&[u8]]] = &[&[
        VAULT_SEED,
        channel_config_key.as_ref(),
        &[vault_bump],
    ]];

    let transfer_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_2022_program.to_account_info(),
        TransferChecked {
            from: ctx.accounts.vault_ccm_buffer.to_account_info(),
            mint: ctx.accounts.ccm_mint.to_account_info(),
            to: ctx.accounts.user_ccm.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        },
        signer_seeds,
    );
    anchor_spl::token_interface::transfer_checked(
        transfer_ctx,
        ccm_returned,
        ctx.accounts.ccm_mint.decimals,
    )?;

    // Reload user CCM account to measure actual received (accounts for transfer fees).
    ctx.accounts.user_ccm.reload()?;
    let user_balance_after = ctx.accounts.user_ccm.amount;
    let actual_received = user_balance_after.saturating_sub(user_balance_before);
    require!(actual_received >= min_amount, VaultError::SlippageExceeded);

    // Update vault state
    let vault = &mut ctx.accounts.vault;
    vault.total_shares = vault
        .total_shares
        .checked_sub(shares)
        .ok_or(VaultError::MathOverflow)?;

    // Use emergency reserve first for instant exits, then draw from pending deposits.
    let reserve_to_use = vault.emergency_reserve.min(ccm_returned);
    let from_pending = ccm_returned.saturating_sub(reserve_to_use);

    if reserve_to_use > 0 {
        vault.emergency_reserve = vault
            .emergency_reserve
            .checked_sub(reserve_to_use)
            .ok_or(VaultError::MathOverflow)?;
    }

    // Reduce pending_deposits by the portion actually leaving from the non-reserve pool.
    if from_pending > 0 {
        vault.pending_deposits = vault
            .pending_deposits
            .checked_sub(from_pending)
            .ok_or(VaultError::MathOverflow)?;
    }

    // Penalty stays in buffer - move into emergency reserve (up to cap).
    // Reserve is included in NAV, so we must subtract the moved amount from pending_deposits.
    let added_to_reserve = vault.add_to_reserve(penalty_amount)?;
    if added_to_reserve > 0 {
        vault.pending_deposits = vault
            .pending_deposits
            .checked_sub(added_to_reserve)
            .ok_or(VaultError::MathOverflow)?;
    }

    emit!(InstantRedeemed {
        user: ctx.accounts.user.key(),
        vault: vault.key(),
        shares_burned: shares,
        ccm_gross,
        ccm_returned,
        penalty_to_reserve: added_to_reserve,
        reserve_balance: vault.emergency_reserve,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Instant redeem: {} vLOFI -> {} CCM (gross {}, penalty {} -> {} to reserve)",
        shares,
        ccm_returned,
        ccm_gross,
        penalty_amount,
        added_to_reserve
    );

    Ok(())
}

// =============================================================================
// ADMIN EMERGENCY UNSTAKE (Break Glass - Affects All Stakers)
// =============================================================================

/// Admin-only emergency unstake from Oracle.
/// WARNING: This triggers a 20% penalty on ALL staked CCM, affecting all shareholders.
/// Only use in catastrophic scenarios (e.g., Oracle exploit, protocol shutdown).
#[derive(Accounts)]
pub struct AdminEmergencyUnstake<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [VAULT_SEED, vault.channel_config.as_ref()],
        bump = vault.bump,
        constraint = vault.admin == admin.key() @ VaultError::Unauthorized,
    )]
    pub vault: Box<Account<'info, ChannelVault>>,

    #[account(
        mut,
        seeds = [VAULT_ORACLE_POSITION_SEED, vault.key().as_ref()],
        bump = vault_oracle_position.bump,
        constraint = vault_oracle_position.is_active @ VaultError::VaultStakeNotActive,
    )]
    pub vault_oracle_position: Box<Account<'info, VaultOraclePosition>>,

    /// CCM mint
    #[account(address = vault.ccm_mint)]
    pub ccm_mint: Box<InterfaceAccount<'info, MintInterface>>,

    /// Vault's CCM buffer
    #[account(
        mut,
        seeds = [VAULT_CCM_BUFFER_SEED, vault.key().as_ref()],
        bump,
    )]
    pub vault_ccm_buffer: Box<InterfaceAccount<'info, TokenAccount>>,

    // -------------------------------------------------------------------------
    // Oracle Accounts
    // -------------------------------------------------------------------------

    /// CHECK: Oracle program
    #[account(address = token_2022::ID)]
    pub oracle_program: AccountInfo<'info>,

    /// Oracle channel config
    #[account(address = vault.channel_config)]
    pub oracle_channel_config: Box<Account<'info, ChannelConfigV2>>,

    /// Oracle stake pool
    #[account(
        mut,
        seeds = [CHANNEL_STAKE_POOL_SEED, oracle_channel_config.key().as_ref()],
        bump = oracle_stake_pool.bump,
        seeds::program = token_2022::ID,
    )]
    pub oracle_stake_pool: Box<Account<'info, ChannelStakePool>>,

    /// Oracle vault
    #[account(
        mut,
        seeds = [STAKE_VAULT_SEED, oracle_stake_pool.key().as_ref()],
        bump,
        seeds::program = token_2022::ID,
    )]
    pub oracle_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Vault's stake position in Oracle
    /// CHECK: Required for emergency unstake
    #[account(mut)]
    pub oracle_user_stake: UncheckedAccount<'info>,

    /// NFT mint
    /// CHECK: Used for unstake CPI
    #[account(mut)]
    pub oracle_nft_mint: UncheckedAccount<'info>,

    /// Vault's NFT ATA
    /// CHECK: Used for unstake CPI
    #[account(mut)]
    pub vault_nft_ata: UncheckedAccount<'info>,

    #[account(
        constraint = token_2022_program.key() == TOKEN_2022_PROGRAM_ID @ VaultError::InvalidTokenProgram,
    )]
    pub token_2022_program: Interface<'info, TokenInterface>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn admin_emergency_unstake<'info>(
    ctx: Context<'_, '_, '_, 'info, AdminEmergencyUnstake<'info>>,
) -> Result<()> {
    use crate::events::AdminEmergencyUnstaked;

    let vault = &ctx.accounts.vault;
    let position = &ctx.accounts.vault_oracle_position;
    let clock = Clock::get()?;

    let oracle_stake_before = position.stake_amount;
    let channel_config_key = vault.channel_config;
    let vault_bump = vault.bump;

    // Capture buffer balance before
    let buffer_before = ctx.accounts.vault_ccm_buffer.amount;

    // Emergency unstake from Oracle (20% penalty)
    let signer_seeds: &[&[&[u8]]] = &[&[
        VAULT_SEED,
        channel_config_key.as_ref(),
        &[vault_bump],
    ]];

    let emergency_accounts = EmergencyUnstakeChannel {
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

    let emergency_ctx = CpiContext::new_with_signer(
        ctx.accounts.oracle_program.to_account_info(),
        emergency_accounts,
        signer_seeds,
    );

    token_2022::cpi::emergency_unstake_channel(emergency_ctx)?;

    // Reload buffer to see what we received
    ctx.accounts.vault_ccm_buffer.reload()?;
    let buffer_after = ctx.accounts.vault_ccm_buffer.amount;
    let ccm_returned = buffer_after.saturating_sub(buffer_before);
    let oracle_penalty = oracle_stake_before.saturating_sub(ccm_returned);

    // Update vault state - all staked CCM is now pending deposits
    let vault = &mut ctx.accounts.vault;
    vault.total_staked = 0;
    vault.pending_deposits = vault
        .pending_deposits
        .checked_add(ccm_returned)
        .ok_or(VaultError::MathOverflow)?;

    // Update position
    let position = &mut ctx.accounts.vault_oracle_position;
    position.is_active = false;
    position.stake_amount = 0;

    emit!(AdminEmergencyUnstaked {
        vault: vault.key(),
        admin: ctx.accounts.admin.key(),
        oracle_stake_before,
        ccm_returned,
        oracle_penalty,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "ADMIN EMERGENCY UNSTAKE: {} CCM was staked, {} returned (Oracle penalty: {})",
        oracle_stake_before,
        ccm_returned,
        oracle_penalty
    );

    Ok(())
}
