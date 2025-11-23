use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};
use switchboard_on_demand::on_demand::accounts::PullFeedAccountData;
use switchboard_on_demand::prelude::rust_decimal::prelude::ToPrimitive;
use switchboard_on_demand::prelude::rust_decimal::Decimal;
use switchboard_on_demand::program_id::{ON_DEMAND_DEVNET_PID, ON_DEMAND_MAINNET_PID};
use std::str::FromStr;

declare_id!("G2v5XVA4SZnZ5NVLSC7pHJp9JRWSN13jHoXQ9ebpujvB");

const MINT_REWARD_DISCRIMINATOR: [u8; 8] = [66, 151, 112, 62, 139, 184, 250, 209];

#[program]
pub mod x402_on_demand {
    use super::*;

    pub fn initialize_session(ctx: Context<InitializeSession>, amount: u64) -> Result<()> {
        let session = &mut ctx.accounts.payment_session;
        session.payer = ctx.accounts.payer.key();
        session.amount = amount;
        session.mint = ctx.accounts.mint.key();
        session.settled = false;
        Ok(())
    }

    pub fn settle_x402_payment(ctx: Context<SettleX402Payment>) -> Result<()> {
        // Parse feed manually (most efficient)
        let feed_data = ctx.accounts.switchboard_feed.data.borrow();
        let feed = PullFeedAccountData::parse(feed_data).map_err(|_| ErrorCode::InvalidFeed)?;

        // Fresh price guaranteed because client added pull ix first
        let clock = Clock::get()?;
        let price_value = feed
            .get_value(clock.slot, 90, 1, true)
            .map_err(|_| ErrorCode::StaleFeed)?;

        // Safe u128 math – no f64 in critical path
        let amount_dec = Decimal::from_i128_with_scale(ctx.accounts.payment_session.amount as i128, 0);
        let reward_dec = (amount_dec * price_value)
            / Decimal::from_i128_with_scale(1_000_000, 0);
        let adjusted_reward = reward_dec
            .trunc()
            .to_u64()
            .ok_or(ErrorCode::MathOverflow)?;

        // Transfer payment (works with Token-2022 or classic)
        token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.from.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.to.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            ctx.accounts.payment_session.amount,
            ctx.accounts.mint.decimals,
        )?;

        // Manual CPI to attention-oracle-program (discriminator precomputed)
        let mut ix_data = MINT_REWARD_DISCRIMINATOR.to_vec();
        ix_data.extend_from_slice(&adjusted_reward.to_le_bytes());

        let attention_oracle_pid =
            Pubkey::from_str("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop").unwrap();

        let mint_reward_ix = Instruction {
            program_id: attention_oracle_pid,
            accounts: vec![
                AccountMeta::new(ctx.accounts.reward_mint.key(), false),
                AccountMeta::new(ctx.accounts.reward_ata.key(), false),
                AccountMeta::new(ctx.accounts.protocol_authority.key(), true),
                AccountMeta::new_readonly(ctx.accounts.token_program.key(), false),
                AccountMeta::new_readonly(ctx.accounts.system_program.key(), false),
            ],
            data: ix_data,
        };

        let signer_seeds: &[&[u8]] = &[b"global", &[ctx.bumps.protocol_authority]];

        invoke_signed(
            &mint_reward_ix,
            &[
                ctx.accounts.reward_mint.to_account_info(),
                ctx.accounts.reward_ata.to_account_info(),
                ctx.accounts.protocol_authority.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[signer_seeds],
        )?;

        ctx.accounts.payment_session.settled = true;

        emit!(PaymentSettled {
            payer: ctx.accounts.authority.key(),
            payment_amount: ctx.accounts.payment_session.amount,
            reward_amount: adjusted_reward,
            price_mantissa: price_value.mantissa(),
            price_scale: price_value.scale(),
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeSession<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + 32 + 8 + 32 + 1,
        seeds = [b"session", payer.key().as_ref()],
        bump,
    )]
    pub payment_session: Account<'info, PaymentSession>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SettleX402Payment<'info> {
    #[account(mut, constraint = payment_session.settled == false)]
    pub payment_session: Account<'info, PaymentSession>,
    #[account(mut)]
    pub from: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub to: InterfaceAccount<'info, TokenAccount>,
    pub authority: Signer<'info>,
    /// CHECK: owner validated by constraint (Switchboard On-Demand devnet/mainnet)
    #[account(
        constraint = *switchboard_feed.owner == ON_DEMAND_DEVNET_PID
            || *switchboard_feed.owner == ON_DEMAND_MAINNET_PID
            || *switchboard_feed.owner == switchboard_on_demand::program_id::SWITCHBOARD_PROGRAM_ID
            @ ErrorCode::InvalidFeed
    )]
    pub switchboard_feed: AccountInfo<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    #[account(mut)]
    pub reward_mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub reward_ata: InterfaceAccount<'info, TokenAccount>,
    /// CHECK: protocol PDA
    #[account(seeds = [b"global"], bump)]
    pub protocol_authority: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct PaymentSession {
    pub payer: Pubkey,
    pub amount: u64,
    pub mint: Pubkey,
    pub settled: bool,
}

#[event]
pub struct PaymentSettled {
    pub payer: Pubkey,
    pub payment_amount: u64,
    pub reward_amount: u64,
    pub price_mantissa: i128,
    pub price_scale: u32,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Stale or invalid feed")]
    StaleFeed,
    #[msg("Invalid feed account")]
    InvalidFeed,
    #[msg("Math overflow")]
    MathOverflow,
}
