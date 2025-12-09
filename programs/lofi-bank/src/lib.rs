use anchor_lang::prelude::*;
use anchor_spl::token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked};

declare_id!("EHsyY7uroV6gRUt8gNB6eMXNtRdy5L9q6GA5um4teYTA");

const MAX_YIELD_BPS: u16 = 1_000; // 10%
const BASIS_POINTS: u64 = 10_000;

#[program]
pub mod lofi_bank {
    use super::*;

    pub fn initialize_treasury(ctx: Context<InitializeTreasury>, yield_bps: u16) -> Result<()> {
        require!(yield_bps <= MAX_YIELD_BPS, BankError::YieldTooHigh);
        let treasury = &mut ctx.accounts.treasury_state;
        treasury.bump = ctx.bumps.treasury_state;
        treasury.total_staked = 0;
        treasury.yield_bps = yield_bps;
        Ok(())
    }

    pub fn stake(ctx: Context<Stake>, amount: u64, lock_period: u32) -> Result<()> {
        require!(amount > 0, BankError::InvalidAmount);
        let now = Clock::get()?.unix_timestamp;

        let user_vault = &mut ctx.accounts.user_vault;
        let treasury = &mut ctx.accounts.treasury_state;

        user_vault.staked_amount = user_vault
            .staked_amount
            .checked_add(amount)
            .ok_or(BankError::MathOverflow)?;
        if lock_period > user_vault.lock_period {
            user_vault.lock_period = lock_period;
        }
        user_vault.last_stake_ts = now;
        user_vault.bump = ctx.bumps.user_vault;

        treasury.total_staked = treasury
            .total_staked
            .checked_add(amount)
            .ok_or(BankError::MathOverflow)?;

        let cpi_accounts = TransferChecked {
            from: ctx.accounts.user_token.to_account_info(),
            to: ctx.accounts.treasury_token.to_account_info(),
            authority: ctx.accounts.payer.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        transfer_checked(cpi_ctx, amount, ctx.accounts.mint.decimals)?;

        emit!(StakeEvent {
            payer: ctx.accounts.payer.key(),
            amount,
            lock_period,
        });
        Ok(())
    }

    pub fn unstake(ctx: Context<Unstake>, amount: u64) -> Result<()> {
        require!(amount > 0, BankError::InvalidAmount);
        let now = Clock::get()?.unix_timestamp;
        let user_vault = &mut ctx.accounts.user_vault;
        let treasury = &mut ctx.accounts.treasury_state;

        require!(
            user_vault.staked_amount >= amount,
            BankError::InsufficientStake
        );
        if user_vault.lock_period > 0 && user_vault.last_stake_ts > 0 {
            let unlock_time = user_vault
                .last_stake_ts
                .checked_add(user_vault.lock_period as i64)
                .ok_or(BankError::MathOverflow)?;
            require!(now >= unlock_time, BankError::LockActive);
        }

        user_vault.staked_amount = user_vault
            .staked_amount
            .checked_sub(amount)
            .ok_or(BankError::MathOverflow)?;
        treasury.total_staked = treasury
            .total_staked
            .checked_sub(amount)
            .ok_or(BankError::MathOverflow)?;

        let yield_amount = amount
            .saturating_mul(treasury.yield_bps as u64)
            .checked_div(BASIS_POINTS)
            .ok_or(BankError::MathOverflow)?;

        let treasury_seeds: &[&[u8]] = &[b"treasury_state", &[treasury.bump]];
        let signer_seeds = &[treasury_seeds];
        let cpi_accounts = TransferChecked {
            from: ctx.accounts.treasury_token.to_account_info(),
            to: ctx.accounts.user_token.to_account_info(),
            authority: ctx.accounts.treasury_state.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer_seeds,
        );
        transfer_checked(cpi_ctx, amount + yield_amount, ctx.accounts.mint.decimals)?;

        emit!(UnstakeEvent {
            payer: ctx.accounts.payer.key(),
            amount,
            yield_amount,
        });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeTreasury<'info> {
    #[account(
        init,
        payer = payer,
        space = TreasuryState::SPACE,
        seeds = [b"treasury_state"],
        bump
    )]
    pub treasury_state: Account<'info, TreasuryState>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(
        init_if_needed,
        payer = payer,
        space = UserVault::SPACE,
        seeds = [b"user_vault", payer.key().as_ref()],
        bump
    )]
    pub user_vault: Account<'info, UserVault>,
    #[account(
        mut,
        seeds = [b"treasury_state"],
        bump = treasury_state.bump,
    )]
    pub treasury_state: Account<'info, TreasuryState>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = payer,
    )]
    pub user_token: InterfaceAccount<'info, TokenAccount>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = treasury_state,
    )]
    pub treasury_token: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(
        mut,
        seeds = [b"user_vault", payer.key().as_ref()],
        bump = user_vault.bump,
    )]
    pub user_vault: Account<'info, UserVault>,
    #[account(
        mut,
        seeds = [b"treasury_state"],
        bump = treasury_state.bump,
    )]
    pub treasury_state: Account<'info, TreasuryState>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = payer,
    )]
    pub user_token: InterfaceAccount<'info, TokenAccount>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = treasury_state,
    )]
    pub treasury_token: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[account]
pub struct TreasuryState {
    pub total_staked: u64,
    pub yield_bps: u16,
    pub bump: u8,
}

impl TreasuryState {
    pub const SPACE: usize = 8  // discriminator
        + 8 // total_staked
        + 2 // yield_bps
        + 1; // bump
}

#[account]
pub struct UserVault {
    pub staked_amount: u64,
    pub lock_period: u32,
    pub last_stake_ts: i64,
    pub bump: u8,
}

impl UserVault {
    pub const SPACE: usize = 8  // discriminator
        + 8 // staked_amount
        + 4 // lock_period
        + 8 // last_stake_ts
        + 1; // bump
}

#[event]
pub struct StakeEvent {
    pub payer: Pubkey,
    pub amount: u64,
    pub lock_period: u32,
}

#[event]
pub struct UnstakeEvent {
    pub payer: Pubkey,
    pub amount: u64,
    pub yield_amount: u64,
}

#[error_code]
pub enum BankError {
    #[msg("amount must be greater than zero")]
    InvalidAmount,
    #[msg("yield basis points exceed maximum")]
    YieldTooHigh,
    #[msg("math overflow")]
    MathOverflow,
    #[msg("insufficient staked balance")]
    InsufficientStake,
    #[msg("tokens remain locked")]
    LockActive,
}
