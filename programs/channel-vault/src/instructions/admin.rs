//! Admin instructions for vault management.

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint as MintInterface, TokenAccount, TokenInterface, TransferChecked};

use crate::constants::{TOKEN_2022_PROGRAM_ID, VAULT_ORACLE_POSITION_SEED, VAULT_SEED};
use crate::errors::VaultError;
use crate::events::{AdminUpdated, CapitalInjected, LockDurationSlotsUpdated, OraclePositionSynced, VaultPaused, VaultResumed, WithdrawQueueSlotsUpdated};
use crate::state::{ChannelVault, VaultOraclePosition};

use token_2022::UserChannelStake;

#[derive(Accounts)]
pub struct AdminAction<'info> {
    #[account(
        constraint = admin.key() == vault.admin @ VaultError::Unauthorized
    )]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [VAULT_SEED, vault.channel_config.as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, ChannelVault>,
}

/// Pause the vault (stops deposits).
pub fn pause(ctx: Context<AdminAction>) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    vault.paused = true;

    emit!(VaultPaused {
        vault: vault.key(),
        admin: ctx.accounts.admin.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!("Vault paused");
    Ok(())
}

/// Resume the vault.
pub fn resume(ctx: Context<AdminAction>) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    vault.paused = false;

    emit!(VaultResumed {
        vault: vault.key(),
        admin: ctx.accounts.admin.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!("Vault resumed");
    Ok(())
}

/// Update admin authority.
pub fn update_admin(ctx: Context<AdminAction>, new_admin: Pubkey) -> Result<()> {
    require!(
        new_admin != Pubkey::default(),
        VaultError::InvalidPubkey
    );

    let vault = &mut ctx.accounts.vault;
    let old_admin = vault.admin;
    vault.admin = new_admin;

    emit!(AdminUpdated {
        vault: vault.key(),
        old_admin,
        new_admin,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!("Admin updated: {} -> {}", old_admin, new_admin);
    Ok(())
}

// ---------------------------------------------------------------------------
// Sync Oracle Position
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct SyncOraclePosition<'info> {
    #[account(
        constraint = admin.key() == vault.admin @ VaultError::Unauthorized
    )]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [VAULT_SEED, vault.channel_config.as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, ChannelVault>,

    #[account(
        mut,
        seeds = [VAULT_ORACLE_POSITION_SEED, vault.key().as_ref()],
        bump = vault_oracle_position.bump,
    )]
    pub vault_oracle_position: Account<'info, VaultOraclePosition>,

    /// Oracle UserChannelStake for this vault.
    #[account(
        constraint = oracle_user_stake.user == vault.key() @ VaultError::InvalidOracleAccount,
        constraint = oracle_user_stake.channel == vault.channel_config @ VaultError::InvalidOracleAccount,
    )]
    pub oracle_user_stake: Account<'info, UserChannelStake>,
}

/// Sync vault oracle position state from the Oracle's UserChannelStake.
/// Fixes drift between local VaultOraclePosition and on-chain Oracle state.
pub fn sync_oracle_position(ctx: Context<SyncOraclePosition>) -> Result<()> {
    let position = &mut ctx.accounts.vault_oracle_position;
    let stake = &ctx.accounts.oracle_user_stake;

    position.is_active = stake.amount > 0;
    position.stake_amount = stake.amount;
    position.lock_end_slot = stake.lock_end_slot;
    position.oracle_nft_mint = stake.nft_mint;
    position.oracle_user_stake = ctx.accounts.oracle_user_stake.key();

    // Correct vault.total_staked to match Oracle truth
    let vault = &mut ctx.accounts.vault;
    vault.total_staked = stake.amount;

    emit!(OraclePositionSynced {
        vault: vault.key(),
        admin: ctx.accounts.admin.key(),
        is_active: position.is_active,
        stake_amount: position.stake_amount,
        lock_end_slot: position.lock_end_slot,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "Oracle position synced: active={}, amount={}, total_staked corrected",
        position.is_active,
        position.stake_amount
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Update Withdraw Queue Slots
// ---------------------------------------------------------------------------

/// Update withdrawal queue duration in slots (admin only).
pub fn update_withdraw_queue_slots(
    ctx: Context<AdminAction>,
    new_withdraw_queue_slots: u64,
) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    let old_value = vault.withdraw_queue_slots;
    vault.withdraw_queue_slots = new_withdraw_queue_slots;

    emit!(WithdrawQueueSlotsUpdated {
        vault: vault.key(),
        admin: ctx.accounts.admin.key(),
        old_withdraw_queue_slots: old_value,
        new_withdraw_queue_slots,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "Withdraw queue slots updated: {} -> {}",
        old_value,
        new_withdraw_queue_slots
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Update Lock Duration Slots
// ---------------------------------------------------------------------------

/// Update lock duration in slots (admin only).
///
/// Controls how long each compound stake is locked in the Oracle.
/// Reducing this shortens the time before users can request withdrawals.
pub fn update_lock_duration_slots(
    ctx: Context<AdminAction>,
    new_lock_duration_slots: u64,
) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    let old_value = vault.lock_duration_slots;
    vault.lock_duration_slots = new_lock_duration_slots;

    emit!(LockDurationSlotsUpdated {
        vault: vault.key(),
        admin: ctx.accounts.admin.key(),
        old_lock_duration_slots: old_value,
        new_lock_duration_slots,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "Lock duration slots updated: {} -> {}",
        old_value,
        new_lock_duration_slots
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Inject Capital (Insolvency Recovery)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct InjectCapital<'info> {
    #[account(
        mut,
        constraint = admin.key() == vault.admin @ VaultError::Unauthorized
    )]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [VAULT_SEED, vault.channel_config.as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, ChannelVault>,

    /// CCM mint (Token-2022)
    #[account(address = vault.ccm_mint)]
    pub ccm_mint: InterfaceAccount<'info, MintInterface>,

    /// Admin's CCM token account
    #[account(
        mut,
        constraint = admin_ccm.owner == admin.key() @ VaultError::Unauthorized,
        constraint = admin_ccm.mint == ccm_mint.key() @ VaultError::InvalidMint,
    )]
    pub admin_ccm: InterfaceAccount<'info, TokenAccount>,

    /// Vault's CCM buffer
    #[account(
        mut,
        address = vault.ccm_buffer,
    )]
    pub vault_ccm_buffer: InterfaceAccount<'info, TokenAccount>,

    /// Token-2022 program (for CCM transfer)
    #[account(
        constraint = token_2022_program.key() == TOKEN_2022_PROGRAM_ID @ VaultError::InvalidTokenProgram,
    )]
    pub token_2022_program: Interface<'info, TokenInterface>,
}

/// Admin injects capital to recover from insolvency.
///
/// Use case: When pending_withdrawals > (total_staked + pending_deposits + emergency_reserve),
/// the vault becomes insolvent and cannot honor withdrawal requests. This instruction
/// allows the admin to inject CCM to restore solvency.
///
/// The injected capital goes to pending_deposits and will be:
/// 1. Used to honor queued withdrawals (if any)
/// 2. Staked to Oracle on next compound
///
/// Note: CCM has a 0.5% transfer fee, so less than `amount` arrives in the vault.
pub fn inject_capital(ctx: Context<InjectCapital>, amount: u64) -> Result<()> {
    require!(amount > 0, VaultError::DepositTooSmall);

    let clock = Clock::get()?;

    // Capture buffer balance BEFORE transfer (for transfer-fee accounting)
    let balance_before = ctx.accounts.vault_ccm_buffer.amount;

    // Transfer CCM from admin to vault buffer (Token-2022)
    let transfer_ctx = CpiContext::new(
        ctx.accounts.token_2022_program.to_account_info(),
        TransferChecked {
            from: ctx.accounts.admin_ccm.to_account_info(),
            mint: ctx.accounts.ccm_mint.to_account_info(),
            to: ctx.accounts.vault_ccm_buffer.to_account_info(),
            authority: ctx.accounts.admin.to_account_info(),
        },
    );
    anchor_spl::token_interface::transfer_checked(
        transfer_ctx,
        amount,
        ctx.accounts.ccm_mint.decimals,
    )?;

    // Reload buffer to get actual received amount (post transfer-fee)
    ctx.accounts.vault_ccm_buffer.reload()?;
    let balance_after = ctx.accounts.vault_ccm_buffer.amount;
    let actual_received = balance_after
        .checked_sub(balance_before)
        .ok_or(VaultError::MathOverflow)?;

    // Add to pending_deposits (will be staked on next compound)
    let vault = &mut ctx.accounts.vault;
    vault.pending_deposits = vault
        .pending_deposits
        .checked_add(actual_received)
        .ok_or(VaultError::MathOverflow)?;

    // Check if vault is now solvent
    let gross_assets = vault.total_staked
        .checked_add(vault.pending_deposits)
        .ok_or(VaultError::MathOverflow)?
        .checked_add(vault.emergency_reserve)
        .ok_or(VaultError::MathOverflow)?;
    let is_solvent = gross_assets >= vault.pending_withdrawals;

    emit!(CapitalInjected {
        vault: vault.key(),
        admin: ctx.accounts.admin.key(),
        ccm_requested: amount,
        ccm_received: actual_received,
        pending_deposits_after: vault.pending_deposits,
        is_solvent,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Capital injected: {} CCM (requested {}), pending_deposits={}, solvent={}",
        actual_received,
        amount,
        vault.pending_deposits,
        is_solvent
    );

    Ok(())
}
