use crate::{
    constants::{
        PROTOCOL_SEED, WITHDRAW_TRACKER_SEED,
        MAX_WITHDRAW_PER_TX, MAX_WITHDRAW_PER_DAY, SECONDS_PER_DAY,
    },
    errors::OracleError,
    events::{
        TreasuryWithdrawn, PublisherUpdated, PolicyUpdated,
        ProtocolPaused, AdminTransferred,
    },
    state::{ProtocolState, WithdrawTracker},
    token_transfer::transfer_checked_with_remaining,
};
use anchor_lang::prelude::*;
use anchor_spl::token_2022::spl_token_2022;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

/// Update the allowlisted publisher (singleton protocol_state)
#[derive(Accounts)]
pub struct UpdatePublisher<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_publisher(ctx: Context<UpdatePublisher>, new_publisher: Pubkey) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    let old_publisher = state.publisher;
    state.publisher = new_publisher;

    emit!(PublisherUpdated {
        admin: ctx.accounts.admin.key(),
        old_publisher,
        new_publisher,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Update the allowlisted publisher (open variant keyed by mint)
#[derive(Accounts)]
pub struct UpdatePublisherOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_publisher_open(
    ctx: Context<UpdatePublisherOpen>,
    new_publisher: Pubkey,
) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    let old_publisher = state.publisher;
    state.publisher = new_publisher;

    emit!(PublisherUpdated {
        admin: ctx.accounts.admin.key(),
        old_publisher,
        new_publisher,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Set receipt requirement policy (singleton protocol_state)
#[derive(Accounts)]
pub struct SetPolicy<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_policy(ctx: Context<SetPolicy>, require_receipt: bool) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.require_receipt = require_receipt;

    emit!(PolicyUpdated {
        admin: ctx.accounts.admin.key(),
        require_receipt,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Set receipt requirement policy (open variant keyed by mint)
#[derive(Accounts)]
pub struct SetPolicyOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_policy_open(ctx: Context<SetPolicyOpen>, require_receipt: bool) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.require_receipt = require_receipt;

    emit!(PolicyUpdated {
        admin: ctx.accounts.admin.key(),
        require_receipt,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Emergency pause/unpause (singleton protocol_state)
#[derive(Accounts)]
pub struct SetPaused<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_paused(ctx: Context<SetPaused>, paused: bool) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.paused = paused;

    emit!(ProtocolPaused {
        admin: ctx.accounts.admin.key(),
        paused,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Emergency pause/unpause (open variant keyed by mint)
#[derive(Accounts)]
pub struct SetPausedOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn set_paused_open(ctx: Context<SetPausedOpen>, paused: bool) -> Result<()> {
    let state = &mut ctx.accounts.protocol_state;
    state.paused = paused;

    emit!(ProtocolPaused {
        admin: ctx.accounts.admin.key(),
        paused,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Transfer admin authority (open variant keyed by mint)
/// Used for migrating to hardware wallet or new admin key
#[derive(Accounts)]
pub struct UpdateAdminOpen<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_admin_open(ctx: Context<UpdateAdminOpen>, new_admin: Pubkey) -> Result<()> {
    require!(new_admin != Pubkey::default(), OracleError::InvalidPubkey);
    let state = &mut ctx.accounts.protocol_state;
    let old_admin = state.admin;
    state.admin = new_admin;

    emit!(AdminTransferred {
        old_admin,
        new_admin,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

/// Transfer admin authority (singleton variant)
#[derive(Accounts)]
pub struct UpdateAdmin<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,
}

pub fn update_admin(ctx: Context<UpdateAdmin>, new_admin: Pubkey) -> Result<()> {
    require!(new_admin != Pubkey::default(), OracleError::InvalidPubkey);
    let state = &mut ctx.accounts.protocol_state;
    let old_admin = state.admin;
    state.admin = new_admin;

    emit!(AdminTransferred {
        old_admin,
        new_admin,
        mint: state.mint,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

// =============================================================================
// TREASURY WITHDRAW
// =============================================================================
//
// SECURITY CONTEXT (read before flagging as "backdoor"):
//
// This instruction exists for legitimate operational needs:
// - LP seeding (adding liquidity to DEXs like Meteora/Raydium)
// - Partner/creator payments
// - Protocol operational expenses
//
// It is NOT an unlimited withdrawal mechanism. Constraints:
//
// 1. RATE LIMITS: 50M per tx, 100M per day (~5% of supply)
//    - At max rate, full drain would take ~20 days
//    - Provides time window to detect compromise and respond
//
// 2. AUDIT TRAIL: Every withdrawal emits TreasuryWithdrawn event
//    - On-chain, immutable record of all fund movements
//    - Monitoring bots can alert on unusual patterns
//
// 3. GOVERNANCE ROADMAP:
//    - Current: Single admin key (operational phase)
//    - Phase 2: Multisig (3-of-5) before significant withdrawals
//    - Phase 3: DAO governance with timelock
//
// 4. PDA CONTROL: Treasury is owned by protocol_state PDA
//    - No private key can directly move funds
//    - Only this instruction (with constraints) can transfer
//
// The rate limits are a circuit breaker, not a security guarantee.
// The real security comes from key management (multisig) and monitoring.
// =============================================================================

/// Admin treasury withdrawal with rate limits.
/// - Per-transaction limit: 50M CCM
/// - Per-day limit: 100M CCM (~5% of 2B supply)
///
/// See SECURITY CONTEXT comment block above for rationale.
#[derive(Accounts)]
pub struct AdminWithdraw<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PROTOCOL_SEED, protocol_state.mint.as_ref()],
        bump = protocol_state.bump,
        constraint = admin.key() == protocol_state.admin @ OracleError::Unauthorized,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        init_if_needed,
        payer = admin,
        space = WithdrawTracker::LEN,
        seeds = [WITHDRAW_TRACKER_SEED, mint.key().as_ref()],
        bump,
    )]
    pub withdraw_tracker: Account<'info, WithdrawTracker>,

    #[account(
        constraint = mint.key() == protocol_state.mint @ OracleError::InvalidMint,
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    /// Treasury token account (owned by protocol_state PDA)
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = protocol_state,
        associated_token::token_program = token_program,
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,

    /// Destination token account for withdrawn funds
    #[account(
        mut,
        constraint = destination_ata.mint == mint.key() @ OracleError::InvalidMint,
        // FIX: Ensure the account is owned by the expected token program (Token-2022)
        constraint = *destination_ata.to_account_info().owner == spl_token_2022::ID @ OracleError::InvalidTokenProgram,
    )]
    pub destination_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        constraint = token_program.key() == spl_token_2022::ID @ OracleError::InvalidTokenProgram,
    )]
    pub token_program: Interface<'info, TokenInterface>,

    pub system_program: Program<'info, System>,
}

pub fn admin_withdraw<'info>(
    ctx: Context<'_, '_, '_, 'info, AdminWithdraw<'info>>,
    amount: u64,
) -> Result<()> {
    // Validate amount > 0
    require!(amount > 0, OracleError::InvalidAmount);

    // Validate per-transaction limit
    require!(amount <= MAX_WITHDRAW_PER_TX, OracleError::ExceedsWithdrawLimit);

    let clock = Clock::get()?;
    let now = clock.unix_timestamp;
    let tracker = &mut ctx.accounts.withdraw_tracker;

    // Initialize tracker if new
    if tracker.version == 0 {
        tracker.version = 1;
        tracker.mint = ctx.accounts.mint.key();
        tracker.day_start = now - (now % SECONDS_PER_DAY);
        tracker.withdrawn_today = 0;
        tracker.total_withdrawn = 0;
        tracker.last_withdraw_at = 0;
    }

    // Check if we've rolled into a new day (reset daily counter)
    let current_day_start = now - (now % SECONDS_PER_DAY);
    if current_day_start > tracker.day_start {
        tracker.day_start = current_day_start;
        tracker.withdrawn_today = 0;
    }

    // Validate daily limit
    let new_daily_total = tracker.withdrawn_today
        .checked_add(amount)
        .ok_or(OracleError::MathOverflow)?;
    require!(new_daily_total <= MAX_WITHDRAW_PER_DAY, OracleError::DailyLimitExceeded);

    // Validate treasury balance
    require!(
        ctx.accounts.treasury_ata.amount >= amount,
        OracleError::InsufficientTreasuryBalance
    );

    // Execute transfer (PDA signs)
    let mint_key = ctx.accounts.mint.key();
    let seeds = &[
        PROTOCOL_SEED,
        mint_key.as_ref(),
        &[ctx.accounts.protocol_state.bump],
    ];
    let signer_seeds = &[&seeds[..]];

    transfer_checked_with_remaining(
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.treasury_ata.to_account_info(),
        &ctx.accounts.mint.to_account_info(),
        &ctx.accounts.destination_ata.to_account_info(),
        &ctx.accounts.protocol_state.to_account_info(),
        amount,
        ctx.accounts.mint.decimals,
        signer_seeds,
        ctx.remaining_accounts,
    )?;

    // Update tracker
    tracker.withdrawn_today = new_daily_total;
    tracker.total_withdrawn = tracker.total_withdrawn
        .checked_add(amount)
        .ok_or(OracleError::MathOverflow)?;
    tracker.last_withdraw_at = now;

    // Emit audit event
    emit!(TreasuryWithdrawn {
        admin: ctx.accounts.admin.key(),
        destination: ctx.accounts.destination_ata.key(),
        amount,
        withdrawn_today: tracker.withdrawn_today,
        total_withdrawn: tracker.total_withdrawn,
        timestamp: now,
    });

    msg!(
        "Treasury withdraw: {} CCM to {}, daily total: {}/{}",
        amount / 1_000_000_000,
        ctx.accounts.destination_ata.key(),
        tracker.withdrawn_today / 1_000_000_000,
        MAX_WITHDRAW_PER_DAY / 1_000_000_000,
    );

    Ok(())
}
